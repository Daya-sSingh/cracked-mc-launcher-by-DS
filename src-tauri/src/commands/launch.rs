use std::path::PathBuf;

use minecraft::{GameAccount, LaunchEvent, LaunchRequest, LaunchStage, Loader};
use serde::Serialize;
use tauri::Emitter;
use uuid::Uuid;

use crate::commands::instances::parse_instance_id;
use crate::state::AppState;

/// Everything sent to the frontend over the `launch:<instance_id>` event
/// channel. A separate type from `minecraft::LaunchEvent` on purpose — that
/// enum is this project's internal, transport-agnostic representation, and
/// deliberately doesn't derive `Serialize` so the `minecraft` crate stays
/// decoupled from Tauri. This is the (only) place that boundary is crossed.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum LaunchEventPayload {
    Stage {
        stage: &'static str,
    },
    DownloadStarted {
        label: String,
        total_bytes: Option<u64>,
    },
    DownloadProgress {
        label: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    DownloadSkipped {
        label: String,
    },
    DownloadRetrying {
        label: String,
        attempt: u32,
        error: String,
    },
    DownloadCompleted {
        label: String,
    },
    DownloadFailed {
        label: String,
        error: String,
    },
    AggregateProgress {
        completed_tasks: usize,
        total_tasks: usize,
        bytes_downloaded: u64,
        total_bytes: u64,
        bytes_per_sec: f64,
    },
    ProcessOutput {
        line: String,
        is_stderr: bool,
    },
    Exited {
        exit_code: Option<i32>,
        was_stopped_by_user: bool,
    },
    /// Emitted directly by this module (not derived from
    /// `minecraft::LaunchEvent`) when `minecraft::launch` itself returns an
    /// `Err` before ever spawning a process — e.g. the version doesn't
    /// exist, every download retry was exhausted, or no Java could be
    /// found or installed.
    Failed {
        message: String,
    },
}

impl From<LaunchEvent> for LaunchEventPayload {
    fn from(event: LaunchEvent) -> Self {
        match event {
            LaunchEvent::Stage(stage) => LaunchEventPayload::Stage {
                stage: match stage {
                    LaunchStage::ResolvingVersion => "resolving_version",
                    LaunchStage::DownloadingFiles => "downloading_files",
                    LaunchStage::InstallingJava => "installing_java",
                    LaunchStage::ExtractingNatives => "extracting_natives",
                    LaunchStage::Starting => "starting",
                },
            },
            LaunchEvent::Download(download_event) => match download_event {
                downloads::DownloadEvent::Started { label, total_bytes } => {
                    LaunchEventPayload::DownloadStarted { label, total_bytes }
                }
                downloads::DownloadEvent::Progress {
                    label,
                    bytes_downloaded,
                    total_bytes,
                } => LaunchEventPayload::DownloadProgress {
                    label,
                    bytes_downloaded,
                    total_bytes,
                },
                downloads::DownloadEvent::Skipped { label } => {
                    LaunchEventPayload::DownloadSkipped { label }
                }
                downloads::DownloadEvent::Retrying {
                    label,
                    attempt,
                    error,
                } => LaunchEventPayload::DownloadRetrying {
                    label,
                    attempt,
                    error,
                },
                downloads::DownloadEvent::Completed { label } => {
                    LaunchEventPayload::DownloadCompleted { label }
                }
                downloads::DownloadEvent::Failed { label, error } => {
                    LaunchEventPayload::DownloadFailed { label, error }
                }
                downloads::DownloadEvent::AggregateProgress {
                    completed_tasks,
                    total_tasks,
                    bytes_downloaded,
                    total_bytes,
                    bytes_per_sec,
                } => LaunchEventPayload::AggregateProgress {
                    completed_tasks,
                    total_tasks,
                    bytes_downloaded,
                    total_bytes,
                    bytes_per_sec,
                },
            },
            LaunchEvent::ProcessOutput { line, is_stderr } => {
                LaunchEventPayload::ProcessOutput { line, is_stderr }
            }
            LaunchEvent::Exited {
                exit_code,
                was_stopped_by_user,
            } => LaunchEventPayload::Exited {
                exit_code,
                was_stopped_by_user,
            },
        }
    }
}

fn event_channel_name(instance_id: Uuid) -> String {
    format!("launch:{instance_id}")
}

/// Starts an instance. Returns almost immediately — the actual download and
/// launch work happens on a background task, with all progress reported
/// through `launch:<instance_id>` events, because a launch that needs to
/// download gigabytes of assets can take far longer than any sane command
/// timeout, and the frontend needs incremental progress anyway.
///
/// `account_username` is the offline-mode display name for this milestone;
/// Microsoft account selection plugs in at this same parameter once auth
/// lands (see `docs/ARCHITECTURE.md`).
#[tauri::command]
pub async fn launch_instance(
    instance_id: String,
    account_username: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let id = parse_instance_id(&instance_id)?;

    {
        let running = state.running.lock().unwrap();
        if running.contains_key(&id) {
            return Err("This instance is already running.".to_string());
        }
    }

    if account_username.trim().is_empty() {
        return Err("Enter a username before launching in offline mode.".to_string());
    }

    let instance = state.instances.get(id).await.map_err(|e| e.to_string())?;

    let request = LaunchRequest {
        instance_id: id,
        minecraft_version: instance.minecraft_version.clone(),
        loader: to_minecraft_loader(instance.loader),
        loader_version: instance.loader_version.clone(),
        account: GameAccount::offline(account_username.trim().to_string()),
        java_override: instance.java_path.clone().map(PathBuf::from),
        extra_java_args: split_args(&instance.java_args),
        extra_game_args: split_args(&instance.game_args),
        memory_min_mb: instance.memory_min_mb,
        memory_max_mb: instance.memory_max_mb,
        window_width: instance.window_width,
        window_height: instance.window_height,
        fullscreen: instance.fullscreen,
    };

    let app_state = state.inner().clone();
    let app_handle = app.clone();

    // Two cooperating tasks: this one drives `minecraft::launch` to
    // completion (downloads + process spawn) and records the resulting
    // handle; the other (spawned just below) drains the event channel for
    // the whole lifetime of the launch, forwarding to the frontend and
    // cleaning up `state.running` the moment the game process exits.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<LaunchEvent>();

    let forwarder_state = app_state.clone();
    let forwarder_app = app_handle.clone();
    tokio::spawn(async move {
        let channel = event_channel_name(id);
        while let Some(event) = rx.recv().await {
            if let LaunchEvent::Exited { .. } = &event {
                let elapsed = forwarder_state
                    .running
                    .lock()
                    .unwrap()
                    .remove(&id)
                    .map(|handle| handle.elapsed_seconds())
                    .unwrap_or(0);
                if let Err(err) = forwarder_state.instances.record_session(id, elapsed).await {
                    tracing::warn!(error = %err, instance_id = %id, "failed to record play session");
                }
            }
            let _ = forwarder_app.emit(&channel, LaunchEventPayload::from(event));
        }
    });

    tokio::spawn(async move {
        let result = minecraft::launch::launch(
            request,
            &app_state.paths,
            &app_state.http,
            &app_state.download_manager,
            tx,
        )
        .await;

        match result {
            Ok(handle) => {
                app_state.running.lock().unwrap().insert(id, handle);
            }
            Err(err) => {
                tracing::error!(error = %err, instance_id = %id, "launch failed before the game process could start");
                let channel = event_channel_name(id);
                let _ = app_handle.emit(
                    &channel,
                    LaunchEventPayload::Failed {
                        message: err.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn stop_instance(instance_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_instance_id(&instance_id)?;
    let running = state.running.lock().unwrap();
    match running.get(&id) {
        Some(handle) => {
            handle.request_stop();
            Ok(())
        }
        None => Err("This instance is not currently running.".to_string()),
    }
}

#[tauri::command]
pub fn is_instance_running(
    instance_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let id = parse_instance_id(&instance_id)?;
    Ok(state.running.lock().unwrap().contains_key(&id))
}

#[tauri::command]
pub fn list_running_instances(state: tauri::State<'_, AppState>) -> Vec<String> {
    state
        .running
        .lock()
        .unwrap()
        .keys()
        .map(|id| id.to_string())
        .collect()
}

fn split_args(raw: &Option<String>) -> Vec<String> {
    raw.as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// `minecraft` deliberately doesn't depend on `database` (see
/// `docs/ARCHITECTURE.md`), so the two crates each have their own `Loader`
/// enum with the same two variants. This is the one place they meet.
fn to_minecraft_loader(loader: database::Loader) -> Loader {
    match loader {
        database::Loader::Vanilla => Loader::Vanilla,
        database::Loader::Fabric => Loader::Fabric,
    }
}
