//! Liveness and readiness endpoints.
//!
//! Liveness answers "is this process running"; readiness answers "can it
//! serve traffic right now". They are separate because restarting a
//! process whose database is briefly unavailable makes an outage worse.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::app::AppState;
use crate::db;
use crate::error::ApiError;

#[derive(Debug, Serialize)]
pub struct HealthBody {
    status: &'static str,
}

/// Liveness never touches the database: a dependency failure must not be
/// reported as a dead process.
pub async fn live() -> Response {
    Json(HealthBody { status: "ok" }).into_response()
}

/// Readiness performs a bounded database probe. The response body never
/// includes driver messages, hostnames, or credentials.
pub async fn ready(State(state): State<AppState>) -> Result<Json<HealthBody>, ApiError> {
    db::check_health(state.db())
        .await
        .map_err(|_| ApiError::dependency_unavailable("database"))?;

    Ok(Json(HealthBody { status: "ready" }))
}
