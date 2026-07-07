use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use database::{InstanceRepository, SettingsRepository};
use downloads::DownloadManager;
use minecraft::{LaunchHandle, LauncherPaths};
use uuid::Uuid;

/// Everything a Tauri command might need, constructed once at startup and
/// shared via `app.manage(...)`. Every field is either an `Arc`/cheap-clone
/// type or wrapped in one, so individual fields can be cloned into spawned
/// background tasks without cloning the whole struct's intent — commands
/// borrow what they need from `tauri::State<'_, AppState>` and clone only
/// that.
///
/// Repositories are stored as trait objects (`Arc<dyn ...>`) rather than the
/// concrete SQLite types: this is the project's one Tauri-facing instance of
/// the Repository Pattern + Dependency Injection the spec calls for —
/// commands depend on `InstanceRepository`'s interface, not on SQLite.
pub struct AppState {
    pub instances: Arc<dyn InstanceRepository>,
    pub settings: Arc<dyn SettingsRepository>,
    pub paths: LauncherPaths,
    pub http: reqwest::Client,
    pub download_manager: DownloadManager,
    /// Instances with a game process currently running, keyed by instance
    /// id. An instance's entry is inserted once `minecraft::launch` returns
    /// a handle (i.e. the process has actually spawned) and removed once
    /// its `Exited` event has been observed and persisted.
    pub running: Arc<Mutex<HashMap<Uuid, LaunchHandle>>>,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            instances: self.instances.clone(),
            settings: self.settings.clone(),
            paths: self.paths.clone(),
            http: self.http.clone(),
            download_manager: self.download_manager.clone(),
            running: self.running.clone(),
        }
    }
}
