use std::collections::HashMap;
use std::path::{Path, PathBuf};

use downloads::DownloadTask;
use serde::Deserialize;

use crate::error::MinecraftError;
use crate::version_detail::AssetIndexRef;

const RESOURCES_BASE_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Downloads and parses the asset index referenced by a version's
/// `assetIndex` field — the list of every sound, texture, language file,
/// etc. that version needs, each identified by its SHA-1 hash.
pub async fn fetch_asset_index(
    client: &reqwest::Client,
    asset_index_ref: &AssetIndexRef,
) -> Result<AssetIndex, MinecraftError> {
    let bytes = fetch_asset_index_bytes(client, &asset_index_ref.url).await?;
    parse_asset_index(&bytes, &asset_index_ref.id)
}

/// Same as [`fetch_asset_index`] but returns the raw bytes, so callers can
/// persist exactly what was downloaded (asset indexes are immutable once
/// published for a given id, so a cached copy never needs to expire).
pub async fn fetch_asset_index_bytes(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, MinecraftError> {
    let response = client.get(url).send().await?;
    Ok(response.error_for_status()?.bytes().await?.to_vec())
}

pub fn parse_asset_index(bytes: &[u8], context: &str) -> Result<AssetIndex, MinecraftError> {
    serde_json::from_slice(bytes).map_err(|source| MinecraftError::Deserialize {
        context: format!("asset index '{context}'"),
        source,
    })
}

/// Builds one [`DownloadTask`] per asset object, deduplicated by hash (many
/// logical asset names share identical bytes, e.g. empty/placeholder
/// files) so we never queue the same download twice in one batch.
///
/// Layout matches the official launcher's: `assets_dir/objects/<hh>/<hash>`,
/// where `<hh>` is the first two hex characters of the hash. This is shared,
/// content-addressed storage across every instance — see
/// `docs/ARCHITECTURE.md` for why instances don't each get their own copy.
pub fn build_asset_download_tasks(
    asset_index: &AssetIndex,
    assets_dir: &Path,
) -> Vec<DownloadTask> {
    let mut seen_hashes = std::collections::HashSet::new();
    let mut tasks = Vec::new();

    for (name, object) in &asset_index.objects {
        if !seen_hashes.insert(object.hash.clone()) {
            continue;
        }

        let prefix = &object.hash[0..2.min(object.hash.len())];
        let destination = assets_dir.join("objects").join(prefix).join(&object.hash);
        let url = format!("{RESOURCES_BASE_URL}/{prefix}/{}", object.hash);

        tasks.push(
            DownloadTask::new(url, destination, format!("asset: {name}"))
                .with_sha1(object.hash.clone())
                .with_size(object.size),
        );
    }

    tasks
}

/// Versions before 1.7 (assets index id `"legacy"` or `"pre-1.6"`) expect
/// assets laid out by their *logical* path under `assets/virtual/<id>/...`
/// rather than content-addressed by hash. Modern launchers materialize this
/// view by copying (not moving — the hashed copy is shared with every other
/// instance) each object to its legacy path once the content-addressed
/// download has completed.
pub async fn materialize_legacy_asset_layout(
    asset_index: &AssetIndex,
    assets_dir: &Path,
    asset_index_id: &str,
) -> Result<(), MinecraftError> {
    let virtual_root = assets_dir.join("virtual").join(asset_index_id);

    for (name, object) in &asset_index.objects {
        let prefix = &object.hash[0..2.min(object.hash.len())];
        let source = assets_dir.join("objects").join(prefix).join(&object.hash);
        let target = virtual_root.join(name);

        if target.exists() {
            continue;
        }

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|source| MinecraftError::Io {
                    context: format!("creating legacy asset directory for {name}"),
                    source,
                })?;
        }

        tokio::fs::copy(&source, &target)
            .await
            .map_err(|source| MinecraftError::Io {
                context: format!("copying legacy asset {name}"),
                source,
            })?;
    }

    Ok(())
}

/// Whether this asset index needs the legacy on-disk layout in addition to
/// the normal content-addressed one.
pub fn needs_legacy_layout(asset_index_id: &str) -> bool {
    asset_index_id == "legacy" || asset_index_id == "pre-1.6"
}

#[allow(dead_code)]
fn object_path(assets_dir: &Path, object: &AssetObject) -> PathBuf {
    let prefix = &object.hash[0..2.min(object.hash.len())];
    assets_dir.join("objects").join(prefix).join(&object.hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> AssetIndex {
        let mut objects = HashMap::new();
        objects.insert(
            "minecraft/sounds/random/click.ogg".to_string(),
            AssetObject {
                hash: "aabbccddeeff00112233445566778899aabbccd".to_string(),
                size: 1234,
            },
        );
        objects.insert(
            "minecraft/lang/en_us.json".to_string(),
            AssetObject {
                hash: "00112233445566778899aabbccddeeff0011223".to_string(),
                size: 5678,
            },
        );
        AssetIndex { objects }
    }

    #[test]
    fn builds_one_task_per_unique_hash() {
        let index = sample_index();
        let tasks = build_asset_download_tasks(&index, Path::new("/tmp/assets"));
        assert_eq!(tasks.len(), 2);
        for task in &tasks {
            assert!(task.url.starts_with(RESOURCES_BASE_URL));
            assert!(task.sha1.is_some());
        }
    }

    #[test]
    fn duplicate_hashes_collapse_to_one_task() {
        let mut objects = HashMap::new();
        objects.insert(
            "a.txt".to_string(),
            AssetObject {
                hash: "1111111111111111111111111111111111111".to_string(),
                size: 1,
            },
        );
        objects.insert(
            "b.txt".to_string(),
            AssetObject {
                hash: "1111111111111111111111111111111111111".to_string(),
                size: 1,
            },
        );
        let index = AssetIndex { objects };
        let tasks = build_asset_download_tasks(&index, Path::new("/tmp/assets"));
        assert_eq!(
            tasks.len(),
            1,
            "identical hashes should only be downloaded once"
        );
    }

    #[test]
    fn legacy_layout_detection() {
        assert!(needs_legacy_layout("legacy"));
        assert!(needs_legacy_layout("pre-1.6"));
        assert!(!needs_legacy_layout("21"));
    }
}
