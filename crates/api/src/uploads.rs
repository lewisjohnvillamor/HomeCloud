//! Resumable uploads.
//!
//! One request per file is fine for a photo and hopeless for a 40 GB
//! video over house wifi: a single dropped connection and the whole
//! thing starts again. A session records where an upload had got to, so
//! a client that comes back asks "how much did you get?" and continues.
//!
//! Two rules make this safe. The offset is read from the staging file's
//! own length rather than from anything a client sends, so a client
//! cannot claim to be further along than it is and leave a hole in the
//! middle of a file. And the destination is checked again at completion,
//! because a name can be taken while an upload is in flight.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::Json;
use futures::StreamExt;
use homecloud_domain::identity::{LibraryId, UserId};
use homecloud_storage::MutableStorage;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::items::{parse_path, record_uploaded_file, storage_error};
use crate::library::{authorize, parse_library, storage_for};
use crate::view::ItemView;

/// Largest resumable upload. Far beyond a single request's limit,
/// because being able to send a very large file is the entire point.
pub const MAX_RESUMABLE_BYTES: u64 = 512 * 1024 * 1024 * 1024;

/// Largest single chunk accepted. Bounds what one request can cost while
/// still being large enough that a big file is not thousands of round
/// trips.
pub const MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;

/// How long an unfinished upload is kept. Long enough to survive a
/// laptop being closed overnight, short enough that abandoned bytes do
/// not sit on the disk forever.
const SESSION_TTL_HOURS: i64 = 48;

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
pub struct CreateUploadRequest {
    pub library_id: String,
    /// Destination path, including the file name.
    pub path: String,
    /// Total size of the file, so an upload that cannot possibly fit is
    /// refused before any bytes are sent.
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct UploadSessionView {
    pub id: String,
    pub path: String,
    /// Where to continue from. Read from the staging file itself.
    pub offset: u64,
    pub size_bytes: i64,
    /// Largest chunk this server will accept in one request.
    pub max_chunk_bytes: u64,
    pub expires_at: String,
}

/// `POST /api/v1/uploads` — open a session.
pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<CreateUploadRequest>,
) -> Result<Json<UploadSessionView>, ApiError> {
    let library = parse_library(&request.library_id)?;
    authorize(&state, user, library).await?;

    if request.size_bytes > MAX_RESUMABLE_BYTES {
        return Err(ApiError::new(
            crate::error::ErrorCode::PayloadTooLarge,
            "That file is larger than this server accepts.",
        ));
    }

    // Validated now so a client learns about a bad path before sending
    // gigabytes, and again at completion because names can be taken.
    let destination = parse_path(&request.path)?;

    let staging_name = format!("session-{}", uuid::Uuid::new_v4().simple());
    let expires_at = OffsetDateTime::now_utc() + Duration::hours(SESSION_TTL_HOURS);

    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO upload_sessions
            (library_id, created_by, destination_path, declared_size, staging_name, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .bind(destination.to_string())
    .bind(request.size_bytes as i64)
    .bind(&staging_name)
    .bind(expires_at)
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not open an upload session");
        ApiError::internal()
    })?;

    Ok(Json(UploadSessionView {
        id: id.to_string(),
        path: destination.to_string(),
        offset: 0,
        size_bytes: request.size_bytes as i64,
        max_chunk_bytes: MAX_CHUNK_BYTES,
        expires_at: rfc3339(expires_at),
    }))
}

/// `GET /api/v1/uploads/{id}` — how much arrived?
///
/// The answer comes from the staging file, not from the database: the
/// bytes on disk are the truth about what survived a dropped connection.
pub async fn status(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<UploadSessionView>, ApiError> {
    let session = load(&state, user, &id).await?;
    let storage = storage_for(&state, session.library).await?;
    let offset = storage.staged_bytes(&session.staging_name).await;

    Ok(Json(UploadSessionView {
        id: session.id.to_string(),
        path: session.destination_path,
        offset,
        size_bytes: session.declared_size,
        max_chunk_bytes: MAX_CHUNK_BYTES,
        expires_at: rfc3339(session.expires_at),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ChunkQuery {
    /// Where the client believes it is continuing from. Checked against
    /// the file rather than trusted.
    pub offset: u64,
}

/// `PATCH /api/v1/uploads/{id}?offset=` — append a chunk.
///
/// A client that is behind or ahead is told the real offset rather than
/// being allowed to write at the wrong place: appending at a wrong
/// offset is how a resumable upload silently corrupts a file.
pub async fn append(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Query(query): Query<ChunkQuery>,
    body: Body,
) -> Result<Json<UploadSessionView>, ApiError> {
    let session = load(&state, user, &id).await?;
    let storage = storage_for(&state, session.library).await?;

    let offset = storage.staged_bytes(&session.staging_name).await;
    if query.offset != offset {
        return Err(ApiError::conflict(format!(
            "This upload is at {offset} bytes. Continue from there."
        )));
    }

    let declared = session.declared_size.max(0) as u64;
    let remaining = declared.saturating_sub(offset);

    let mut staged = storage
        .resume_upload(&session.staging_name)
        .await
        .map_err(storage_error)?
        // Never accept more than the file was declared to be: a client
        // that keeps sending must not be able to fill the disk.
        .with_limit(declared.min(MAX_RESUMABLE_BYTES));

    let mut stream = body.into_data_stream();
    let mut written = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::debug!(error = %error, "an upload chunk was cut short");
                // Whatever arrived is kept: that is the whole point of a
                // resumable upload. The client asks again and continues.
                break;
            }
        };

        written = written.saturating_add(chunk.len() as u64);
        if written > MAX_CHUNK_BYTES || written > remaining {
            return Err(ApiError::new(
                crate::error::ErrorCode::PayloadTooLarge,
                "That chunk is larger than this server accepts.",
            ));
        }

        if let Err(error) = staged.write_chunk(&chunk).await {
            return Err(storage_error(error));
        }
    }

    // Durable before acknowledged: the client is about to be told this
    // much arrived, and will continue from there rather than send it
    // again.
    staged.persist().await.map_err(storage_error)?;

    let offset = storage.staged_bytes(&session.staging_name).await;

    sqlx::query("UPDATE upload_sessions SET received_bytes = $2, updated_at = now() WHERE id = $1")
        .bind(session.id)
        .bind(offset as i64)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not record upload progress");
            ApiError::dependency_unavailable("database")
        })?;

    Ok(Json(UploadSessionView {
        id: session.id.to_string(),
        path: session.destination_path,
        offset,
        size_bytes: session.declared_size,
        max_chunk_bytes: MAX_CHUNK_BYTES,
        expires_at: rfc3339(session.expires_at),
    }))
}

/// `POST /api/v1/uploads/{id}/complete` — move the file into place.
///
/// Refuses an upload that is not all there: a file that is short is a
/// broken file, and putting it in someone's library as though it were
/// finished is worse than making them send it again.
pub async fn complete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<ItemView>, ApiError> {
    let session = load(&state, user, &id).await?;
    let storage = storage_for(&state, session.library).await?;

    let offset = storage.staged_bytes(&session.staging_name).await;
    let declared = session.declared_size.max(0) as u64;

    if offset != declared {
        return Err(ApiError::conflict(format!(
            "This upload has {offset} of {declared} bytes. Send the rest first."
        )));
    }

    let requested = parse_path(&session.destination_path)?;
    // The name is chosen now, not when the session opened: something
    // else may have taken it during a long upload.
    let destination = storage
        .available_path(&requested)
        .await
        .map_err(storage_error)?;

    let staged = storage
        .resume_upload(&session.staging_name)
        .await
        .map_err(storage_error)?;

    storage
        .finish_upload(staged, &destination)
        .await
        .map_err(storage_error)?;

    sqlx::query("UPDATE upload_sessions SET completed_at = now() WHERE id = $1")
        .bind(session.id)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not close an upload session");
            ApiError::dependency_unavailable("database")
        })?;

    let item = record_uploaded_file(&state, session.library, &destination).await?;

    tracing::info!(bytes = offset, "a resumable upload finished");

    Ok(Json(ItemView::from(&item)))
}

/// `DELETE /api/v1/uploads/{id}` — give up on an upload.
pub async fn abort(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = load(&state, user, &id).await?;
    let storage = storage_for(&state, session.library).await?;

    // The bytes go first: a session row without its staging file is
    // recoverable, a staging file nobody remembers is not.
    storage.discard_staged(&session.staging_name).await;

    sqlx::query("DELETE FROM upload_sessions WHERE id = $1")
        .bind(session.id)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not remove an upload session");
            ApiError::dependency_unavailable("database")
        })?;

    Ok(Json(serde_json::json!({ "aborted": true })))
}

/// `GET /api/v1/libraries/{library}/uploads` — what is unfinished.
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<UploadSessionView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let rows: Vec<(uuid::Uuid, String, i64, i64, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, destination_path, received_bytes, declared_size, expires_at
         FROM upload_sessions
         WHERE library_id = $1 AND created_by = $2
           AND completed_at IS NULL AND expires_at > now()
         ORDER BY updated_at DESC",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not list upload sessions");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, path, received, declared, expires_at)| UploadSessionView {
                    id: id.to_string(),
                    path,
                    offset: received.max(0) as u64,
                    size_bytes: declared,
                    max_chunk_bytes: MAX_CHUNK_BYTES,
                    expires_at: rfc3339(expires_at),
                },
            )
            .collect(),
    ))
}

/// One upload session, as far as its owner is concerned.
struct Session {
    id: uuid::Uuid,
    library: LibraryId,
    destination_path: String,
    declared_size: i64,
    staging_name: String,
    expires_at: OffsetDateTime,
}

/// Loads a session belonging to this person.
///
/// Someone else's session is a "not found", including another member of
/// the same library: an upload in progress is not shared work, and its
/// staging file is not something anyone else should be able to append
/// to.
async fn load(state: &AppState, user: UserId, id: &str) -> Result<Session, ApiError> {
    let id = uuid::Uuid::parse_str(id).map_err(|_| ApiError::not_found())?;

    let row: Option<(uuid::Uuid, uuid::Uuid, String, i64, String, OffsetDateTime)> =
        sqlx::query_as(
            "SELECT id, library_id, destination_path, declared_size, staging_name, expires_at
             FROM upload_sessions
             WHERE id = $1 AND created_by = $2
               AND completed_at IS NULL AND expires_at > now()",
        )
        .bind(id)
        .bind(user.as_uuid())
        .fetch_optional(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "upload session lookup failed");
            ApiError::dependency_unavailable("database")
        })?;

    let Some((id, library, destination_path, declared_size, staging_name, expires_at)) = row else {
        return Err(ApiError::not_found());
    };

    Ok(Session {
        id,
        library: LibraryId::from_uuid(library),
        destination_path,
        declared_size,
        staging_name,
        expires_at,
    })
}

/// Removes sessions nobody finished, and the bytes they were holding.
///
/// Unlike the other sweeps this one has to touch the disk: an abandoned
/// upload is bytes, not just a row.
pub async fn purge_expired(pool: &PgPool, state: &AppState) -> Result<u64, sqlx::Error> {
    let expired: Vec<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
        "DELETE FROM upload_sessions
         WHERE completed_at IS NULL AND expires_at <= now()
         RETURNING id, library_id, staging_name",
    )
    .fetch_all(pool)
    .await?;

    for (_, library, staging_name) in &expired {
        if let Ok(storage) = storage_for(state, LibraryId::from_uuid(*library)).await {
            storage.discard_staged(staging_name).await;
        }
    }

    Ok(expired.len() as u64)
}
