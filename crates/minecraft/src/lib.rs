//! Everything needed to go from "a version id and an account" to "a running
//! Minecraft process" — version manifest lookups, asset/library resolution,
//! Java detection/auto-install, and the launch itself.
//!
//! This crate is intentionally Mojang/vanilla-and-Fabric-only; it knows
//! nothing about Tauri, SQLite, or the UI. `src-tauri` is the only thing
//! that imports it directly, translating between this crate's types and
//! the `database` crate's `Instance` model.

pub mod assets;
mod error;
pub mod fabric;
pub mod java;
pub mod launch;
pub mod libraries;
pub mod manifest;
pub mod os_match;
pub mod paths;
pub mod version_detail;

pub use error::MinecraftError;
pub use launch::{
    AccountType, GameAccount, LaunchEvent, LaunchHandle, LaunchRequest, LaunchStage, Loader,
};
pub use manifest::{
    fetch_version_manifest, LatestVersions, VersionManifest, VersionSummary, VersionType,
};
pub use paths::LauncherPaths;
