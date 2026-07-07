use minecraft::fabric::{fetch_compatible_loader_versions, FabricLoaderForGame};

use crate::state::AppState;

/// Returns every Fabric Loader build compatible with `game_version`, used
/// to populate the loader-version picker that appears in the "create
/// instance" flow once the user selects the Fabric loader. No local
/// caching here (unlike `get_version_manifest`) — this is a small, fast
/// endpoint that's only hit when the create-instance dialog is actually
/// open and Fabric is selected, so the added complexity of a persisted
/// cache isn't worth it for how rarely and briefly it's called.
#[tauri::command]
pub async fn get_fabric_loader_versions(
    game_version: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FabricLoaderForGame>, String> {
    if game_version.trim().is_empty() {
        return Err("A Minecraft version is required to list Fabric loader builds.".to_string());
    }

    fetch_compatible_loader_versions(&state.http, &game_version)
        .await
        .map_err(|e| e.to_string())
}
