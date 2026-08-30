//! PostgreSQL access.
//!
//! The pool is bounded and every health probe is time-boxed: a database
//! that has become slow must surface as an unready server, never as an
//! unbounded queue of waiting requests.

use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::DatabaseConfig;

/// Migrations are embedded in the binary so a deployment cannot run
/// against a schema from a different build.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// Upper bound for a readiness probe query. Shorter than the acquire
/// timeout so a probe cannot outlive the readiness request itself.
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database connection failed")]
    Connect(#[source] sqlx::Error),
    #[error("database migration failed")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    #[error("database is not responding")]
    Unavailable,
}

/// Opens a bounded connection pool. Connections are established lazily so
/// the server can start and report "not ready" while the database is
/// still coming up, instead of crash-looping next to it.
pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .connect_lazy(config.database_url())
        .map_err(DatabaseError::Connect)
}

/// Applies pending migrations. Safe to run repeatedly; already-applied
/// migrations are skipped, so restarting a deployment never reinitialises
/// existing data.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseError> {
    MIGRATOR.run(pool).await.map_err(DatabaseError::Migrate)
}

/// Time-boxed liveness check of the database connection.
pub async fn check_health(pool: &PgPool) -> Result<(), DatabaseError> {
    let probe = sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool);

    match tokio::time::timeout(HEALTH_CHECK_TIMEOUT, probe).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "database health check failed");
            Err(DatabaseError::Unavailable)
        }
        Err(_elapsed) => {
            tracing::warn!(
                timeout_ms = HEALTH_CHECK_TIMEOUT.as_millis() as u64,
                "database health check timed out"
            );
            Err(DatabaseError::Unavailable)
        }
    }
}
