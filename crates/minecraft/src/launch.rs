use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use downloads::{DownloadController, DownloadEvent, DownloadManager, DownloadTask};
use md5::{Digest, Md5};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::assets;
use crate::error::MinecraftError;
use crate::java;
use crate::libraries;
use crate::manifest;
use crate::os_match::FeatureFlags;
use crate::paths::LauncherPaths;
use crate::version_detail::{self, VersionDetail};

/// Which kind of account is launching. `Microsoft` is the shape a future
/// Microsoft-auth milestone will populate with a real Xbox-issued token —
/// nothing in this file needs to change when that lands, since every
/// downstream JVM argument is identical either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Offline,
    Microsoft,
}

impl AccountType {
    fn as_user_type_arg(&self) -> &'static str {
        match self {
            AccountType::Offline => "legacy",
            AccountType::Microsoft => "msa",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameAccount {
    pub username: String,
    pub uuid: Uuid,
    pub access_token: String,
    pub account_type: AccountType,
}

impl GameAccount {
    /// Builds an offline-mode account. The UUID is derived from the
    /// username with the exact same `MD5("OfflinePlayer:" + name)` recipe
    /// the vanilla client itself uses for offline play (Java's
    /// `UUID.nameUUIDFromBytes`), so the same name always maps to the same
    /// UUID — including across other launchers.
    pub fn offline(username: impl Into<String>) -> Self {
        let username = username.into();
        Self {
            uuid: offline_uuid_for_username(&username),
            username,
            access_token: "0".repeat(32),
            account_type: AccountType::Offline,
        }
    }
}

fn offline_uuid_for_username(username: &str) -> Uuid {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);

    // RFC 4122 version/variant bits — matches what `UUID.nameUUIDFromBytes`
    // does to a raw MD5 digest.
    bytes[6] = (bytes[6] & 0x0F) | 0x30;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    Uuid::from_bytes(bytes)
}

/// Which mod loader to launch with. Deliberately a standalone enum in this
/// crate rather than reusing `database::Loader` — `minecraft` must not
/// depend on `database` (see `docs/ARCHITECTURE.md`), so `src-tauri` is
/// responsible for converting between the two at the one boundary where
/// both are in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
}

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub instance_id: Uuid,
    pub minecraft_version: String,
    pub loader: Loader,
    /// Required when `loader` is `Fabric`; ignored for `Vanilla`. Validated
    /// at the start of `launch()` rather than at construction time, so the
    /// error surfaces through the same `LaunchEvent`/`Failed` reporting
    /// path as every other launch failure instead of needing its own
    /// special case at every call site.
    pub loader_version: Option<String>,
    pub account: GameAccount,
    /// Overrides auto-detection entirely when set (the per-instance "use a
    /// specific Java executable" setting).
    pub java_override: Option<std::path::PathBuf>,
    pub extra_java_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub memory_min_mb: u32,
    pub memory_max_mb: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
}


/// Progress and lifecycle updates for one launch, from "resolving the
/// version" all the way through the game process exiting. Tauri commands
/// forward these to the frontend largely as-is.
#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Stage(LaunchStage),
    Download(DownloadEvent),
    /// One line of the game's stdout/stderr, already log-level-tagged by
    /// `is_stderr` (Minecraft's own logger writes plenty of legitimate
    /// non-error output to stderr, so this is *not* the same as "is an
    /// error").
    ProcessOutput { line: String, is_stderr: bool },
    Exited { exit_code: Option<i32>, was_stopped_by_user: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStage {
    ResolvingVersion,
    DownloadingFiles,
    InstallingJava,
    ExtractingNatives,
    Starting,
}

/// A handle to a running game process. Dropping this does **not** kill the
/// game — call [`LaunchHandle::request_stop`] explicitly, the same as
/// closing the official launcher doesn't close the game either.
pub struct LaunchHandle {
    stop_signal: Arc<Notify>,
    stop_requested: Arc<AtomicBool>,
    pub pid: Option<u32>,
    pub started_at: Instant,
}

impl LaunchHandle {
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
        self.stop_signal.notify_waiters();
    }

    pub fn elapsed_seconds(&self) -> i64 {
        self.started_at.elapsed().as_secs() as i64
    }
}

/// Resolves the requested version, downloads anything missing (client jar,
/// libraries, assets, a Java runtime if no suitable one is installed),
/// extracts natives, then spawns the game. Returns as soon as the process
/// has started — `events` keeps reporting progress, log lines, and
/// eventually the exit code asynchronously.
pub async fn launch(
    request: LaunchRequest,
    paths: &LauncherPaths,
    http: &reqwest::Client,
    download_manager: &DownloadManager,
    events: UnboundedSender<LaunchEvent>,
) -> Result<LaunchHandle, MinecraftError> {
    let _ = events.send(LaunchEvent::Stage(LaunchStage::ResolvingVersion));
    let vanilla_version = load_or_fetch_version_detail(http, paths, &request.minecraft_version).await?;

    // The client jar is always the unmodified vanilla one, cached under the
    // plain Minecraft version id regardless of loader, so every Fabric
    // instance targeting the same Minecraft version shares one copy with
    // each other and with any vanilla instance on that version — see the
    // shared-cache design in `docs/ARCHITECTURE.md`.
    let jar_cache_id = vanilla_version.id.clone();

    let version = match request.loader {
        Loader::Vanilla => vanilla_version,
        Loader::Fabric => {
            let loader_version = request
                .loader_version
                .as_deref()
                .ok_or(MinecraftError::MissingLoaderVersion)?;
            crate::fabric::load_or_fetch_fabric_version_detail(
                http,
                paths,
                &vanilla_version,
                &request.minecraft_version,
                loader_version,
            )
            .await?
        }
    };

    let _ = events.send(LaunchEvent::Stage(LaunchStage::DownloadingFiles));
    let resolved_libraries = libraries::resolve_libraries(&version.libraries, &paths.libraries_dir());
    let asset_index = load_or_fetch_asset_index(http, paths, &version).await?;

    let mut tasks: Vec<DownloadTask> = Vec::new();
    tasks.push(
        DownloadTask::new(
            version.downloads.client.url.clone(),
            paths.version_jar_path(&jar_cache_id),
            format!("{jar_cache_id} client"),
        )
        .with_sha1(version.downloads.client.sha1.clone())
        .with_size(version.downloads.client.size),
    );
    tasks.extend(resolved_libraries.classpath_tasks.clone());
    tasks.extend(resolved_libraries.native_jars.iter().map(|n| n.task.clone()));
    tasks.extend(assets::build_asset_download_tasks(&asset_index, &paths.assets_dir()));

    let logging_argument = if let Some(logging) = &version.logging {
        let destination = paths.assets_dir().join("log_configs").join(&logging.client.file.id);
        tasks.push(
            DownloadTask::new(
                logging.client.file.url.clone(),
                destination.clone(),
                format!("logging config: {}", logging.client.file.id),
            )
            .with_sha1(logging.client.file.sha1.clone())
            .with_size(logging.client.file.size),
        );
        Some((logging.client.argument.clone(), destination))
    } else {
        None
    };

    let controller = DownloadController::new();
    let forwarding_events = events.clone();
    let (download_tx, mut download_rx) = tokio::sync::mpsc::unbounded_channel();
    let forward_handle = tokio::spawn(async move {
        while let Some(event) = download_rx.recv().await {
            let _ = forwarding_events.send(LaunchEvent::Download(event));
        }
    });
    download_manager.run(tasks, controller, download_tx).await?;
    forward_handle.abort();

    if assets::needs_legacy_layout(&version.asset_index.id) {
        assets::materialize_legacy_asset_layout(&asset_index, &paths.assets_dir(), &version.asset_index.id).await?;
    }

    let _ = events.send(LaunchEvent::Stage(LaunchStage::ExtractingNatives));
    let natives_dir = paths.instance_natives_dir(request.instance_id, &version.id);
    tokio::fs::create_dir_all(&natives_dir).await.map_err(|source| MinecraftError::Io {
        context: format!("creating natives directory {}", natives_dir.display()),
        source,
    })?;
    for native_jar in &resolved_libraries.native_jars {
        libraries::extract_native_jar(native_jar.destination.clone(), natives_dir.clone(), native_jar.exclude.clone())
            .await?;
    }

    let _ = events.send(LaunchEvent::Stage(LaunchStage::InstallingJava));
    let java_path = resolve_java(http, download_manager, paths, &version, request.java_override.clone(), events.clone())
        .await?;

    let game_dir = paths.instance_game_dir(request.instance_id);
    tokio::fs::create_dir_all(&game_dir).await.map_err(|source| MinecraftError::Io {
        context: format!("creating instance game directory {}", game_dir.display()),
        source,
    })?;
    if request.fullscreen {
        apply_fullscreen_preference(&game_dir).await;
    }

    let mut classpath_entries = resolved_libraries.classpath_entries.clone();
    classpath_entries.push(paths.version_jar_path(&jar_cache_id));
    let classpath = std::env::join_paths(&classpath_entries)
        .map_err(|err| MinecraftError::Io {
            context: "joining classpath entries".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, err),
        })?
        .to_string_lossy()
        .to_string();

    let placeholders = build_placeholders(&request, &version, paths, &classpath, &natives_dir, &paths.assets_dir());

    let mut features = FeatureFlags::new();
    features.set("has_custom_resolution", true);

    let mut jvm_args = request.extra_java_args.clone();
    jvm_args.push(format!("-Xms{}M", request.memory_min_mb));
    jvm_args.push(format!("-Xmx{}M", request.memory_max_mb));

    if let Some((argument_template, log_config_path)) = &logging_argument {
        let mut logging_placeholders = HashMap::new();
        logging_placeholders.insert("path", log_config_path.to_string_lossy().to_string());
        jvm_args.push(substitute_one(argument_template, &logging_placeholders));
    }

    match &version.arguments {
        Some(arguments) => {
            jvm_args.extend(version_detail::resolve_arguments(&arguments.jvm, &features, &placeholders));
        }
        None => {
            // Pre-1.13 versions have no structured `jvm` argument array —
            // this is the fixed minimum every such version needs.
            jvm_args.push(format!("-Djava.library.path={}", natives_dir.display()));
            jvm_args.push("-cp".to_string());
            jvm_args.push(classpath.clone());
        }
    }

    let mut game_args = match &version.arguments {
        Some(arguments) => version_detail::resolve_arguments(&arguments.game, &features, &placeholders),
        None => version_detail::resolve_legacy_game_arguments(
            version.legacy_minecraft_arguments.as_deref().unwrap_or_default(),
            &placeholders,
        ),
    };
    game_args.extend(request.extra_game_args.clone());

    let _ = events.send(LaunchEvent::Stage(LaunchStage::Starting));

    let mut command = Command::new(&java_path);
    command
        .args(&jvm_args)
        .arg(&version.main_class)
        .args(&game_args)
        .current_dir(&game_dir)
        .kill_on_drop(false)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(MinecraftError::LaunchFailed)?;
    let pid = child.id();

    if let Some(stdout) = child.stdout.take() {
        spawn_log_forwarder(stdout, false, events.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_forwarder(stderr, true, events.clone());
    }

    let stop_signal = Arc::new(Notify::new());
    let stop_requested = Arc::new(AtomicBool::new(false));
    let monitor_stop_signal = stop_signal.clone();
    let monitor_stop_requested = stop_requested.clone();
    let monitor_events = events.clone();

    tokio::spawn(async move {
        let exit_code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()),
            _ = monitor_stop_signal.notified() => {
                let _ = child.kill().await;
                None
            }
        };
        let _ = monitor_events.send(LaunchEvent::Exited {
            exit_code,
            was_stopped_by_user: monitor_stop_requested.load(Ordering::Relaxed),
        });
    });

    Ok(LaunchHandle {
        stop_signal,
        stop_requested,
        pid,
        started_at: Instant::now(),
    })
}

fn spawn_log_forwarder<R>(reader: R, is_stderr: bool, events: UnboundedSender<LaunchEvent>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let _ = events.send(LaunchEvent::ProcessOutput { line, is_stderr });
                }
                _ => break,
            }
        }
    });
}

async fn load_or_fetch_version_detail(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    version_id: &str,
) -> Result<VersionDetail, MinecraftError> {
    let cache_path = paths.version_json_path(version_id);

    if let Ok(bytes) = tokio::fs::read(&cache_path).await {
        if let Ok(detail) = manifest::parse_version_detail(&bytes, "cache") {
            return Ok(detail);
        }
    }

    let manifest_doc = manifest::fetch_version_manifest(client).await?;
    let summary = manifest_doc
        .find(version_id)
        .ok_or_else(|| MinecraftError::VersionNotFound(version_id.to_string()))?;

    let bytes = manifest::fetch_version_detail_bytes(client, &summary.url).await?;

    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&cache_path, &bytes).await;

    manifest::parse_version_detail(&bytes, &summary.url)
}

async fn load_or_fetch_asset_index(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    version: &VersionDetail,
) -> Result<assets::AssetIndex, MinecraftError> {
    let cache_path = paths.asset_index_path(&version.asset_index.id);

    if let Ok(bytes) = tokio::fs::read(&cache_path).await {
        if let Ok(index) = assets::parse_asset_index(&bytes, "cache") {
            return Ok(index);
        }
    }

    let bytes = assets::fetch_asset_index_bytes(client, &version.asset_index.url).await?;
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&cache_path, &bytes).await;

    assets::parse_asset_index(&bytes, &version.asset_index.id)
}

async fn resolve_java(
    client: &reqwest::Client,
    download_manager: &DownloadManager,
    paths: &LauncherPaths,
    version: &VersionDetail,
    java_override: Option<std::path::PathBuf>,
    events: UnboundedSender<LaunchEvent>,
) -> Result<std::path::PathBuf, MinecraftError> {
    if let Some(path) = java_override {
        return Ok(path);
    }

    let required_major = version.java_version.as_ref().map(|v| v.major_version);

    if let Some(system_java) = java::find_system_java().await {
        let satisfies_requirement = match required_major {
            Some(required) => java::detect_java_major_version(&system_java)
                .await
                .map(|detected| detected >= required)
                .unwrap_or(false),
            None => true,
        };
        if satisfies_requirement {
            return Ok(system_java);
        }
    }

    let component = version
        .java_version
        .as_ref()
        .map(|v| v.component.as_str())
        .unwrap_or("jre-legacy");

    let (download_tx, mut download_rx) = tokio::sync::mpsc::unbounded_channel();
    let forwarding_events = events.clone();
    let forward_handle = tokio::spawn(async move {
        while let Some(event) = download_rx.recv().await {
            let _ = forwarding_events.send(LaunchEvent::Download(event));
        }
    });

    let result = java::ensure_managed_runtime(client, download_manager, &paths.java_runtimes_dir(), component, download_tx).await;
    forward_handle.abort();
    result
}

/// Sets `fullscreen:true` in the instance's `options.txt` if not already
/// present. We only ever *set* this, never force it back to `false` — a
/// player who alt-tabs out of fullscreen in-game shouldn't have that
/// choice silently reverted on their next launch.
async fn apply_fullscreen_preference(game_dir: &Path) {
    let options_path = game_dir.join("options.txt");
    let existing = tokio::fs::read_to_string(&options_path).await.unwrap_or_default();

    if existing.lines().any(|line| line.trim() == "fullscreen:true") {
        return;
    }

    let mut lines: Vec<String> = existing
        .lines()
        .filter(|line| !line.starts_with("fullscreen:"))
        .map(|line| line.to_string())
        .collect();
    lines.push("fullscreen:true".to_string());

    let _ = tokio::fs::write(&options_path, lines.join("\n") + "\n").await;
}

fn build_placeholders(
    request: &LaunchRequest,
    version: &VersionDetail,
    paths: &LauncherPaths,
    classpath: &str,
    natives_dir: &Path,
    assets_dir: &Path,
) -> HashMap<&'static str, String> {
    let mut map = HashMap::new();
    map.insert("auth_player_name", request.account.username.clone());
    map.insert("version_name", version.id.clone());
    map.insert(
        "game_directory",
        paths.instance_game_dir(request.instance_id).to_string_lossy().to_string(),
    );
    map.insert("assets_root", assets_dir.to_string_lossy().to_string());
    map.insert("game_assets", assets_dir.to_string_lossy().to_string());
    map.insert("assets_index_name", version.asset_index.id.clone());
    map.insert("auth_uuid", request.account.uuid.simple().to_string());
    map.insert("auth_access_token", request.account.access_token.clone());
    map.insert("auth_session", request.account.access_token.clone());
    map.insert("user_type", request.account.account_type.as_user_type_arg().to_string());
    map.insert("user_properties", "{}".to_string());
    map.insert("version_type", version.version_type.clone());
    map.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    map.insert("launcher_name", "launcher".to_string());
    map.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    map.insert("classpath", classpath.to_string());
    map.insert("resolution_width", request.window_width.to_string());
    map.insert("resolution_height", request.window_height.to_string());
    map.insert("clientid", request.instance_id.simple().to_string());
    map.insert("auth_xuid", "0".to_string());
    map
}

fn substitute_one(template: &str, placeholders: &HashMap<&str, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in placeholders {
        result = result.replace(&format!("${{{key}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_deterministic_for_the_same_name() {
        let a = offline_uuid_for_username("Notch");
        let b = offline_uuid_for_username("Notch");
        assert_eq!(a, b);
    }

    #[test]
    fn offline_uuid_differs_between_names() {
        assert_ne!(offline_uuid_for_username("Notch"), offline_uuid_for_username("Jeb_"));
    }

    #[test]
    fn offline_uuid_has_correct_version_and_variant_bits() {
        let uuid = offline_uuid_for_username("Steve");
        let bytes = uuid.as_bytes();
        assert_eq!(bytes[6] & 0xF0, 0x30, "version nibble should be 3");
        assert_eq!(bytes[8] & 0xC0, 0x80, "variant bits should be RFC 4122");
    }

    #[test]
    fn substitute_one_replaces_template_token() {
        let mut placeholders = HashMap::new();
        placeholders.insert("path", "/tmp/log4j.xml".to_string());
        assert_eq!(
            substitute_one("-Dlog4j.configurationFile=${path}", &placeholders),
            "-Dlog4j.configurationFile=/tmp/log4j.xml"
        );
    }
}
