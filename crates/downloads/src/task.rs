use std::path::PathBuf;

use thiserror::Error;

/// A single file to fetch. `label` is purely cosmetic (shown in the UI's
/// progress list) — everything that affects correctness is one of the other
/// fields.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub destination: PathBuf,
    /// Expected lowercase hex SHA-1, when the source provides one (Mojang's
    /// manifests always do). Used both to skip re-downloading files that are
    /// already correct on disk, and to detect corruption after a download.
    pub sha1: Option<String>,
    /// Expected size in bytes, if known. Used for progress percentage before
    /// the response headers arrive, and as a sanity check after.
    pub expected_size: Option<u64>,
    pub label: String,
}

impl DownloadTask {
    pub fn new(
        url: impl Into<String>,
        destination: impl Into<PathBuf>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            destination: destination.into(),
            sha1: None,
            expected_size: None,
            label: label.into(),
        }
    }

    pub fn with_sha1(mut self, sha1: impl Into<String>) -> Self {
        self.sha1 = Some(sha1.into());
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("download was cancelled")]
    Cancelled,

    #[error("network request failed for {url}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("server returned HTTP {status} for {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("checksum mismatch for {label}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        label: String,
        expected: String,
        actual: String,
    },

    #[error("filesystem error while downloading {label}")]
    Io {
        label: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{label} failed after {attempts} attempts: {last_error}")]
    RetriesExhausted {
        label: String,
        attempts: u32,
        last_error: String,
    },

    #[error("{failed_count} of {total_count} downloads failed; first error: {first_message}")]
    PartialFailure {
        failed_count: usize,
        total_count: usize,
        first_message: String,
    },
}
