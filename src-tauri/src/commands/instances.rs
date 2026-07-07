use chrono::{DateTime, Utc};
use database::{Instance, InstanceDraft, InstanceSort, InstanceUpdate, Loader};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

/// Frontend-facing shape of a new-instance request. Kept separate from
/// `database::InstanceDraft` so the wire format (loader as a plain lowercase
/// string) doesn't have to track Rust enum derive details, and so this file
/// is the only place that has to change if the two ever diverge.
#[derive(Debug, Deserialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub minecraft_version: String,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSortRequest {
    RecentlyPlayed,
    NameAscending,
    NameDescending,
    FavoritesFirst,
}

impl From<InstanceSortRequest> for InstanceSort {
    fn from(value: InstanceSortRequest) -> Self {
        match value {
            InstanceSortRequest::RecentlyPlayed => InstanceSort::RecentlyPlayed,
            InstanceSortRequest::NameAscending => InstanceSort::NameAscending,
            InstanceSortRequest::NameDescending => InstanceSort::NameDescending,
            InstanceSortRequest::FavoritesFirst => InstanceSort::FavoritesFirst,
        }
    }
}

#[tauri::command]
pub async fn create_instance(
    request: CreateInstanceRequest,
    state: tauri::State<'_, AppState>,
) -> Result<Instance, String> {
    let loader: Loader = request.loader.parse().map_err(|e: database::DatabaseError| e.to_string())?;

    if request.name.trim().is_empty() {
        return Err("Instance name cannot be empty.".to_string());
    }

    let draft = InstanceDraft {
        name: request.name.trim().to_string(),
        loader,
        loader_version: request.loader_version,
        minecraft_version: request.minecraft_version,
        icon: request.icon,
    };

    let instance = state.instances.create(draft).await.map_err(|e| e.to_string())?;

    // The game directory is created eagerly (rather than lazily on first
    // launch) so the user can e.g. drop mods into an instance's folder via
    // their file manager before ever pressing Play.
    let game_dir = state.paths.instance_game_dir(instance.id);
    tokio::fs::create_dir_all(&game_dir)
        .await
        .map_err(|e| format!("Created the instance, but failed to prepare its folder: {e}"))?;

    Ok(instance)
}

#[tauri::command]
pub async fn list_instances(
    sort: Option<InstanceSortRequest>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Instance>, String> {
    let sort = sort.map(InstanceSort::from).unwrap_or_default();
    state.instances.list(sort).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_instance(instance_id: String, state: tauri::State<'_, AppState>) -> Result<Instance, String> {
    let id = parse_instance_id(&instance_id)?;
    state.instances.get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_instance(
    instance_id: String,
    update: InstanceUpdate,
    state: tauri::State<'_, AppState>,
) -> Result<Instance, String> {
    let id = parse_instance_id(&instance_id)?;
    state.instances.update(id, update).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_instance(instance_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_instance_id(&instance_id)?;

    {
        let running = state.running.lock().unwrap();
        if running.contains_key(&id) {
            return Err("Stop the instance before deleting it.".to_string());
        }
    }

    state.instances.delete(id).await.map_err(|e| e.to_string())?;

    let instance_dir = state.paths.instance_dir(id);
    if instance_dir.exists() {
        tokio::fs::remove_dir_all(&instance_dir)
            .await
            .map_err(|e| format!("Instance was removed from the library, but its files could not be deleted: {e}"))?;
    }

    Ok(())
}

/// Opens an instance's game folder (the `.minecraft`-equivalent directory
/// holding saves, resource packs, config, logs, and mods) in the OS's file
/// manager, so a player can drop in a mod jar downloaded from GitHub or
/// built themselves — anything that isn't installed through the launcher's
/// own mod browser.
#[tauri::command]
pub async fn open_instance_folder(instance_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_instance_id(&instance_id)?;
    let game_dir = state.paths.instance_game_dir(id);

    // Fabric (and Forge/NeoForge/Quilt, once supported) look for mods in a
    // `mods` subfolder that's normally created on first launch. Ensuring it
    // exists here means a brand new instance still has somewhere obvious
    // to drop a mod into even before it's ever been launched once.
    tokio::fs::create_dir_all(game_dir.join("mods"))
        .await
        .map_err(|e| format!("Could not prepare the instance folder: {e}"))?;

    open_path_in_file_manager(&game_dir).map_err(|e| format!("Could not open the instance folder: {e}"))
}

#[cfg(target_os = "windows")]
fn open_path_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    // `explorer.exe` is well known for returning a non-zero exit code even
    // on a fully successful open, so this only checks whether the process
    // could be *spawned* at all, never its exit status.
    std::process::Command::new("explorer").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_path_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_path_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

/// One `.jar` found in an instance's `mods` folder. Read-only listing for
/// now — enabling/disabling, dependency info, and update checks all need
/// real mod-metadata parsing (or a Modrinth/CurseForge lookup by hash),
/// which belongs to the mod-browser milestone, not this one.
#[derive(Debug, Serialize)]
pub struct ModFileInfo {
    pub file_name: String,
    pub size_bytes: u64,
    /// Best-effort — `None` if the platform/filesystem doesn't report a
    /// modification time, which is rare but not impossible.
    pub modified_at: Option<String>,
}

/// Lists every `.jar` in an instance's `mods` folder. Returns an empty list
/// (not an error) if the folder doesn't exist yet — a brand new instance
/// that's never had a mod dropped into it is a normal state, not a fault.
#[tauri::command]
pub async fn list_instance_mods(
    instance_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModFileInfo>, String> {
    let id = parse_instance_id(&instance_id)?;
    let mods_dir = state.paths.instance_game_dir(id).join("mods");

    if !mods_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&mods_dir).await.map_err(|e| e.to_string())?;
    let mut mods = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        let is_jar = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("jar"))
            .unwrap_or(false);
        if !is_jar {
            continue;
        }

        let metadata = entry.metadata().await.map_err(|e| e.to_string())?;
        let modified_at = metadata
            .modified()
            .ok()
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339());

        mods.push(ModFileInfo {
            file_name: entry.file_name().to_string_lossy().to_string(),
            size_bytes: metadata.len(),
            modified_at,
        });
    }

    mods.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    Ok(mods)
}

/// Copies dropped/browsed `.jar` files into an instance's `mods` folder —
/// the backend half of the drag-and-drop import zone. Non-jar paths in the
/// input (a user can drag a mixed selection) are silently skipped rather
/// than erroring the whole batch, since dropping "a mod and also some
/// unrelated file" is a normal accident, not something worth failing over.
///
/// Returns how many files were actually imported, so the frontend can show
/// "3 mods added" style feedback.
#[tauri::command]
pub async fn import_mod_files(
    instance_id: String,
    file_paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let id = parse_instance_id(&instance_id)?;
    let mods_dir = state.paths.instance_game_dir(id).join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| format!("Could not prepare the mods folder: {e}"))?;

    let mut imported = 0usize;

    for raw_path in file_paths {
        let source = std::path::PathBuf::from(&raw_path);

        let is_jar = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("jar"))
            .unwrap_or(false);
        if !is_jar {
            continue;
        }

        let Some(file_name) = source.file_name() else {
            continue;
        };

        let destination = mods_dir.join(file_name);
        tokio::fs::copy(&source, &destination)
            .await
            .map_err(|e| format!("Failed to copy {}: {e}", file_name.to_string_lossy()))?;
        imported += 1;
    }

    Ok(imported)
}

pub fn parse_instance_id(raw: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw).map_err(|_| format!("'{raw}' is not a valid instance id."))
}
