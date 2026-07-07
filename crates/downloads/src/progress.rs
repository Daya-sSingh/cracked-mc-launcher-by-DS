/// Per-file and aggregate progress, emitted while a [`crate::DownloadManager`]
/// works through a batch of tasks. Consumers (Tauri commands today) forward
/// these to the frontend largely as-is.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Started {
        label: String,
        total_bytes: Option<u64>,
    },
    Progress {
        label: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    /// File was already present on disk with a matching checksum — nothing
    /// was transferred. Distinguished from `Completed` so the UI can show
    /// "verified" instead of implying a download just happened.
    Skipped {
        label: String,
    },
    Retrying {
        label: String,
        attempt: u32,
        error: String,
    },
    Completed {
        label: String,
    },
    Failed {
        label: String,
        error: String,
    },
    /// Batch-wide snapshot, emitted on a timer rather than per-file, so the
    /// UI can show one smooth overall progress bar + speed readout instead
    /// of needing to reconstruct that from hundreds of small file events.
    AggregateProgress {
        completed_tasks: usize,
        total_tasks: usize,
        bytes_downloaded: u64,
        total_bytes: u64,
        bytes_per_sec: f64,
    },
}
