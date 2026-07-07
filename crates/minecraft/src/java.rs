use std::collections::HashMap;
use std::path::{Path, PathBuf};

use downloads::{DownloadController, DownloadEvent, DownloadManager, DownloadTask};
use regex::Regex;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::error::MinecraftError;

const JAVA_RUNTIME_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Top level of `java-runtime/.../all.json`: platform key → component name
/// (`jre-legacy`, `java-runtime-gamma`, ...) → candidate builds, newest
/// first.
#[derive(Debug, Clone, Deserialize)]
pub struct JavaRuntimeManifest(HashMap<String, HashMap<String, Vec<JavaRuntimeEntry>>>);

#[derive(Debug, Clone, Deserialize)]
pub struct JavaRuntimeEntry {
    pub manifest: JavaManifestRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaManifestRef {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeFileManifest {
    files: HashMap<String, RuntimeFileEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeFileEntry {
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    downloads: Option<RuntimeFileDownloads>,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeFileDownloads {
    raw: RuntimeFileDownloadRaw,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeFileDownloadRaw {
    sha1: String,
    size: u64,
    url: String,
}

/// Looks for an existing Java installation: `$JAVA_HOME` first, then
/// whatever `java`/`java.exe` the shell would find on `PATH`. Returns the
/// path to the executable, not just a directory.
pub async fn find_system_java() -> Option<PathBuf> {
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join(java_binary_name());
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let which_command = if cfg!(windows) { "where" } else { "which" };
    let output = tokio::process::Command::new(which_command)
        .arg(java_binary_name())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())?;

    if first_line.is_empty() {
        None
    } else {
        Some(PathBuf::from(first_line))
    }
}

/// Runs `java -version` and parses the major version out of its (famously
/// stderr-only) output, handling both the old `1.8.0_401` scheme and the
/// modern `21.0.3` one.
pub async fn detect_java_major_version(java_path: &Path) -> Option<u32> {
    let output = tokio::process::Command::new(java_path)
        .arg("-version")
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    parse_java_major_version(&text)
}

fn parse_java_major_version(text: &str) -> Option<u32> {
    let pattern = Regex::new(r#"version "(\d+)(?:\.(\d+))?"#).ok()?;
    let captures = pattern.captures(text)?;
    let first: u32 = captures.get(1)?.as_str().parse().ok()?;
    if first == 1 {
        // Old versioning scheme: "1.8.0_401" means Java 8, the real major
        // version is the second dotted component.
        captures.get(2)?.as_str().parse().ok()
    } else {
        Some(first)
    }
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// Downloads and installs (if not already present) the Mojang-distributed
/// JRE build identified by `component` (taken from a version's
/// `javaVersion.component` field, e.g. `"java-runtime-gamma"`). Returns the
/// path to the resulting `java`/`javaw.exe` executable.
///
/// This is what lets a brand new instance launch with zero manual Java
/// setup — the same thing the official launcher does.
pub async fn ensure_managed_runtime(
    client: &reqwest::Client,
    download_manager: &DownloadManager,
    runtime_dir: &Path,
    component: &str,
    progress: UnboundedSender<DownloadEvent>,
) -> Result<PathBuf, MinecraftError> {
    let platform_key = runtime_platform_key().ok_or_else(|| MinecraftError::NoJavaAvailable {
        component: component.to_string(),
    })?;

    let java_root = runtime_dir.join(component).join(platform_key).join(component);
    let executable = managed_java_executable_path(&java_root);
    let sentinel = java_root.join(".install-complete");

    if executable.exists() && sentinel.exists() {
        return Ok(executable);
    }

    let manifest = fetch_runtime_manifest(client).await?;
    let entry = manifest
        .0
        .get(platform_key)
        .and_then(|by_component| by_component.get(component))
        .and_then(|candidates| candidates.first())
        .ok_or_else(|| MinecraftError::NoJavaAvailable {
            component: component.to_string(),
        })?;

    let file_manifest = fetch_runtime_file_manifest(client, &entry.manifest.url).await?;

    let mut tasks = Vec::new();
    let mut executable_relative_paths = Vec::new();

    for (relative_path, file_entry) in &file_manifest.files {
        match file_entry.entry_type.as_str() {
            "directory" => {
                let dir = java_root.join(relative_path);
                tokio::fs::create_dir_all(&dir).await.map_err(|source| MinecraftError::Io {
                    context: format!("creating runtime directory {}", dir.display()),
                    source,
                })?;
            }
            "file" => {
                if let Some(downloads) = &file_entry.downloads {
                    let destination = java_root.join(relative_path);
                    tasks.push(
                        DownloadTask::new(
                            downloads.raw.url.clone(),
                            destination,
                            format!("java runtime: {relative_path}"),
                        )
                        .with_sha1(downloads.raw.sha1.clone())
                        .with_size(downloads.raw.size),
                    );
                    if file_entry.executable {
                        executable_relative_paths.push(relative_path.clone());
                    }
                }
            }
            "link" => {
                create_runtime_symlink(&java_root, relative_path, file_entry.target.as_deref());
            }
            _ => {}
        }
    }

    download_manager.run(tasks, DownloadController::new(), progress).await?;

    mark_files_executable(&java_root, &executable_relative_paths).await;
    tokio::fs::write(&sentinel, b"ok").await.ok();

    if executable.exists() {
        Ok(executable)
    } else {
        Err(MinecraftError::NoJavaAvailable {
            component: component.to_string(),
        })
    }
}

#[cfg(unix)]
fn create_runtime_symlink(java_root: &Path, relative_path: &str, target: Option<&str>) {
    let Some(target) = target else { return };
    let link_path = java_root.join(relative_path);
    if let Some(parent) = link_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&link_path);
    let _ = std::os::unix::fs::symlink(target, &link_path);
}

#[cfg(not(unix))]
fn create_runtime_symlink(_java_root: &Path, _relative_path: &str, _target: Option<&str>) {
    // Windows runtime listings don't use "link" entries in practice; if a
    // future build ever does, we simply skip it rather than fail the whole
    // install over an auxiliary symlink.
}

#[cfg(unix)]
async fn mark_files_executable(java_root: &Path, relative_paths: &[String]) {
    use std::os::unix::fs::PermissionsExt;
    for relative_path in relative_paths {
        let path = java_root.join(relative_path);
        if let Ok(metadata) = tokio::fs::metadata(&path).await {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            let _ = tokio::fs::set_permissions(&path, permissions).await;
        }
    }
}

#[cfg(not(unix))]
async fn mark_files_executable(_java_root: &Path, _relative_paths: &[String]) {}

fn managed_java_executable_path(java_root: &Path) -> PathBuf {
    if cfg!(windows) {
        let windowed = java_root.join("bin").join("javaw.exe");
        if windowed.exists() {
            return windowed;
        }
        java_root.join("bin").join("java.exe")
    } else {
        java_root.join("bin").join("java")
    }
}

/// Mojang's runtime manifest keys platforms differently from
/// `std::env::consts`; this is the mapping. Returns `None` for platforms
/// Mojang doesn't publish a managed runtime for (e.g. 32-bit ARM, BSDs) —
/// callers fall back to requiring a system Java install in that case.
fn runtime_platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-x64"),
        ("windows", "x86") => Some("windows-x86"),
        ("windows", "aarch64") => Some("windows-arm64"),
        ("macos", "aarch64") => Some("mac-os-arm64"),
        ("macos", _) => Some("mac-os"),
        ("linux", "x86") => Some("linux-i386"),
        ("linux", "x86_64") => Some("linux"),
        _ => None,
    }
}

async fn fetch_runtime_manifest(client: &reqwest::Client) -> Result<JavaRuntimeManifest, MinecraftError> {
    let response = client.get(JAVA_RUNTIME_MANIFEST_URL).send().await?;
    let bytes = response.error_for_status()?.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|source| MinecraftError::Deserialize {
        context: "java-runtime all.json".to_string(),
        source,
    })
}

async fn fetch_runtime_file_manifest(
    client: &reqwest::Client,
    url: &str,
) -> Result<RuntimeFileManifest, MinecraftError> {
    let response = client.get(url).send().await?;
    let bytes = response.error_for_status()?.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|source| MinecraftError::Deserialize {
        context: format!("java runtime file manifest at {url}"),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_version_scheme() {
        let output = "openjdk version \"21.0.3\" 2024-04-16\nOpenJDK Runtime Environment\n";
        assert_eq!(parse_java_major_version(output), Some(21));
    }

    #[test]
    fn parses_legacy_1_dot_8_scheme() {
        let output = "java version \"1.8.0_401\"\nJava(TM) SE Runtime Environment\n";
        assert_eq!(parse_java_major_version(output), Some(8));
    }

    #[test]
    fn unparseable_output_returns_none() {
        assert_eq!(parse_java_major_version("not java output at all"), None);
    }

    #[test]
    fn runtime_platform_key_covers_common_targets() {
        // We can't change std::env::consts in a test, but we can confirm
        // the function returns *something* sensible for whichever platform
        // actually ran this test, since CI covers Linux, Windows, macOS.
        let key = runtime_platform_key();
        if matches!(std::env::consts::ARCH, "x86_64" | "aarch64") {
            assert!(key.is_some(), "should resolve a runtime key for common 64-bit targets");
        }
    }
}
