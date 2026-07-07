use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DatabaseError;

/// The mod loaders the launcher knows how to install and launch today.
///
/// Deliberately not `Forge | NeoForge | Quilt` — see `docs/ARCHITECTURE.md`
/// for why, and for the extension point that lets those be added later
/// without touching this enum's callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
}

impl Loader {
    pub fn as_str(&self) -> &'static str {
        match self {
            Loader::Vanilla => "vanilla",
            Loader::Fabric => "fabric",
        }
    }
}

impl FromStr for Loader {
    type Err = DatabaseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "vanilla" => Ok(Loader::Vanilla),
            "fabric" => Ok(Loader::Fabric),
            other => Err(DatabaseError::InvalidLoader(other.to_string())),
        }
    }
}

/// A fully-loaded instance as the rest of the app sees it. Constructed only
/// by [`crate::repository`] from database rows — there is no public
/// constructor, so an `Instance` value is always something that round-trips
/// through storage cleanly.
#[derive(Debug, Clone, Serialize)]
pub struct Instance {
    pub id: Uuid,
    pub name: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub minecraft_version: String,
    pub icon: Option<String>,
    pub group_name: Option<String>,
    pub favorite: bool,

    pub java_path: Option<String>,
    pub java_args: Option<String>,
    pub memory_min_mb: u32,
    pub memory_max_mb: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub game_args: Option<String>,

    pub last_played_at: Option<DateTime<Utc>>,
    pub total_playtime_seconds: i64,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What's needed to create a brand new instance. Every field not listed here
/// gets a sensible default (see the `instances` table's `DEFAULT` clauses).
#[derive(Debug, Clone, Deserialize)]
pub struct InstanceDraft {
    pub name: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub minecraft_version: String,
    pub icon: Option<String>,
}

/// A partial update: every field is optional, and only the `Some(_)` ones
/// are written. This mirrors what the settings panel in the UI lets a user
/// change about a single instance.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstanceUpdate {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub group_name: Option<String>,
    pub favorite: Option<bool>,
    pub java_path: Option<String>,
    pub java_args: Option<String>,
    pub memory_min_mb: Option<u32>,
    pub memory_max_mb: Option<u32>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub game_args: Option<String>,
}

/// Raw shape of a row from the `instances` table — every column as the type
/// SQLite hands back, with no domain validation applied yet. Converting this
/// into [`Instance`] is where we parse UUIDs, timestamps, and the loader
/// enum, and is the only place that conversion logic lives.
pub(crate) struct InstanceRow {
    pub id: String,
    pub name: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub minecraft_version: String,
    pub icon: Option<String>,
    pub group_name: Option<String>,
    pub favorite: i64,
    pub java_path: Option<String>,
    pub java_args: Option<String>,
    pub memory_min_mb: i64,
    pub memory_max_mb: i64,
    pub window_width: i64,
    pub window_height: i64,
    pub fullscreen: i64,
    pub game_args: Option<String>,
    pub last_played_at: Option<String>,
    pub total_playtime_seconds: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<InstanceRow> for Instance {
    type Error = DatabaseError;

    fn try_from(row: InstanceRow) -> Result<Self, Self::Error> {
        let parse_ts = |s: &str| -> Result<DateTime<Utc>, DatabaseError> {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DatabaseError::InvalidTimestamp(s.to_string()))
        };

        Ok(Instance {
            id: Uuid::parse_str(&row.id).map_err(|_| DatabaseError::InvalidId(row.id.clone()))?,
            name: row.name,
            loader: Loader::from_str(&row.loader)?,
            loader_version: row.loader_version,
            minecraft_version: row.minecraft_version,
            icon: row.icon,
            group_name: row.group_name,
            favorite: row.favorite != 0,
            java_path: row.java_path,
            java_args: row.java_args,
            memory_min_mb: row.memory_min_mb as u32,
            memory_max_mb: row.memory_max_mb as u32,
            window_width: row.window_width as u32,
            window_height: row.window_height as u32,
            fullscreen: row.fullscreen != 0,
            game_args: row.game_args,
            last_played_at: row.last_played_at.as_deref().map(parse_ts).transpose()?,
            total_playtime_seconds: row.total_playtime_seconds,
            created_at: parse_ts(&row.created_at)?,
            updated_at: parse_ts(&row.updated_at)?,
        })
    }
}
