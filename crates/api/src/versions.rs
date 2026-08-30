//! Previous contents of a file.
//!
//! Deliberately narrow. HomeCloud can only keep a version of a change it
//! made itself: replacing a file through the app moves the old bytes
//! aside first. When someone edits a file with another program, the old
//! contents are gone before any scan notices it changed, and a version
//! list that quietly missed those would be worse than one that never
//! claimed to have them.
//!
//! Nothing here copies anything. Replacing moves the current file into
//! the version store and the new one into its place; restoring does the
//! same in reverse, keeping what was current as a version of its own —
//! so a restore is never a way to lose the thing you had.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use futures::StreamExt;
use homecloud_catalog::repository;
use homecloud_domain::identity::{ItemId, UserId};
use serde::Serialize;
use time::OffsetDateTime;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::items::storage_error;
use crate::library::{catalog_error, parse_item, storage_for};
use crate::transfers::MAX_UPLOAD_BYTES;
use crate::view::ItemView;

/// One row of a file's history, as the database returns it.
type VersionRow = (
    uuid::Uuid,
    i64,
    Option<String>,
    Option<OffsetDateTime>,
    OffsetDateTime,
);

/// Most versions kept for one file. Past this the oldest is discarded,
/// so a file edited every minute cannot fill a disk on its own.
const MAX_VERSIONS_PER_ITEM: i64 = 50;

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
pub struct VersionView {
    pub id: String,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    /// When this content was last written, before it was replaced.
    pub content_modified_at: Option<String>,
    /// When it stopped being the current content.
    pub replaced_at: String,
}

/// `PUT /api/v1/items/{item}/content` — replace a file's contents.
///
/// The body is the new file. The old contents become a version rather
/// than being overwritten, which is the difference between a product
/// that keeps history and one that merely says it does.
pub async fn replace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    body: Body,
) -> Result<Json<ItemView>, ApiError> {
    let item = readable(&state, user, &item).await?;

    if item.is_folder() {
        return Err(ApiError::bad_request(
            "A folder has no contents to replace.",
        ));
    }
    if item.trashed_at.is_some() || item.missing_since.is_some() {
        return Err(ApiError::conflict(
            "Restore this file before replacing its contents.",
        ));
    }

    let storage = storage_for(&state, item.library).await?;

    // The new contents are staged first: if the upload fails there must
    // still be a file where the old one was.
    let mut staged = storage
        .begin_upload(MAX_UPLOAD_BYTES)
        .await
        .map_err(storage_error)?;

    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::debug!(error = %error, "a replacement upload was cut short");
                staged.abort().await;
                return Err(ApiError::bad_request("The upload did not complete."));
            }
        };

        if let Err(error) = staged.write_chunk(&chunk).await {
            staged.abort().await;
            return Err(storage_error(error));
        }
    }

    // Only now is the old file moved aside, leaving the path free for
    // the replacement.
    let kept = storage.keep_version(&item.path).await.map_err(|error| {
        tracing::warn!(error = %error, "could not keep a version");
        storage_error(error)
    })?;

    if let Err(error) = storage.finish_upload(staged, &item.path).await {
        // Put the original back rather than leaving a gap where a file
        // used to be.
        let _ = storage.restore_version(&kept, &item.path).await;

        return Err(storage_error(error));
    }

    record(&state, item.id, &item, &kept, user).await?;
    prune(&state, item.id, &storage).await;

    let refreshed = refresh(&state, &item).await?;

    tracing::info!("a file's contents were replaced");

    Ok(Json(ItemView::from(&refreshed)))
}

/// `GET /api/v1/items/{item}/versions`
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<Vec<VersionView>>, ApiError> {
    let item = readable(&state, user, &item).await?;

    let rows: Vec<VersionRow> = sqlx::query_as(
        "SELECT id, size_bytes, content_type, content_modified_at, created_at
             FROM content_versions
             WHERE item_id = $1
             ORDER BY created_at DESC",
    )
    .bind(item.id.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not list versions");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, size, content_type, modified, created)| VersionView {
                id: id.to_string(),
                size_bytes: size,
                content_type,
                content_modified_at: modified.map(rfc3339),
                replaced_at: rfc3339(created),
            })
            .collect(),
    ))
}

/// `GET /api/v1/items/{item}/versions/{version}/content`
pub async fn download(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((item, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let item = readable(&state, user, &item).await?;
    let version = load_version(&state, item.id, &version).await?;
    let storage = storage_for(&state, item.library).await?;

    let (file, size) = storage
        .open_version(&version.storage_name)
        .await
        .map_err(storage_error)?;

    crate::transfers::stream_reader(
        file,
        size,
        &format!("{} (earlier version)", item.name),
        version.content_type.as_deref(),
        &headers,
    )
    .await
}

/// `POST /api/v1/items/{item}/versions/{version}/restore`
///
/// Puts an earlier version back, keeping what was current as a version
/// of its own: a restore must never be a way to lose the file you had.
pub async fn restore(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((item, version)): Path<(String, String)>,
) -> Result<Json<ItemView>, ApiError> {
    let item = readable(&state, user, &item).await?;
    let version = load_version(&state, item.id, &version).await?;
    let storage = storage_for(&state, item.library).await?;

    if item.trashed_at.is_some() || item.missing_since.is_some() {
        return Err(ApiError::conflict(
            "Restore this file before putting an earlier version back.",
        ));
    }

    let kept = storage
        .keep_version(&item.path)
        .await
        .map_err(storage_error)?;

    if let Err(error) = storage
        .restore_version(&version.storage_name, &item.path)
        .await
    {
        let _ = storage.restore_version(&kept, &item.path).await;

        return Err(storage_error(error));
    }

    // The version just put back is no longer history, and what it
    // replaced now is.
    sqlx::query("DELETE FROM content_versions WHERE id = $1")
        .bind(version.id)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not close a restored version");
            ApiError::dependency_unavailable("database")
        })?;

    record(&state, item.id, &item, &kept, user).await?;

    let refreshed = refresh(&state, &item).await?;

    tracing::info!("an earlier version of a file was restored");

    Ok(Json(ItemView::from(&refreshed)))
}

/// One kept version, as far as the database is concerned.
struct Version {
    id: uuid::Uuid,
    storage_name: String,
    content_type: Option<String>,
}

async fn load_version(state: &AppState, item: ItemId, version: &str) -> Result<Version, ApiError> {
    let version = uuid::Uuid::parse_str(version).map_err(|_| ApiError::not_found())?;

    let row: Option<(uuid::Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, storage_name, content_type FROM content_versions
         WHERE id = $1 AND item_id = $2",
    )
    .bind(version)
    .bind(item.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "version lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    match row {
        Some((id, storage_name, content_type)) => Ok(Version {
            id,
            storage_name,
            content_type,
        }),
        None => Err(ApiError::not_found()),
    }
}

/// An item the caller is allowed to see.
async fn readable(
    state: &AppState,
    user: UserId,
    item: &str,
) -> Result<homecloud_catalog::Item, ApiError> {
    let item = parse_item(item)?;

    repository::item_for_user(state.db(), user, item)
        .await
        .map_err(catalog_error)
}

/// Re-reads an item after its contents changed, so the response carries
/// the new size and modification time rather than the old ones.
async fn refresh(
    state: &AppState,
    item: &homecloud_catalog::Item,
) -> Result<homecloud_catalog::Item, ApiError> {
    crate::items::record_uploaded_file(state, item.library, &item.path).await
}

/// Records what a file used to be.
async fn record(
    state: &AppState,
    item: ItemId,
    previous: &homecloud_catalog::Item,
    storage_name: &str,
    user: UserId,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO content_versions
            (item_id, storage_name, size_bytes, content_type, content_modified_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(item.as_uuid())
    .bind(storage_name)
    .bind(previous.size_bytes)
    .bind(previous.content_type.as_deref())
    .bind(previous.modified_at)
    .bind(user.as_uuid())
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not record a version");
        ApiError::internal()
    })?;

    Ok(())
}

/// Drops the oldest versions past the limit, bytes and all.
///
/// A file someone edits every minute must not be able to fill a disk
/// with its own history.
async fn prune(state: &AppState, item: ItemId, storage: &homecloud_storage::FilesystemStorage) {
    let stale: Result<Vec<(uuid::Uuid, String)>, _> = sqlx::query_as(
        "DELETE FROM content_versions
         WHERE id IN (
             SELECT id FROM content_versions
             WHERE item_id = $1
             ORDER BY created_at DESC
             OFFSET $2
         )
         RETURNING id, storage_name",
    )
    .bind(item.as_uuid())
    .bind(MAX_VERSIONS_PER_ITEM)
    .fetch_all(state.db())
    .await;

    match stale {
        Ok(stale) => {
            for (_, name) in stale {
                storage.discard_version(&name).await;
            }
        }
        Err(error) => tracing::warn!(error = %error, "could not prune versions"),
    }
}
