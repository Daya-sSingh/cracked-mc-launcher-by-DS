//! Generic, protocol-agnostic concurrent file downloader.
//!
//! This crate knows nothing about Minecraft, Mojang, or Modrinth — it just
//! moves a list of (url, destination, checksum) tuples to disk efficiently,
//! with resume, retry, and pause/cancel support. The `minecraft` crate (and
//! later the Modrinth/CurseForge integrations) build [`DownloadTask`] lists
//! and hand them to a [`DownloadManager`]; none of the protocol-specific
//! code needs to know how downloading actually works.

mod manager;
mod progress;
mod task;
pub mod verify;

pub use manager::{DownloadController, DownloadManager};
pub use progress::DownloadEvent;
pub use task::{DownloadError, DownloadTask};
