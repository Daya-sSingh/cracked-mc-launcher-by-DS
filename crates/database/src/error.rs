use thiserror::Error;

/// Every fallible operation in this crate returns one of these. Kept
/// `Serialize`-free on purpose — callers (Tauri commands) decide how much of
/// the underlying error to expose to the UI rather than leaking SQL details
/// straight to the frontend.
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("failed to open database connection")]
    Connect(#[source] sqlx::Error),

    #[error("failed to run database migrations")]
    Migrate(#[source] sqlx::migrate::MigrateError),

    #[error("database query failed")]
    Query(#[source] sqlx::Error),

    #[error("could not prepare database directory")]
    Io(#[source] std::io::Error),

    #[error("instance {0} was not found")]
    InstanceNotFound(uuid::Uuid),

    #[error("'{0}' is not a recognized mod loader")]
    InvalidLoader(String),

    #[error("stored timestamp '{0}' could not be parsed")]
    InvalidTimestamp(String),

    #[error("stored id '{0}' is not a valid UUID")]
    InvalidId(String),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        DatabaseError::Query(err)
    }
}
