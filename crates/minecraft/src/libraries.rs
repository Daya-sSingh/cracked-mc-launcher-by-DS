use std::path::{Path, PathBuf};

use downloads::DownloadTask;

use crate::error::MinecraftError;
use crate::os_match::{current_arch, current_os_name, rules_allow, FeatureFlags};
use crate::version_detail::Library;

/// Everything needed to launch: which jars belong on the classpath, the
/// downloads required to get them there, and which native-library jars
/// need to be unpacked into the instance's natives directory before the
/// JVM starts (LWJGL et al. load these via `java.library.path`, not the
/// classpath).
#[derive(Debug, Default)]
pub struct ResolvedLibraries {
    pub classpath_tasks: Vec<DownloadTask>,
    pub classpath_entries: Vec<PathBuf>,
    pub native_jars: Vec<NativeJar>,
}

#[derive(Debug, Clone)]
pub struct NativeJar {
    pub task: DownloadTask,
    pub destination: PathBuf,
    pub exclude: Vec<String>,
}

/// Filters `libraries` down to the ones that apply on this OS/arch, and
/// builds the download tasks plus final classpath order. Order matters for
/// the classpath (first-listed wins on symbol conflicts), so this preserves
/// the manifest's own library order rather than e.g. sorting alphabetically.
pub fn resolve_libraries(libraries: &[Library], libraries_dir: &Path) -> ResolvedLibraries {
    let mut resolved = ResolvedLibraries::default();
    let features = FeatureFlags::new();

    for library in libraries {
        if !rules_allow(&library.rules, &features) {
            continue;
        }

        match resolve_main_artifact(library, libraries_dir) {
            Some((destination, task)) => {
                resolved.classpath_entries.push(destination);
                resolved.classpath_tasks.push(task);
            }
            None => {
                tracing::warn!(
                    library = %library.name,
                    "skipping library with no resolvable download (no downloads.artifact and a malformed name)"
                );
                continue;
            }
        }

        if let Some(native_jar) = resolve_native_artifact(library, libraries_dir) {
            resolved.native_jars.push(native_jar);
        }
    }

    resolved
}

fn resolve_main_artifact(
    library: &Library,
    libraries_dir: &Path,
) -> Option<(PathBuf, DownloadTask)> {
    if let Some(artifact) = library.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        let destination = libraries_dir.join(&artifact.path);
        let task = DownloadTask::new(
            artifact.url.clone(),
            destination.clone(),
            format!("library: {}", library.name),
        )
        .with_sha1(artifact.sha1.clone())
        .with_size(artifact.size);
        return Some((destination, task));
    }

    // No `downloads` block — this is how Fabric (and some other
    // third-party meta) describes libraries: a bare Maven coordinate plus a
    // repository base URL, with no pre-computed hash.
    let path = maven_coordinate_to_path(&library.name).ok()?;
    let base = library
        .url
        .clone()
        .unwrap_or_else(|| "https://libraries.minecraft.net/".to_string());
    let base = if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    };
    let destination = libraries_dir.join(&path);
    let url = format!("{base}{path}");
    let task = DownloadTask::new(
        url,
        destination.clone(),
        format!("library: {}", library.name),
    );
    Some((destination, task))
}

/// Resolves the **legacy** (pre-1.19) native-library convention: a single
/// library entry carrying both a `natives` map (OS name → classifier) and a
/// `downloads.classifiers` map (classifier → artifact), extracted to a
/// directory that gets passed as `-Djava.library.path`.
///
/// Minecraft 1.19+ dropped this convention — each platform's native LWJGL
/// variant is instead its own top-level library entry with the classifier
/// baked directly into `name` (e.g. `org.lwjgl:lwjgl-glfw:3.3.3:natives-windows`)
/// and a `rules` array restricting it to that OS, with no `natives` field
/// at all. Those entries fall through to [`resolve_main_artifact`] like any
/// other library — which is correct, not a gap: LWJGL 3.x bundles its
/// native binary inside that same jar and self-extracts it at runtime via
/// its own `SharedLibraryLoader` as long as the jar is on the classpath, so
/// no separate extraction step is needed for modern versions. This function
/// (and the `-Djava.library.path` it feeds) only matters for pre-1.19
/// instances still using LWJGL 2's older loading model.
fn resolve_native_artifact(library: &Library, libraries_dir: &Path) -> Option<NativeJar> {
    let natives_map = library.natives.as_ref()?;
    let downloads = library.downloads.as_ref()?;

    let raw_classifier = natives_map.get(current_os_name())?;
    // A handful of old LWJGL2 entries parameterize the classifier by
    // architecture, e.g. `"natives-windows-${arch}"`.
    let classifier = raw_classifier.replace("${arch}", arch_bits());

    let artifact = downloads.classifiers.get(&classifier)?;
    let destination = libraries_dir.join(&artifact.path);
    let exclude = library
        .extract
        .as_ref()
        .map(|e| e.exclude.clone())
        .unwrap_or_default();

    let task = DownloadTask::new(
        artifact.url.clone(),
        destination.clone(),
        format!("natives: {}", library.name),
    )
    .with_sha1(artifact.sha1.clone())
    .with_size(artifact.size);

    Some(NativeJar {
        task,
        destination,
        exclude,
    })
}

fn arch_bits() -> &'static str {
    if current_arch().contains("64") {
        "64"
    } else {
        "32"
    }
}

/// Converts a Maven coordinate (`group:artifact:version` or
/// `group:artifact:version:classifier`) into the relative path Maven
/// repositories serve it at, e.g.
/// `net.fabricmc:fabric-loader:0.16.9` →
/// `net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar`.
pub fn maven_coordinate_to_path(coordinate: &str) -> Result<String, MinecraftError> {
    let parts: Vec<&str> = coordinate.split(':').collect();
    if parts.len() < 3 {
        return Err(MinecraftError::MalformedLibraryName(coordinate.to_string()));
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3);

    let filename = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };

    Ok(format!("{group}/{artifact}/{version}/{filename}"))
}

/// Collapses a library list down to one entry per **platform-specific**
/// Maven artifact — see [`maven_group_artifact_key`] for exactly what
/// counts as "the same" artifact — keeping whichever occurrence comes
/// **last** in the input and discarding earlier ones that key the same.
/// Order of the surviving entries matches their first appearance in the
/// input.
///
/// This exists for merging a loader's library list on top of vanilla's: if
/// both specify a dependency on the same artifact but pin different
/// versions (e.g. a newer ASM required by Fabric than whatever vanilla
/// might reference), the loader's copy — appended after vanilla's — is the
/// one actually meant to end up on the classpath. Without this, both jars
/// would load and which one wins would depend on classpath ordering, an
/// easy source of a launch that fails only sometimes.
///
/// Getting the key wrong in the other direction is just as real a bug:
/// modern Minecraft (1.19+) lists each platform's native LWJGL variant as
/// its own library entry with the classifier baked into the coordinate
/// (`org.lwjgl:lwjgl-glfw:3.3.3:natives-windows`,
/// `...:natives-linux`, `...:natives-macos`, ...). An earlier version of
/// this function's key derivation discarded the classifier along with the
/// version, so every platform variant of a given LWJGL module collapsed
/// into whichever one happened to appear last in the list — silently
/// dropping every other platform's native library from the classpath,
/// including whichever one the current OS actually needed.
pub fn dedupe_libraries_preferring_last(libraries: Vec<Library>) -> Vec<Library> {
    use std::collections::HashMap;

    let mut first_seen_order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, Library> = HashMap::new();

    for library in libraries {
        let key = maven_group_artifact_key(&library.name);
        if !by_key.contains_key(&key) {
            first_seen_order.push(key.clone());
        }
        by_key.insert(key, library);
    }

    first_seen_order
        .into_iter()
        .filter_map(|key| by_key.remove(&key))
        .collect()
}

/// Extracts the part of a Maven coordinate that identifies "the same
/// library for the same platform", deliberately ignoring only the version.
///
/// This distinction matters because modern Minecraft (1.19+) gives each
/// platform's native library its own top-level entry, with the classifier
/// baked directly into the coordinate — e.g. a single vanilla version can
/// list `org.lwjgl:lwjgl-glfw:3.3.3:natives-windows`,
/// `org.lwjgl:lwjgl-glfw:3.3.3:natives-linux`, and
/// `org.lwjgl:lwjgl-glfw:3.3.3:natives-macos` all as distinct libraries.
/// Those must never collapse into each other — they aren't alternate
/// versions of the same thing, they're different files needed on different
/// platforms, and every one of them (well, whichever match the current OS
/// per that library's `rules`) needs to survive onto the classpath.
///
/// A coordinate with no classifier (`group:artifact:version`) keys on just
/// `group:artifact`, which *is* meant to dedupe across versions — that's
/// what allows Fabric to pin a newer version of a shared dependency (ASM,
/// for instance) than vanilla without both ending up on the classpath at
/// once.
fn maven_group_artifact_key(coordinate: &str) -> String {
    let parts: Vec<&str> = coordinate.split(':').collect();
    match parts.as_slice() {
        [group, artifact, _version, classifier] => format!("{group}:{artifact}:{classifier}"),
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => coordinate.to_string(),
    }
}

/// Unpacks a downloaded natives jar into the instance's natives directory,
/// skipping any path that starts with one of `exclude`'s prefixes (almost
/// always just `META-INF/`). Runs on a blocking thread since the `zip`
/// crate's reader is synchronous and these archives are small enough that
/// the blocking I/O is brief.
pub async fn extract_native_jar(
    jar_path: PathBuf,
    natives_dir: PathBuf,
    exclude: Vec<String>,
) -> Result<(), MinecraftError> {
    tokio::task::spawn_blocking(move || {
        extract_native_jar_blocking(&jar_path, &natives_dir, &exclude)
    })
    .await
    .map_err(|join_err| MinecraftError::NativeExtraction(join_err.to_string()))?
}

fn extract_native_jar_blocking(
    jar_path: &Path,
    natives_dir: &Path,
    exclude: &[String],
) -> Result<(), MinecraftError> {
    let file = std::fs::File::open(jar_path).map_err(|source| MinecraftError::Io {
        context: format!("opening native archive {}", jar_path.display()),
        source,
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|err| {
        MinecraftError::NativeExtraction(format!("{}: {err}", jar_path.display()))
    })?;

    std::fs::create_dir_all(natives_dir).map_err(|source| MinecraftError::Io {
        context: format!("creating natives directory {}", natives_dir.display()),
        source,
    })?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| MinecraftError::NativeExtraction(err.to_string()))?;

        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_string();
        if exclude
            .iter()
            .any(|prefix| name.starts_with(prefix.as_str()))
        {
            continue;
        }

        let destination = natives_dir.join(&name);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|source| MinecraftError::Io {
                context: format!("creating directory for {}", destination.display()),
                source,
            })?;
        }

        let mut out_file =
            std::fs::File::create(&destination).map_err(|source| MinecraftError::Io {
                context: format!("writing native file {}", destination.display()),
                source,
            })?;
        std::io::copy(&mut entry, &mut out_file).map_err(|source| MinecraftError::Io {
            context: format!("writing native file {}", destination.display()),
            source,
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_coordinate_without_classifier() {
        let path = maven_coordinate_to_path("net.fabricmc:fabric-loader:0.16.9").unwrap();
        assert_eq!(
            path,
            "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
    }

    #[test]
    fn maven_coordinate_with_classifier() {
        let path = maven_coordinate_to_path("org.lwjgl:lwjgl:3.3.3:natives-linux").unwrap();
        assert_eq!(path, "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar");
    }

    #[test]
    fn malformed_coordinate_is_rejected() {
        assert!(maven_coordinate_to_path("not-a-valid-coordinate").is_err());
    }

    #[test]
    fn arch_bits_is_32_or_64() {
        assert!(arch_bits() == "32" || arch_bits() == "64");
    }

    /// Minimal `Library` builder for tests that only care about `name` —
    /// every other field is optional/defaulted in the real struct, but Rust
    /// still requires them to be spelled out since `Library` has no
    /// `Default` derive (its `Deserialize` impl relies on `#[serde(default)]`
    /// per-field instead, which doesn't produce a `Default::default()`).
    fn lib(name: &str) -> Library {
        Library {
            name: name.to_string(),
            downloads: None,
            rules: vec![],
            natives: None,
            url: None,
            extract: None,
        }
    }

    #[test]
    fn maven_group_artifact_key_ignores_version_but_keeps_group_and_artifact_distinct() {
        assert_eq!(
            maven_group_artifact_key("org.ow2.asm:asm:9.6"),
            maven_group_artifact_key("org.ow2.asm:asm:9.7"),
            "same group:artifact at different versions should collapse to one key",
        );
        assert_ne!(
            maven_group_artifact_key("org.ow2.asm:asm:9.6"),
            maven_group_artifact_key("org.ow2.asm:asm-commons:9.6"),
            "different artifacts must never share a key",
        );
    }

    #[test]
    fn maven_group_artifact_key_keeps_different_platform_classifiers_distinct() {
        // This is the exact real-world shape that exposed the original bug:
        // modern Minecraft (1.19+) lists one library entry per platform for
        // every native LWJGL module, with the classifier baked into the
        // coordinate rather than expressed via a `natives` map. These must
        // never collapse into each other — each is a genuinely different
        // file needed on a genuinely different platform.
        let windows = maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows");
        let linux = maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.3:natives-linux");
        let macos = maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.3:natives-macos");
        let macos_arm64 =
            maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.3:natives-macos-arm64");

        assert_ne!(windows, linux);
        assert_ne!(windows, macos);
        assert_ne!(linux, macos);
        assert_ne!(
            macos, macos_arm64,
            "arm64 variant must be distinct from the base macos one"
        );
    }

    #[test]
    fn maven_group_artifact_key_still_dedupes_same_classifier_across_versions() {
        // If the *same* platform variant appears twice at different
        // versions (not a real Mojang scenario today, but a loader could
        // in principle pin a newer natives build), it should still dedupe
        // like the no-classifier case — only the version differs.
        assert_eq!(
            maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows"),
            maven_group_artifact_key("org.lwjgl:lwjgl-glfw:3.3.4:natives-windows"),
        );
    }

    #[test]
    fn dedupe_keeps_last_occurrence_of_duplicate_coordinates() {
        let libraries = vec![
            lib("org.ow2.asm:asm:9.1"), // vanilla's copy — older
            lib("net.fabricmc:fabric-loader:0.16.9"),
            lib("org.ow2.asm:asm:9.7"), // fabric's copy — newer, should win
        ];

        let result = dedupe_libraries_preferring_last(libraries);

        assert_eq!(
            result.len(),
            2,
            "the two asm entries should collapse into one"
        );
        let asm = result
            .iter()
            .find(|l| l.name.starts_with("org.ow2.asm:asm:"))
            .unwrap();
        assert_eq!(
            asm.name, "org.ow2.asm:asm:9.7",
            "the later (Fabric) version should win"
        );
    }

    #[test]
    fn dedupe_preserves_every_platform_variant_of_a_native_library() {
        // Direct regression test for the real bug: a realistic slice of
        // what a modern vanilla version.json actually lists for one LWJGL
        // module — six platform-specific entries, all sharing the same
        // group, artifact, and version, distinguished only by classifier.
        // Every single one must survive the merge; losing even one means a
        // player on that platform gets a NoClassDefFoundError at launch.
        let libraries = vec![
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows"),
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows-x86"),
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows-arm64"),
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-linux"),
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-macos"),
            lib("org.lwjgl:lwjgl-glfw:3.3.3:natives-macos-arm64"),
        ];

        let result = dedupe_libraries_preferring_last(libraries.clone());

        assert_eq!(
            result.len(),
            libraries.len(),
            "every platform variant must survive — none of these are duplicates of each other"
        );
        for original in &libraries {
            assert!(
                result.iter().any(|l| l.name == original.name),
                "{} was dropped by dedup",
                original.name
            );
        }
    }

    #[test]
    fn dedupe_preserves_first_seen_order_for_survivors() {
        let libraries = vec![lib("a:a:1"), lib("b:b:1"), lib("a:a:2")];
        let result = dedupe_libraries_preferring_last(libraries);
        // "a:a" was first seen at index 0, so it should still occupy that
        // slot in the output even though its surviving value came from the
        // later entry.
        assert_eq!(result[0].name, "a:a:2");
        assert_eq!(result[1].name, "b:b:1");
    }

    #[test]
    fn dedupe_with_no_duplicates_is_a_no_op() {
        let libraries = vec![lib("a:a:1"), lib("b:b:1"), lib("c:c:1")];
        let result = dedupe_libraries_preferring_last(libraries);
        assert_eq!(result.len(), 3);
    }
}
