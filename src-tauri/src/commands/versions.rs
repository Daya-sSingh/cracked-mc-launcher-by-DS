use chrono::{DateTime, Utc};
use minecraft::VersionManifest;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

const CACHE_KEY: &str = "version_manifest_cache";
const CACHE_TTL_SECONDS: i64 = 60 * 60; // Mojang publishes new snapshots at
                                         // most a few times a week; checking
                                         // at most once an hour is plenty
                                         // fresh while avoiding hitting their
                                         // CDN on every "create instance"
                                         // dialog open.

#[derive(Debug, Serialize, Deserialize)]
struct CachedManifest {
    fetched_at: DateTime<Utc>,
    manifest: VersionManifest,
}

/// Returns the full Mojang version manifest (every release and snapshot),
/// serving from the SQLite-backed cache when it's fresh and falling back to
/// a live fetch otherwise. This is what backs the version picker in the
/// "create instance" flow.
#[tauri::command]
pub async fn get_version_manifest(
    force_refresh: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<VersionManifest, String> {
    let force_refresh = force_refresh.unwrap_or(false);

    if !force_refresh {
        if let Some(cached) = read_cache(&state).await {
            let age_seconds = (Utc::now() - cached.fetched_at).num_seconds();
            if age_seconds < CACHE_TTL_SECONDS {
                return Ok(cached.manifest);
            }
        }
    }

    match minecraft::fetch_version_manifest(&state.http).await {
        Ok(manifest) => {
            write_cache(&state, &manifest).await;
            Ok(manifest)
        }
        Err(err) => {
            // Network hiccup but we have *something* cached, even if
            // stale — better to show slightly-old version list than an
            // error screen when the user is offline.
            if let Some(cached) = read_cache(&state).await {
                tracing::warn!(error = %err, "version manifest fetch failed, serving stale cache");
                return Ok(cached.manifest);
            }
            Err(err.to_string())
        }
    }
}

async fn read_cache(state: &AppState) -> Option<CachedManifest> {
    let raw = state.settings.get(CACHE_KEY).await.ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

async fn write_cache(state: &AppState, manifest: &VersionManifest) {
    let cached = CachedManifest {
        fetched_at: Utc::now(),
        manifest: manifest.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        if let Err(err) = state.settings.set(CACHE_KEY, &json).await {
            tracing::warn!(error = %err, "failed to persist version manifest cache");
        }
    }
}
