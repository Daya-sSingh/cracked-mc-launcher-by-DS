use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{stream, StreamExt};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;

use crate::progress::DownloadEvent;
use crate::task::{DownloadError, DownloadTask};
use crate::verify::sha1_of_file;

const MAX_ATTEMPTS: u32 = 4;
const PROGRESS_EMIT_THRESHOLD_BYTES: u64 = 64 * 1024;

/// Shared handle for pausing, resuming, or cancelling an in-flight batch of
/// downloads. Cheap to clone — every task in a batch holds one.
#[derive(Clone)]
pub struct DownloadController {
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    resume_notify: Arc<Notify>,
}

impl DownloadController {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.resume_notify.notify_waiters();
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
        self.resume_notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Blocks the calling task while paused. Returns immediately once
    /// resumed or cancelled (the caller checks `is_cancelled()` itself
    /// afterward — this just stops burning CPU while parked).
    async fn wait_if_paused(&self) {
        while self.is_paused() && !self.is_cancelled() {
            self.resume_notify.notified().await;
        }
    }
}

impl Default for DownloadController {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs batches of [`DownloadTask`]s with bounded concurrency, automatic
/// retry with backoff, resumable transfers, and checksum verification.
///
/// Cheap to clone: it's just a `reqwest::Client` (itself internally Arc'd
/// connection pooling) plus a concurrency limit, so sharing one across the
/// whole app via cloned app state costs nothing extra.
#[derive(Clone)]
pub struct DownloadManager {
    client: reqwest::Client,
    max_concurrency: usize,
}

impl DownloadManager {
    pub fn new(client: reqwest::Client, max_concurrency: usize) -> Self {
        Self {
            client,
            max_concurrency: max_concurrency.max(1),
        }
    }

    /// Downloads every task, fanning out up to `max_concurrency` at a time.
    /// Every task is attempted (a failure in one does not cancel the
    /// others) — this matters for "repair" flows where most files are
    /// already fine and only a few need re-fetching. If anything ultimately
    /// failed after retries, the first such error is returned once the
    /// whole batch has settled.
    pub async fn run(
        &self,
        tasks: Vec<DownloadTask>,
        controller: DownloadController,
        events: UnboundedSender<DownloadEvent>,
    ) -> Result<(), DownloadError> {
        let total_tasks = tasks.len();
        let total_bytes: u64 = tasks.iter().filter_map(|t| t.expected_size).sum();

        let bytes_downloaded = Arc::new(AtomicU64::new(0));
        let completed_tasks = Arc::new(AtomicUsize::new(0));
        let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ticker_handle = self.spawn_progress_ticker(
            total_tasks,
            total_bytes,
            bytes_downloaded.clone(),
            completed_tasks.clone(),
            events.clone(),
        );

        let client = self.client.clone();
        stream::iter(tasks.into_iter())
            .map(|task| {
                let client = client.clone();
                let controller = controller.clone();
                let events = events.clone();
                let bytes_downloaded = bytes_downloaded.clone();
                let completed_tasks = completed_tasks.clone();
                let failures = failures.clone();

                async move {
                    let result =
                        download_one_with_retry(&client, &task, &controller, &events, &bytes_downloaded)
                            .await;
                    completed_tasks.fetch_add(1, Ordering::Relaxed);

                    match result {
                        Ok(()) => {
                            let _ = events.send(DownloadEvent::Completed { label: task.label.clone() });
                        }
                        Err(err) => {
                            let _ = events.send(DownloadEvent::Failed {
                                label: task.label.clone(),
                                error: err.to_string(),
                            });
                            failures.lock().unwrap().push(err.to_string());
                        }
                    }
                }
            })
            .buffer_unordered(self.max_concurrency)
            .collect::<Vec<()>>()
            .await;

        ticker_handle.abort();

        let failures = failures.lock().unwrap();
        if failures.is_empty() {
            Ok(())
        } else if controller.is_cancelled() {
            Err(DownloadError::Cancelled)
        } else {
            Err(DownloadError::PartialFailure {
                failed_count: failures.len(),
                total_count: total_tasks,
                first_message: failures[0].clone(),
            })
        }
    }

    /// Every 500ms, turns the raw shared byte counter into a speed estimate
    /// and emits one aggregate event — independent of how many individual
    /// files happen to start or finish in that window.
    fn spawn_progress_ticker(
        &self,
        total_tasks: usize,
        total_bytes: u64,
        bytes_downloaded: Arc<AtomicU64>,
        completed_tasks: Arc<AtomicUsize>,
        events: UnboundedSender<DownloadEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut last_bytes = 0u64;
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                let current = bytes_downloaded.load(Ordering::Relaxed);
                let speed = (current.saturating_sub(last_bytes)) as f64 / 0.5;
                last_bytes = current;

                if events
                    .send(DownloadEvent::AggregateProgress {
                        completed_tasks: completed_tasks.load(Ordering::Relaxed),
                        total_tasks,
                        bytes_downloaded: current,
                        total_bytes,
                        bytes_per_sec: speed,
                    })
                    .is_err()
                {
                    // Receiver dropped (caller stopped listening) — nothing
                    // left to report progress to.
                    break;
                }
            }
        })
    }
}

async fn download_one_with_retry(
    client: &reqwest::Client,
    task: &DownloadTask,
    controller: &DownloadController,
    events: &UnboundedSender<DownloadEvent>,
    bytes_counter: &Arc<AtomicU64>,
) -> Result<(), DownloadError> {
    if controller.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }

    if already_up_to_date(task).await {
        let _ = events.send(DownloadEvent::Skipped {
            label: task.label.clone(),
        });
        if let Some(size) = task.expected_size {
            bytes_counter.fetch_add(size, Ordering::Relaxed);
        }
        return Ok(());
    }

    let _ = events.send(DownloadEvent::Started {
        label: task.label.clone(),
        total_bytes: task.expected_size,
    });

    let mut last_error: Option<DownloadError> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download_once(client, task, controller, events, bytes_counter).await {
            Ok(()) => return Ok(()),
            Err(DownloadError::Cancelled) => return Err(DownloadError::Cancelled),
            Err(err) => {
                let _ = events.send(DownloadEvent::Retrying {
                    label: task.label.clone(),
                    attempt,
                    error: err.to_string(),
                });
                last_error = Some(err);
                if attempt < MAX_ATTEMPTS {
                    let backoff_ms = 500u64.saturating_mul(1u64 << (attempt - 1));
                    tokio::time::sleep(Duration::from_millis(backoff_ms.min(8_000))).await;
                }
            }
        }
    }

    Err(DownloadError::RetriesExhausted {
        label: task.label.clone(),
        attempts: MAX_ATTEMPTS,
        last_error: last_error.map(|e| e.to_string()).unwrap_or_default(),
    })
}

/// A file counts as already correct if it exists and either its checksum
/// matches (preferred, used whenever the manifest gave us one) or, lacking a
/// checksum, its size matches what we expect. This is what lets "launch"
/// double as "verify and repair" — re-running it against an intact install
/// downloads nothing.
async fn already_up_to_date(task: &DownloadTask) -> bool {
    if !task.destination.exists() {
        return false;
    }

    if let Some(expected_sha1) = &task.sha1 {
        return match sha1_of_file(&task.destination).await {
            Ok(actual) => actual.eq_ignore_ascii_case(expected_sha1),
            Err(_) => false,
        };
    }

    if let Some(expected_size) = task.expected_size {
        if let Ok(meta) = tokio::fs::metadata(&task.destination).await {
            return meta.len() == expected_size;
        }
        return false;
    }

    // No hash and no expected size to compare against — trust that its
    // presence means a previous run finished it.
    true
}

async fn try_download_once(
    client: &reqwest::Client,
    task: &DownloadTask,
    controller: &DownloadController,
    events: &UnboundedSender<DownloadEvent>,
    bytes_counter: &Arc<AtomicU64>,
) -> Result<(), DownloadError> {
    let part_path = part_path_for(&task.destination);

    if let Some(parent) = task.destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| io_err(task, e))?;
    }

    let mut resume_from = tokio::fs::metadata(&part_path)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0);

    // A `.part` file left over from an earlier, unrelated run (e.g. the
    // server's copy of this asset changed, or a previous bug produced a
    // corrupt partial) can end up larger than — or exactly equal to — the
    // file we're now expecting. Resuming from such an offset asks the
    // server for a byte range past the end of the resource, which is
    // answered with HTTP 416 (Range Not Satisfiable) on every retry,
    // since nothing about the request changes between attempts. Detecting
    // this up front and starting over avoids ever sending that invalid
    // request.
    if let Some(expected_size) = task.expected_size {
        if resume_from >= expected_size {
            tokio::fs::remove_file(&part_path).await.ok();
            resume_from = 0;
        }
    }

    let mut request = client.get(&task.url);
    if resume_from > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
    }

    let response = request
        .send()
        .await
        .map_err(|e| DownloadError::Request {
            url: task.url.clone(),
            source: e,
        })?;

    // Defensive fallback for the case above when `expected_size` wasn't
    // known ahead of time: if the server still rejects our range, the
    // partial file we had must be stale relative to what it's serving now.
    // Clear it so the very next retry (the existing retry loop in
    // `download_one_with_retry` will call this function again) starts a
    // clean, rangeless download instead of repeating the same bad request.
    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        tokio::fs::remove_file(&part_path).await.ok();
        return Err(DownloadError::HttpStatus {
            url: task.url.clone(),
            status: response.status().as_u16(),
        });
    }

    if !response.status().is_success() {
        return Err(DownloadError::HttpStatus {
            url: task.url.clone(),
            status: response.status().as_u16(),
        });
    }

    // Only treat this as a genuine resume if the server actually honored the
    // Range header (HTTP 206). Some servers/proxies ignore Range and send
    // the full body back with a 200 — in that case we must start over, or
    // we'd glue a complete file onto our stale partial one.
    let is_resuming = resume_from > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let starting_offset = if is_resuming { resume_from } else { 0 };

    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(is_resuming)
        .truncate(!is_resuming)
        .open(&part_path)
        .await
        .map_err(|e| io_err(task, e))?;

    let mut writer = BufWriter::new(file);
    let mut stream = response.bytes_stream();
    let mut downloaded_this_attempt = starting_offset;
    let mut last_emitted = downloaded_this_attempt;

    while let Some(chunk) = stream.next().await {
        if controller.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        controller.wait_if_paused().await;
        if controller.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        let chunk = chunk.map_err(|e| DownloadError::Request {
            url: task.url.clone(),
            source: e,
        })?;

        writer.write_all(&chunk).await.map_err(|e| io_err(task, e))?;

        downloaded_this_attempt += chunk.len() as u64;
        bytes_counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);

        if downloaded_this_attempt.saturating_sub(last_emitted) >= PROGRESS_EMIT_THRESHOLD_BYTES {
            let _ = events.send(DownloadEvent::Progress {
                label: task.label.clone(),
                bytes_downloaded: downloaded_this_attempt,
                total_bytes: task.expected_size,
            });
            last_emitted = downloaded_this_attempt;
        }
    }

    writer.flush().await.map_err(|e| io_err(task, e))?;
    drop(writer);

    if let Some(expected) = &task.sha1 {
        let actual = sha1_of_file(&part_path).await.map_err(|e| io_err(task, e))?;
        if !actual.eq_ignore_ascii_case(expected) {
            tokio::fs::remove_file(&part_path).await.ok();
            return Err(DownloadError::ChecksumMismatch {
                label: task.label.clone(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    tokio::fs::rename(&part_path, &task.destination)
        .await
        .map_err(|e| io_err(task, e))?;

    let _ = events.send(DownloadEvent::Progress {
        label: task.label.clone(),
        bytes_downloaded: downloaded_this_attempt,
        total_bytes: task.expected_size,
    });

    Ok(())
}

fn io_err(task: &DownloadTask, source: std::io::Error) -> DownloadError {
    DownloadError::Io {
        label: task.label.clone(),
        source,
    }
}

fn part_path_for(destination: &Path) -> PathBuf {
    let mut part = destination.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// A minimal local HTTP server isn't worth pulling in a dependency for
    /// here — instead these tests exercise the parts of the manager that
    /// don't require a live network: skip-if-correct, checksum mismatch
    /// handling, and the controller's pause/cancel semantics. End-to-end
    /// download behavior against real Mojang endpoints is covered by the
    /// `#[ignore]`-marked integration tests in the `minecraft` crate, which
    /// are meant to be run manually with network access.

    #[tokio::test]
    async fn already_up_to_date_accepts_matching_checksum_and_rejects_mismatch() {
        let dir = std::env::temp_dir().join(format!("dl-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("file.txt");
        tokio::fs::write(&path, b"hello world").await.unwrap();

        let correct_hash = sha1_of_file(&path).await.unwrap();

        let matching = DownloadTask::new("http://example.invalid/x", &path, "test").with_sha1(correct_hash);
        assert!(already_up_to_date(&matching).await);

        let mismatching =
            DownloadTask::new("http://example.invalid/x", &path, "test").with_sha1("deadbeef".repeat(5));
        assert!(!already_up_to_date(&mismatching).await);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[test]
    fn controller_pause_and_resume_toggle_state() {
        let controller = DownloadController::new();
        assert!(!controller.is_paused());
        controller.pause();
        assert!(controller.is_paused());
        controller.resume();
        assert!(!controller.is_paused());
    }

    #[tokio::test]
    async fn controller_wait_if_paused_returns_once_resumed() {
        let controller = DownloadController::new();
        controller.pause();

        let waiter = controller.clone();
        let handle = tokio::spawn(async move {
            waiter.wait_if_paused().await;
        });

        // Give the spawned task a moment to actually start waiting before
        // we resume, otherwise this race could pass for the wrong reason.
        tokio::time::sleep(Duration::from_millis(20)).await;
        controller.resume();

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("wait_if_paused should return promptly after resume()")
            .unwrap();
    }

    #[tokio::test]
    async fn run_with_no_tasks_completes_immediately() {
        let manager = DownloadManager::new(reqwest::Client::new(), 4);
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = manager.run(vec![], DownloadController::new(), tx).await;
        assert!(result.is_ok());
    }
}
