use std::path::PathBuf;

use uuid::Uuid;

/// Resolves every path the launcher cares about from a single app-data
/// root. Two things are deliberately **shared across every instance**
/// rather than duplicated: the libraries/assets cache (content-addressed,
/// so sharing is free) and Java runtimes (multi-hundred-MB each — comically
/// wasteful to duplicate per instance). Everything that must stay isolated
/// per the spec — mods, saves, resource packs, configs, logs — lives under
/// that instance's own directory.
///
/// ```text
/// <root>/
///   launcher.db
///   cache/
///     versions/<id>/<id>.jar         (client jars, shared — always keyed by
///                                     the vanilla version id, even for a
///                                     Fabric instance, since the jar itself
///                                     is unmodified vanilla)
///     libraries/...                  (shared, content-addressed by Maven path)
///     assets/objects/<hh>/<hash>     (shared, content-addressed by SHA-1)
///     java/<component>/<platform>/...(managed JRE builds, shared)
///     fabric/<game-version>/<loader-version>.json
///                                     (cached Fabric profile responses,
///                                     shared across every instance using
///                                     that exact game+loader combination)
///   instances/<instance-id>/
///     .minecraft/                   (game_dir: saves, mods, config, logs, screenshots, options.txt)
///     natives/<version-id>/         (unpacked per launch, cheap to duplicate)
/// ```
#[derive(Debug, Clone)]
pub struct LauncherPaths {
    root: PathBuf,
}

impl LauncherPaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn database_file(&self) -> PathBuf {
        self.root.join("launcher.db")
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.cache_dir().join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.cache_dir().join("assets")
    }

    pub fn asset_index_path(&self, asset_index_id: &str) -> PathBuf {
        self.assets_dir().join("indexes").join(format!("{asset_index_id}.json"))
    }

    pub fn java_runtimes_dir(&self) -> PathBuf {
        self.cache_dir().join("java")
    }

    fn versions_dir(&self) -> PathBuf {
        self.cache_dir().join("versions")
    }

    pub fn version_json_path(&self, version_id: &str) -> PathBuf {
        self.versions_dir().join(version_id).join(format!("{version_id}.json"))
    }

    pub fn version_jar_path(&self, version_id: &str) -> PathBuf {
        self.versions_dir().join(version_id).join(format!("{version_id}.jar"))
    }

    /// Where a Fabric profile response (`.../loader/<game>/<loader>/profile/json`)
    /// is cached. Keyed by both the game version and the loader build since
    /// each combination produces a distinct library list and main class.
    /// These responses are immutable for a given (game, loader) pair once
    /// published, so — like `version_json_path` — a cached copy never needs
    /// to expire.
    pub fn fabric_profile_path(&self, game_version: &str, loader_version: &str) -> PathBuf {
        self.cache_dir()
            .join("fabric")
            .join(sanitize_path_segment(game_version))
            .join(format!("{}.json", sanitize_path_segment(loader_version)))
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, instance_id: Uuid) -> PathBuf {
        self.instances_dir().join(instance_id.to_string())
    }

    /// The directory passed to the JVM as `${game_directory}` — what a
    /// player would recognize as "their .minecraft folder" for this
    /// instance specifically.
    pub fn instance_game_dir(&self, instance_id: Uuid) -> PathBuf {
        self.instance_dir(instance_id).join("minecraft")
    }

    pub fn instance_natives_dir(&self, instance_id: Uuid, version_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("natives").join(version_id)
    }

    pub fn instance_logs_dir(&self, instance_id: Uuid) -> PathBuf {
        self.instance_game_dir(instance_id).join("logs")
    }
}

/// Turns a version string into something safe to use as a single path
/// segment on every supported OS. Windows in particular rejects
/// `<>:"/\|?*` in file/directory names — normal Minecraft version ids never
/// contain these, but Fabric's own API documentation gives version strings
/// like `"1.14 Pre-Release 5"` as a real example, so this defends against
/// any future id shape turning into a confusing "path not found" error deep
/// inside the download manager instead of a clear one here.
fn sanitize_path_segment(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_paths_are_isolated_per_instance() {
        let paths = LauncherPaths::new(PathBuf::from("/data"));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_ne!(paths.instance_game_dir(a), paths.instance_game_dir(b));
    }

    #[test]
    fn cache_paths_do_not_depend_on_instance_id() {
        let paths = LauncherPaths::new(PathBuf::from("/data"));
        assert_eq!(paths.libraries_dir(), PathBuf::from("/data/cache/libraries"));
        assert_eq!(paths.assets_dir(), PathBuf::from("/data/cache/assets"));
    }

    #[test]
    fn fabric_profile_path_is_keyed_by_both_game_and_loader_version() {
        let paths = LauncherPaths::new(PathBuf::from("/data"));
        let a = paths.fabric_profile_path("1.21.11", "0.16.9");
        let b = paths.fabric_profile_path("1.21.11", "0.16.10");
        let c = paths.fabric_profile_path("1.20.6", "0.16.9");
        assert_ne!(a, b, "different loader versions must not collide");
        assert_ne!(a, c, "different game versions must not collide");
        assert_eq!(a, PathBuf::from("/data/cache/fabric/1.21.11/0.16.9.json"));
    }

    #[test]
    fn sanitize_path_segment_strips_filesystem_unsafe_characters() {
        assert_eq!(sanitize_path_segment("1.21.11"), "1.21.11");
        assert_eq!(sanitize_path_segment("1.14 Pre-Release 5"), "1.14 Pre-Release 5");
        assert_eq!(sanitize_path_segment("weird:name*here"), "weird_name_here");
    }
}
