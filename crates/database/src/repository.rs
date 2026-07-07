use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::DatabaseError;
use crate::models::{Instance, InstanceDraft, InstanceRow, InstanceUpdate};

/// How a list of instances should be ordered for display. The "Library"
/// view in the UI lets the user flip between these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceSort {
    #[default]
    RecentlyPlayed,
    NameAscending,
    NameDescending,
    FavoritesFirst,
}

/// Storage operations for instances, expressed as a trait so the rest of the
/// app depends on this interface rather than on SQLite directly. Swapping in
/// a different backend (or a mock for testing) means writing one new impl,
/// not touching every call site.
#[async_trait]
pub trait InstanceRepository: Send + Sync {
    async fn create(&self, draft: InstanceDraft) -> Result<Instance, DatabaseError>;
    async fn get(&self, id: Uuid) -> Result<Instance, DatabaseError>;
    async fn list(&self, sort: InstanceSort) -> Result<Vec<Instance>, DatabaseError>;
    async fn update(&self, id: Uuid, update: InstanceUpdate) -> Result<Instance, DatabaseError>;
    async fn delete(&self, id: Uuid) -> Result<(), DatabaseError>;
    /// Records that an instance was just launched: bumps `last_played_at` to
    /// now and adds `session_seconds` to the running playtime total.
    async fn record_session(&self, id: Uuid, session_seconds: i64) -> Result<(), DatabaseError>;
}

pub struct SqliteInstanceRepository {
    pool: SqlitePool,
}

impl SqliteInstanceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn row_from(row: &sqlx::sqlite::SqliteRow) -> Result<InstanceRow, DatabaseError> {
        Ok(InstanceRow {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            loader: row.try_get("loader")?,
            loader_version: row.try_get("loader_version")?,
            minecraft_version: row.try_get("minecraft_version")?,
            icon: row.try_get("icon")?,
            group_name: row.try_get("group_name")?,
            favorite: row.try_get("favorite")?,
            java_path: row.try_get("java_path")?,
            java_args: row.try_get("java_args")?,
            memory_min_mb: row.try_get("memory_min_mb")?,
            memory_max_mb: row.try_get("memory_max_mb")?,
            window_width: row.try_get("window_width")?,
            window_height: row.try_get("window_height")?,
            fullscreen: row.try_get("fullscreen")?,
            game_args: row.try_get("game_args")?,
            last_played_at: row.try_get("last_played_at")?,
            total_playtime_seconds: row.try_get("total_playtime_seconds")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[async_trait]
impl InstanceRepository for SqliteInstanceRepository {
    async fn create(&self, draft: InstanceDraft) -> Result<Instance, DatabaseError> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO instances (
                id, name, loader, loader_version, minecraft_version, icon,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&draft.name)
        .bind(draft.loader.as_str())
        .bind(&draft.loader_version)
        .bind(&draft.minecraft_version)
        .bind(&draft.icon)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get(id).await
    }

    async fn get(&self, id: Uuid) -> Result<Instance, DatabaseError> {
        let row = sqlx::query("SELECT * FROM instances WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DatabaseError::InstanceNotFound(id))?;

        Instance::try_from(Self::row_from(&row)?)
    }

    async fn list(&self, sort: InstanceSort) -> Result<Vec<Instance>, DatabaseError> {
        let order_by = match sort {
            InstanceSort::RecentlyPlayed => {
                "ORDER BY last_played_at IS NULL, last_played_at DESC, created_at DESC"
            }
            InstanceSort::NameAscending => "ORDER BY name COLLATE NOCASE ASC",
            InstanceSort::NameDescending => "ORDER BY name COLLATE NOCASE DESC",
            InstanceSort::FavoritesFirst => {
                "ORDER BY favorite DESC, name COLLATE NOCASE ASC"
            }
        };

        let query = format!("SELECT * FROM instances {order_by}");
        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        rows.iter()
            .map(|row| Instance::try_from(Self::row_from(row)?))
            .collect()
    }

    async fn update(&self, id: Uuid, update: InstanceUpdate) -> Result<Instance, DatabaseError> {
        // Loaded first so every field has a fallback value — this turns the
        // "only touch what's `Some`" update into one straightforward SQL
        // statement instead of building dynamic SQL by hand.
        let current = self.get(id).await?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE instances SET
                name = ?, icon = ?, group_name = ?, favorite = ?,
                java_path = ?, java_args = ?, memory_min_mb = ?, memory_max_mb = ?,
                window_width = ?, window_height = ?, fullscreen = ?, game_args = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(update.name.unwrap_or(current.name))
        .bind(update.icon.or(current.icon))
        .bind(update.group_name.or(current.group_name))
        .bind(update.favorite.unwrap_or(current.favorite) as i64)
        .bind(update.java_path.or(current.java_path))
        .bind(update.java_args.or(current.java_args))
        .bind(update.memory_min_mb.unwrap_or(current.memory_min_mb) as i64)
        .bind(update.memory_max_mb.unwrap_or(current.memory_max_mb) as i64)
        .bind(update.window_width.unwrap_or(current.window_width) as i64)
        .bind(update.window_height.unwrap_or(current.window_height) as i64)
        .bind(update.fullscreen.unwrap_or(current.fullscreen) as i64)
        .bind(update.game_args.or(current.game_args))
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        self.get(id).await
    }

    async fn delete(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM instances WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::InstanceNotFound(id));
        }
        Ok(())
    }

    async fn record_session(&self, id: Uuid, session_seconds: i64) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE instances
            SET last_played_at = ?,
                total_playtime_seconds = total_playtime_seconds + ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&now)
        .bind(session_seconds.max(0))
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::InstanceNotFound(id));
        }
        Ok(())
    }
}

/// Storage for the small set of launcher-wide preferences (theme, accent
/// color, default JVM args, bandwidth limits, ...). Each value is an
/// arbitrary JSON blob the caller defines the shape of — this table only
/// guarantees persistence, not schema.
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>, DatabaseError>;
    async fn set(&self, key: &str, value_json: &str) -> Result<(), DatabaseError>;
}

pub struct SqliteSettingsRepository {
    pool: SqlitePool,
}

impl SqliteSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SettingsRepository for SqliteSettingsRepository {
    async fn get(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(match row {
            Some(row) => Some(row.try_get::<String, _>("value")?),
            None => None,
        })
    }

    async fn set(&self, key: &str, value_json: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
        )
        .bind(key)
        .bind(value_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Loader;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_then_get_round_trips_all_fields() {
        let repo = SqliteInstanceRepository::new(test_pool().await);

        let created = repo
            .create(InstanceDraft {
                name: "Survival World".into(),
                loader: Loader::Vanilla,
                loader_version: None,
                minecraft_version: "1.21.11".into(),
                icon: None,
            })
            .await
            .expect("create should succeed");

        let fetched = repo.get(created.id).await.expect("get should succeed");
        assert_eq!(fetched.name, "Survival World");
        assert_eq!(fetched.minecraft_version, "1.21.11");
        assert_eq!(fetched.loader, Loader::Vanilla);
        assert_eq!(fetched.memory_max_mb, 4096, "should fall back to the schema default");
        assert!(!fetched.favorite);
    }

    #[tokio::test]
    async fn update_only_touches_provided_fields() {
        let repo = SqliteInstanceRepository::new(test_pool().await);
        let created = repo
            .create(InstanceDraft {
                name: "Modded Adventure".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.9".into()),
                minecraft_version: "1.21.1".into(),
                icon: None,
            })
            .await
            .unwrap();

        let updated = repo
            .update(
                created.id,
                InstanceUpdate {
                    favorite: Some(true),
                    memory_max_mb: Some(8192),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(updated.favorite);
        assert_eq!(updated.memory_max_mb, 8192);
        // Untouched fields must survive the partial update.
        assert_eq!(updated.name, "Modded Adventure");
        assert_eq!(updated.loader_version, Some("0.16.9".to_string()));
    }

    #[tokio::test]
    async fn delete_removes_the_instance() {
        let repo = SqliteInstanceRepository::new(test_pool().await);
        let created = repo
            .create(InstanceDraft {
                name: "Throwaway".into(),
                loader: Loader::Vanilla,
                loader_version: None,
                minecraft_version: "1.20.6".into(),
                icon: None,
            })
            .await
            .unwrap();

        repo.delete(created.id).await.unwrap();
        let err = repo.get(created.id).await.unwrap_err();
        assert!(matches!(err, DatabaseError::InstanceNotFound(_)));
    }

    #[tokio::test]
    async fn delete_unknown_id_returns_not_found() {
        let repo = SqliteInstanceRepository::new(test_pool().await);
        let err = repo.delete(Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, DatabaseError::InstanceNotFound(_)));
    }

    #[tokio::test]
    async fn record_session_accumulates_playtime() {
        let repo = SqliteInstanceRepository::new(test_pool().await);
        let created = repo
            .create(InstanceDraft {
                name: "Speedrun Practice".into(),
                loader: Loader::Vanilla,
                loader_version: None,
                minecraft_version: "1.16.1".into(),
                icon: None,
            })
            .await
            .unwrap();

        repo.record_session(created.id, 120).await.unwrap();
        repo.record_session(created.id, 45).await.unwrap();

        let fetched = repo.get(created.id).await.unwrap();
        assert_eq!(fetched.total_playtime_seconds, 165);
        assert!(fetched.last_played_at.is_some());
    }

    #[tokio::test]
    async fn settings_round_trip_and_upsert() {
        let pool = test_pool().await;
        let repo = SqliteSettingsRepository::new(pool);

        assert_eq!(repo.get("theme").await.unwrap(), None);

        repo.set("theme", r#"{"mode":"dark"}"#).await.unwrap();
        assert_eq!(repo.get("theme").await.unwrap(), Some(r#"{"mode":"dark"}"#.to_string()));

        // Setting the same key again should overwrite, not duplicate.
        repo.set("theme", r#"{"mode":"light"}"#).await.unwrap();
        assert_eq!(repo.get("theme").await.unwrap(), Some(r#"{"mode":"light"}"#.to_string()));
    }
}
