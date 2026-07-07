use crate::state::AppState;

/// Generic settings storage: the frontend owns the shape of each key's
/// value (as a JSON string) and this just persists/retrieves it verbatim.
/// Keeps the Settings panel free to grow new preference fields without
/// needing a new Tauri command or database migration for each one.
#[tauri::command]
pub async fn get_setting(key: String, state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    state.settings.get(&key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_setting(key: String, value_json: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.settings.set(&key, &value_json).await.map_err(|e| e.to_string())
}
