//! SQLite-backed persistence for the launcher.
//!
//! This crate owns the schema, migrations, and repository implementations.
//! Nothing outside `database` should write raw SQL — callers (Tauri commands,
//! other crates) talk to [`InstanceRepository`] / [`SettingsRepository`] and
//! never see a `sqlx::Pool` directly. That keeps the storage engine swappable
//! and keeps query logic in one place instead of scattered across the app.

mod error;
pub mod models;
pub mod repository;

use std::path::Path;

pub use error::DatabaseError;
pub use models::{Instance, InstanceDraft, InstanceUpdate, Loader};
pub use repository::{
    InstanceRepository, InstanceSort, SettingsRepository, SqliteInstanceRepository,
    SqliteSettingsRepository,
};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;

/// Embeds every `.sql` file in `migrations/` at compile time and applies the
/// ones a given database hasn't seen yet, in order. Safe to call on every
/// app start — already-applied migrations are skipped.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Opens (creating if necessary) the SQLite database at `db_path` and brings
/// its schema up to date. Call this once at startup and share the resulting
/// pool through app state.
pub async fn init_pool(db_path: &Path) -> Result<sqlx::SqlitePool, DatabaseError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(DatabaseError::Io)?;
    }

    let options = SqliteConnectOptions::from_str(&db_path.to_string_lossy())
        .map_err(DatabaseError::Connect)?
        .create_if_missing(true)
        // WAL gives us concurrent reads while a write is in flight, which
        // matters once the download manager and UI are both touching the
        // database at the same time.
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .map_err(DatabaseError::Connect)?;

    MIGRATOR.run(&pool).await.map_err(DatabaseError::Migrate)?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_pool_creates_schema_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("launcher-test-{}", uuid::Uuid::new_v4()));
        let db_path = dir.join("launcher.db");

        let pool = init_pool(&db_path)
            .await
            .expect("first init should succeed");
        drop(pool);

        // Re-opening (and re-running migrations against) the same file must
        // not error — this is the path every normal app start takes.
        let pool = init_pool(&db_path)
            .await
            .expect("second init should succeed");
        drop(pool);

        std::fs::remove_dir_all(&dir).ok();
    }
}
