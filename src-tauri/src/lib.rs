mod commands;
mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use database::{SqliteInstanceRepository, SqliteSettingsRepository};
use downloads::DownloadManager;
use minecraft::LauncherPaths;
use tauri::Manager;

use state::AppState;

/// Builds every piece of shared state the app needs: the SQLite pool (and
/// its migrations), the on-disk layout, the shared HTTP client, and the
/// download manager. Called once from `setup()`, before any window is
/// shown — see the comment at the call site for why this blocks instead of
/// running as a detached task.
///
/// Returns a plain `String` error rather than `anyhow::Error`: the only
/// caller immediately needs to hand the error to Tauri's setup hook as
/// `Box<dyn std::error::Error>`, and the standard library's `From<String>`
/// impl for that makes a `String` the path of least resistance there.
async fn initialize_state(app: &tauri::AppHandle) -> Result<AppState, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve the application data directory: {e}"))?;

    let paths = LauncherPaths::new(data_dir);
    std::fs::create_dir_all(paths.root())
        .map_err(|e| format!("could not create app data directory at {}: {e}", paths.root().display()))?;

    let pool = database::init_pool(&paths.database_file())
        .await
        .map_err(|e| format!("failed to open or migrate the launcher database: {e}"))?;

    let instances = Arc::new(SqliteInstanceRepository::new(pool.clone()));
    let settings = Arc::new(SqliteSettingsRepository::new(pool));

    let http = reqwest::Client::builder()
        .user_agent(concat!("launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("failed to construct the shared HTTP client: {e}"))?;

    // 12 concurrent transfers is a deliberately moderate default: enough to
    // saturate most home connections on a fresh asset download without
    // tripping rate limits on Mojang's/Modrinth's CDNs. Exposed later as a
    // user-tunable "bandwidth/parallelism" setting per the spec.
    let download_manager = DownloadManager::new(http.clone(), 12);

    Ok(AppState {
        instances,
        settings,
        paths,
        http,
        download_manager,
        running: Arc::new(Mutex::new(HashMap::new())),
    })
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "launcher_lib=info,minecraft=info,downloads=info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            // `setup` itself is synchronous, but state initialization (DB
            // open + migrate, directory creation) is async. We deliberately
            // block here with `block_on` rather than `tauri::async_runtime::spawn`-ing
            // it: spawning would let the window finish loading — and the
            // frontend start firing commands — before `AppState` is
            // `manage()`d, which would make every early command fail to
            // find its state. Blocking setup by a few tens of milliseconds
            // is a fine trade for "state is always there by the time a
            // command can run."
            let app_handle = app.handle().clone();
            let app_state = tauri::async_runtime::block_on(initialize_state(&app_handle))
                .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::instances::create_instance,
            commands::instances::list_instances,
            commands::instances::get_instance,
            commands::instances::update_instance,
            commands::instances::delete_instance,
            commands::instances::open_instance_folder,
            commands::instances::list_instance_mods,
            commands::instances::import_mod_files,
            commands::versions::get_version_manifest,
            commands::fabric::get_fabric_loader_versions,
            commands::launch::launch_instance,
            commands::launch::stop_instance,
            commands::launch::is_instance_running,
            commands::launch::list_running_instances,
            commands::settings::get_setting,
            commands::settings::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the launcher application");
}

