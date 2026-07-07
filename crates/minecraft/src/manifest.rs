use serde::Deserialize;

use crate::error::MinecraftError;
use crate::version_detail::VersionDetail;

const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

/// Fetches the full list of every Java Edition version Mojang publishes —
/// every release, every snapshot, all the way back to the earliest alphas.
/// This is intentionally not cached at this layer; callers (the
/// `database`-backed cache, in later milestones) decide how long a fetch is
/// considered fresh.
pub async fn fetch_version_manifest(client: &reqwest::Client) -> Result<VersionManifest, MinecraftError> {
    let response = client.get(VERSION_MANIFEST_URL).send().await?;
    let bytes = response.error_for_status()?.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|source| MinecraftError::Deserialize {
        context: "version_manifest_v2.json".to_string(),
        source,
    })
}

/// Fetches the full `<version>.json` for one specific version (its
/// `url` field from [`VersionSummary`]) — this is what describes the
/// libraries, assets, JVM/game arguments, and Java requirement for that
/// exact version.
pub async fn fetch_version_detail(
    client: &reqwest::Client,
    version_url: &str,
) -> Result<VersionDetail, MinecraftError> {
    let bytes = fetch_version_detail_bytes(client, version_url).await?;
    parse_version_detail(&bytes, version_url)
}

/// Same as [`fetch_version_detail`] but returns the raw response body
/// instead of a parsed struct, so callers can persist exactly what the
/// server sent (these files are immutable once published, so the cached
/// copy never needs to expire).
pub async fn fetch_version_detail_bytes(
    client: &reqwest::Client,
    version_url: &str,
) -> Result<Vec<u8>, MinecraftError> {
    let response = client.get(version_url).send().await?;
    Ok(response.error_for_status()?.bytes().await?.to_vec())
}

pub fn parse_version_detail(bytes: &[u8], context: &str) -> Result<VersionDetail, MinecraftError> {
    serde_json::from_slice(bytes).map_err(|source| MinecraftError::Deserialize {
        context: format!("version detail at {context}"),
        source,
    })
}

impl VersionManifest {
    pub fn find(&self, version_id: &str) -> Option<&VersionSummary> {
        self.versions.iter().find(|v| v.id == version_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_realistic_manifest_fragment() {
        let json = r#"
        {
            "latest": {"release": "1.21.11", "snapshot": "26w03a"},
            "versions": [
                {"id": "26w03a", "type": "snapshot", "url": "https://example.test/26w03a.json", "releaseTime": "2026-01-15T12:00:00+00:00"},
                {"id": "1.21.11", "type": "release", "url": "https://example.test/1.21.11.json", "releaseTime": "2026-01-08T12:00:00+00:00"}
            ]
        }
        "#;
        let manifest: VersionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.latest.release, "1.21.11");
        assert_eq!(manifest.versions.len(), 2);
        assert_eq!(manifest.find("1.21.11").unwrap().version_type, VersionType::Release);
        assert!(manifest.find("does-not-exist").is_none());
    }

    /// Hits the real Mojang endpoint. Not run by default (`cargo test`
    /// skips `#[ignore]`d tests) — run explicitly with
    /// `cargo test -- --ignored` on a machine with network access.
    #[tokio::test]
    #[ignore]
    async fn live_manifest_is_reachable_and_well_formed() {
        let client = reqwest::Client::new();
        let manifest = fetch_version_manifest(&client).await.unwrap();
        assert!(!manifest.versions.is_empty());
        assert!(manifest.find(&manifest.latest.release).is_some());
    }
}
