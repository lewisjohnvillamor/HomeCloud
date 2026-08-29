//! First-run bootstrap.
//!
//! A fresh deployment has no accounts. Exactly one owner may be created,
//! and the database — not application code — is the authority on that,
//! so two simultaneous requests cannot produce two owners.

use axum::extract::State;
use axum::Json;
use homecloud_domain::identity::{LibraryId, UserId};
use homecloud_domain::library::LibraryRole;
use homecloud_domain::naming::LibraryName;
use serde::Serialize;
use sqlx::PgPool;

use crate::app::AppState;
use crate::error::ApiError;

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("this deployment already has an owner")]
    AlreadyInitialised,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Whether this deployment still needs its first account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapState {
    /// No owner exists; the first-run flow applies.
    Uninitialised,
    Initialised,
}

impl BootstrapState {
    pub fn needs_owner(self) -> bool {
        matches!(self, BootstrapState::Uninitialised)
    }
}

/// The owner account and its initial library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub user: UserId,
    pub library: LibraryId,
}

pub async fn state(pool: &PgPool) -> Result<BootstrapState, BootstrapError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE is_deployment_owner)")
            .fetch_one(pool)
            .await?;

    Ok(if exists {
        BootstrapState::Initialised
    } else {
        BootstrapState::Uninitialised
    })
}

/// Creates the owner account and its first library in one short
/// transaction. No filesystem, network, or model work happens inside it.
pub async fn create_owner(
    pool: &PgPool,
    display_name: &str,
    library_name: &LibraryName,
) -> Result<Owner, BootstrapError> {
    let mut tx = pool.begin().await?;

    // `ON CONFLICT DO NOTHING` against the partial unique index makes the
    // race explicit: the loser gets no row back and is told the
    // deployment is already initialised.
    let user: Option<uuid::Uuid> = sqlx::query_scalar(
        "INSERT INTO users (display_name, is_deployment_owner)
         VALUES ($1, TRUE)
         ON CONFLICT (is_deployment_owner) WHERE is_deployment_owner DO NOTHING
         RETURNING id",
    )
    .bind(display_name)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(user) = user else {
        tx.rollback().await?;
        return Err(BootstrapError::AlreadyInitialised);
    };

    let library: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO libraries (name) VALUES ($1) RETURNING id")
            .bind(library_name.as_str())
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query("INSERT INTO library_members (library_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(library)
        .bind(user)
        .bind(LibraryRole::Owner.as_str())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Owner {
        user: UserId::from_uuid(user),
        library: LibraryId::from_uuid(library),
    })
}

/// First-run status as the web client sees it.
#[derive(Debug, Serialize)]
pub struct BootstrapStatus {
    /// True while the deployment still has no owner, which is what the
    /// first-run screen keys off.
    needs_owner: bool,
}

/// `GET /api/v1/bootstrap`
pub async fn status(State(state): State<AppState>) -> Result<Json<BootstrapStatus>, ApiError> {
    let state = self::state(state.db()).await.map_err(|error| match error {
        BootstrapError::Database(error) => {
            tracing::warn!(error = %error, "bootstrap status query failed");
            ApiError::dependency_unavailable("database")
        }
        other => {
            tracing::error!(error = %other, "unexpected bootstrap error");
            ApiError::internal()
        }
    })?;

    Ok(Json(BootstrapStatus {
        needs_owner: state.needs_owner(),
    }))
}
