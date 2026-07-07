//! Fabric Loader support: querying which loader builds are compatible with
//! a given Minecraft version, and merging Fabric's loader profile on top of
//! the vanilla version it targets.
//!
//! Fabric's own `.../profile/json` endpoint does **not** return a
//! self-contained version JSON — it returns a *delta*: a main class
//! override, an extra list of libraries (the loader itself, intermediary
//! mappings, ASM, Mixin, etc.), and optionally some extra JVM/game
//! arguments, all meant to be layered on top of the vanilla version named
//! in its `inheritsFrom` field. [`resolve_fabric_version_detail`] performs
//! that merge and hands back an ordinary [`VersionDetail`] — from
//! `crate::launch::launch`'s point of view, a Fabric instance is just a
//! vanilla one with a longer library list and a different main class.

use serde::{Deserialize, Serialize};

use crate::error::MinecraftError;
use crate::libraries::dedupe_libraries_preferring_last;
use crate::paths::LauncherPaths;
use crate::version_detail::{Arguments, Library, VersionDetail};

const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2";

// ─── Loader version listing (for the "create instance" picker) ──────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FabricLoaderVersion {
    pub separator: String,
    pub build: u32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FabricIntermediary {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

/// One entry from `.../v2/versions/loader/<game_version>` — a loader build
/// known to be compatible with that Minecraft version, paired with the
/// intermediary mappings build it needs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FabricLoaderForGame {
    pub loader: FabricLoaderVersion,
    pub intermediary: FabricIntermediary,
}

/// Fetches every Fabric Loader build compatible with `game_version`.
/// Fabric's API returns these newest-first, but callers should not depend
/// on that ordering — [`FabricLoaderVersion::stable`] is the authoritative
/// signal for "recommended", not array position.
pub async fn fetch_compatible_loader_versions(
    client: &reqwest::Client,
    game_version: &str,
) -> Result<Vec<FabricLoaderForGame>, MinecraftError> {
    let url = format!(
        "{FABRIC_META_BASE}/versions/loader/{}",
        percent_encode_segment(game_version)
    );
    let response = client.get(&url).send().await?;
    let bytes = response.error_for_status()?.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|source| MinecraftError::Deserialize {
        context: format!("Fabric loader list for Minecraft {game_version}"),
        source,
    })
}

/// Picks the loader version to preselect in the UI: the newest build with
/// `stable: true`, or — if a game version genuinely has no stable build
/// yet, which does happen shortly after a new Minecraft release — simply
/// the first entry in the list, on the assumption the API's own ordering
/// puts the newest build first.
pub fn recommended_loader_version(
    candidates: &[FabricLoaderForGame],
) -> Option<&FabricLoaderForGame> {
    candidates
        .iter()
        .find(|c| c.loader.stable)
        .or_else(|| candidates.first())
}

// ─── Profile fetching + merge ────────────────────────────────────────────────

/// Raw shape of Fabric's `.../profile/json` response — a delta on top of
/// the vanilla version JSON, not a self-contained one. Reuses
/// [`crate::version_detail::Library`] and [`Arguments`] directly since
/// Fabric's library entries (`name` + `url`, no `downloads` block) and
/// argument structure are both already shapes those types support.
#[derive(Debug, Clone, Deserialize)]
struct FabricProfile {
    id: String,
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(default)]
    arguments: Option<Arguments>,
    #[serde(default)]
    libraries: Vec<Library>,
}

async fn fetch_fabric_profile_bytes(
    client: &reqwest::Client,
    game_version: &str,
    loader_version: &str,
) -> Result<Vec<u8>, MinecraftError> {
    let url = format!(
        "{FABRIC_META_BASE}/versions/loader/{}/{}/profile/json",
        percent_encode_segment(game_version),
        percent_encode_segment(loader_version),
    );
    let response = client.get(&url).send().await?;
    Ok(response.error_for_status()?.bytes().await?.to_vec())
}

fn parse_fabric_profile(bytes: &[u8], context: &str) -> Result<FabricProfile, MinecraftError> {
    serde_json::from_slice(bytes).map_err(|source| MinecraftError::Deserialize {
        context: format!("Fabric profile for {context}"),
        source,
    })
}

/// Loads a Fabric profile (from the on-disk cache if present, otherwise
/// fetching and caching it) and merges it on top of `vanilla`, producing a
/// `VersionDetail` ready to hand to [`crate::launch::launch`] exactly like
/// a plain vanilla one.
///
/// `vanilla` must already be the resolved [`VersionDetail`] for the same
/// `game_version` — this function does not fetch it itself, since the
/// caller (`crate::launch::launch`) always needs the vanilla detail
/// regardless of loader (for the client jar, asset index, and Java
/// requirement, none of which Fabric's profile redefines).
pub async fn load_or_fetch_fabric_version_detail(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    vanilla: &VersionDetail,
    game_version: &str,
    loader_version: &str,
) -> Result<VersionDetail, MinecraftError> {
    let cache_path = paths.fabric_profile_path(game_version, loader_version);

    let cached = match tokio::fs::read(&cache_path).await {
        Ok(bytes) => parse_fabric_profile(&bytes, "cache").ok(),
        Err(_) => None,
    };

    let profile = match cached {
        Some(profile) => profile,
        None => {
            let bytes = fetch_fabric_profile_bytes(client, game_version, loader_version).await?;
            if let Some(parent) = cache_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let _ = tokio::fs::write(&cache_path, &bytes).await;
            parse_fabric_profile(
                &bytes,
                &format!("Minecraft {game_version} / Fabric {loader_version}"),
            )?
        }
    };

    Ok(merge_profile_onto_vanilla(vanilla, profile))
}

/// The actual merge: vanilla supplies everything Fabric's profile doesn't
/// redefine (client jar, asset index, Java requirement, logging config);
/// Fabric supplies the id, main class, and an appended, deduplicated
/// library list.
fn merge_profile_onto_vanilla(vanilla: &VersionDetail, profile: FabricProfile) -> VersionDetail {
    let mut libraries = vanilla.libraries.clone();
    libraries.extend(profile.libraries);
    let libraries = dedupe_libraries_preferring_last(libraries);

    let arguments = match (&vanilla.arguments, profile.arguments) {
        (Some(base), Some(extra)) => Some(Arguments {
            game: concat(base.game.clone(), extra.game),
            jvm: concat(base.jvm.clone(), extra.jvm),
        }),
        (Some(base), None) => Some(base.clone()),
        (None, Some(extra)) => Some(extra),
        (None, None) => None,
    };

    VersionDetail {
        id: profile.id,
        version_type: vanilla.version_type.clone(),
        main_class: profile.main_class,
        asset_index: vanilla.asset_index.clone(),
        assets: vanilla.assets.clone(),
        downloads: vanilla.downloads.clone(),
        libraries,
        java_version: vanilla.java_version.clone(),
        arguments,
        // Fabric Loader requires Minecraft 1.14+, which is always past the
        // point Mojang switched to the structured `arguments` object, so a
        // Fabric profile never needs the legacy flat-string fallback. Vanilla's
        // own value (always `None` for any version Fabric could target) is
        // carried through unchanged for correctness rather than hardcoding
        // `None`, in case that assumption is ever wrong for some edge case.
        legacy_minecraft_arguments: vanilla.legacy_minecraft_arguments.clone(),
        logging: vanilla.logging.clone(),
    }
}

fn concat<T>(mut base: Vec<T>, extra: Vec<T>) -> Vec<T> {
    base.extend(extra);
    base
}

/// Percent-encodes a single path segment for Fabric's API. Most version
/// strings (`"1.21.11"`, `"0.16.9"`) need no encoding at all, but Fabric's
/// own API documentation gives `"1.14 Pre-Release 5"` as a real example of
/// a game version containing a space, so this encodes defensively rather
/// than assuming every version string is already URL-safe.
fn percent_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version_detail::{AssetIndexRef, DownloadArtifact, Downloads};

    /// Minimal `Library` builder for tests that only care about `name` —
    /// mirrors the identical helper in `libraries.rs`'s own test module;
    /// duplicated locally rather than shared across modules to keep each
    /// test module self-contained and avoid introducing a `pub(crate)`
    /// surface that exists only for tests.
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

    fn sample_vanilla() -> VersionDetail {
        VersionDetail {
            id: "1.21.11".to_string(),
            version_type: "release".to_string(),
            main_class: "net.minecraft.client.main.Main".to_string(),
            asset_index: AssetIndexRef {
                id: "21".to_string(),
                sha1: "abc".to_string(),
                size: 1,
                total_size: 1,
                url: "https://example.test/assetindex.json".to_string(),
            },
            assets: "21".to_string(),
            downloads: Downloads {
                client: DownloadArtifact {
                    sha1: "def".to_string(),
                    size: 2,
                    url: "https://example.test/client.jar".to_string(),
                },
            },
            libraries: vec![
                lib("org.ow2.asm:asm:9.1"),
                lib("com.mojang:brigadier:1.0.18"),
            ],
            java_version: None,
            arguments: Some(Arguments {
                game: vec![crate::version_detail::ArgumentEntry::Plain(
                    "--username".to_string(),
                )],
                jvm: vec![crate::version_detail::ArgumentEntry::Plain(
                    "-Djava.library.path=${natives_directory}".to_string(),
                )],
            }),
            legacy_minecraft_arguments: None,
            logging: None,
        }
    }

    fn sample_profile() -> FabricProfile {
        FabricProfile {
            id: "fabric-loader-0.16.9-1.21.11".to_string(),
            main_class: "net.fabricmc.loader.impl.launch.knot.KnotClient".to_string(),
            arguments: Some(Arguments {
                game: vec![],
                jvm: vec![crate::version_detail::ArgumentEntry::Plain(
                    "-DFabricMcEmu=net.minecraft.client.main.Main".to_string(),
                )],
            }),
            libraries: vec![
                lib("net.fabricmc:fabric-loader:0.16.9"),
                lib("net.fabricmc:intermediary:1.21.11"),
                // Deliberately overlaps with vanilla's asm to exercise dedup.
                lib("org.ow2.asm:asm:9.7"),
            ],
        }
    }

    #[test]
    fn merge_uses_fabric_id_and_main_class() {
        let merged = merge_profile_onto_vanilla(&sample_vanilla(), sample_profile());
        assert_eq!(merged.id, "fabric-loader-0.16.9-1.21.11");
        assert_eq!(
            merged.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
    }

    #[test]
    fn merge_keeps_vanilla_client_jar_and_asset_index() {
        let vanilla = sample_vanilla();
        let merged = merge_profile_onto_vanilla(&vanilla, sample_profile());
        assert_eq!(merged.downloads.client.url, vanilla.downloads.client.url);
        assert_eq!(merged.asset_index.id, vanilla.asset_index.id);
        assert_eq!(merged.assets, vanilla.assets);
    }

    #[test]
    fn merge_combines_and_dedupes_libraries() {
        let merged = merge_profile_onto_vanilla(&sample_vanilla(), sample_profile());
        // vanilla had 2 (asm, brigadier), fabric added 3 (loader,
        // intermediary, asm-again) — asm should collapse, leaving 4.
        assert_eq!(merged.libraries.len(), 4);
        let asm = merged
            .libraries
            .iter()
            .find(|l| l.name.starts_with("org.ow2.asm:asm:"))
            .unwrap();
        assert_eq!(
            asm.name, "org.ow2.asm:asm:9.7",
            "fabric's asm version should win"
        );
    }

    #[test]
    fn merge_concatenates_jvm_and_game_arguments() {
        let merged = merge_profile_onto_vanilla(&sample_vanilla(), sample_profile());
        let arguments = merged
            .arguments
            .expect("merged arguments should be present");
        assert_eq!(
            arguments.jvm.len(),
            2,
            "vanilla's jvm arg plus fabric's should both be present"
        );
        assert_eq!(
            arguments.game.len(),
            1,
            "fabric contributed no game args here, vanilla's should survive"
        );
    }

    #[test]
    fn recommended_loader_version_prefers_stable() {
        let candidates = vec![
            FabricLoaderForGame {
                loader: FabricLoaderVersion {
                    separator: ".".into(),
                    build: 2,
                    maven: "net.fabricmc:fabric-loader:0.16.10-beta".into(),
                    version: "0.16.10-beta".into(),
                    stable: false,
                },
                intermediary: FabricIntermediary {
                    maven: "net.fabricmc:intermediary:1.21.11".into(),
                    version: "1.21.11".into(),
                    stable: true,
                },
            },
            FabricLoaderForGame {
                loader: FabricLoaderVersion {
                    separator: ".".into(),
                    build: 1,
                    maven: "net.fabricmc:fabric-loader:0.16.9".into(),
                    version: "0.16.9".into(),
                    stable: true,
                },
                intermediary: FabricIntermediary {
                    maven: "net.fabricmc:intermediary:1.21.11".into(),
                    version: "1.21.11".into(),
                    stable: true,
                },
            },
        ];

        let picked = recommended_loader_version(&candidates).unwrap();
        assert_eq!(
            picked.loader.version, "0.16.9",
            "should skip the unstable entry even though it's first"
        );
    }

    #[test]
    fn recommended_loader_version_falls_back_to_first_when_none_stable() {
        let candidates = vec![FabricLoaderForGame {
            loader: FabricLoaderVersion {
                separator: ".".into(),
                build: 1,
                maven: "net.fabricmc:fabric-loader:0.17.0-beta".into(),
                version: "0.17.0-beta".into(),
                stable: false,
            },
            intermediary: FabricIntermediary {
                maven: "net.fabricmc:intermediary:1.21.11".into(),
                version: "1.21.11".into(),
                stable: true,
            },
        }];

        let picked = recommended_loader_version(&candidates).unwrap();
        assert_eq!(picked.loader.version, "0.17.0-beta");
    }

    #[test]
    fn recommended_loader_version_empty_list_returns_none() {
        assert!(recommended_loader_version(&[]).is_none());
    }

    #[test]
    fn percent_encode_handles_space_and_leaves_normal_versions_untouched() {
        assert_eq!(percent_encode_segment("1.21.11"), "1.21.11");
        assert_eq!(percent_encode_segment("0.16.9"), "0.16.9");
        assert_eq!(
            percent_encode_segment("1.14 Pre-Release 5"),
            "1.14%20Pre-Release%205"
        );
    }

    #[test]
    fn realistic_loader_list_json_parses() {
        // Shape taken from Fabric's own published API documentation.
        let json = r#"
        [
          {
            "loader": {"separator": ".", "build": 11, "maven": "net.fabricmc:fabric-loader:0.14.11", "version": "0.14.11", "stable": true},
            "intermediary": {"maven": "net.fabricmc:intermediary:1.19.2", "version": "1.19.2", "stable": true}
          },
          {
            "loader": {"separator": ".", "build": 10, "maven": "net.fabricmc:fabric-loader:0.14.10", "version": "0.14.10", "stable": false},
            "intermediary": {"maven": "net.fabricmc:intermediary:1.19.2", "version": "1.19.2", "stable": true}
          }
        ]
        "#;
        let parsed: Vec<FabricLoaderForGame> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].loader.version, "0.14.11");
        assert!(parsed[0].loader.stable);
    }

    #[test]
    fn realistic_profile_json_parses() {
        // Shape taken from Fabric's own published API documentation
        // (trimmed to the fields this module actually uses).
        let json = r#"
        {
            "id": "fabric-loader-0.14.11-1.19.2",
            "inheritsFrom": "1.19.2",
            "releaseTime": "2024-01-01T00:00:00+00:00",
            "time": "2024-01-01T00:00:00+00:00",
            "type": "release",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"game": [], "jvm": ["-DFabricMcEmu=net.minecraft.client.main.Main"]},
            "libraries": [
                {"name": "net.fabricmc:fabric-loader:0.14.11", "url": "https://maven.fabricmc.net/"},
                {"name": "net.fabricmc:intermediary:1.19.2", "url": "https://maven.fabricmc.net/"},
                {"name": "org.ow2.asm:asm:9.3", "url": "https://maven.fabricmc.net/"}
            ]
        }
        "#;
        let parsed: FabricProfile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.id, "fabric-loader-0.14.11-1.19.2");
        assert_eq!(
            parsed.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(parsed.libraries.len(), 3);
    }
}
