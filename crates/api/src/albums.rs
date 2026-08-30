//! Curating a library: favorites and albums.
//!
//! Neither owns any bytes. A favorite is a row pointing at an item, and
//! an album is an ordered list of them, so renaming a file, moving it,
//! or reorganising a folder leaves both intact — which is the whole
//! reason the catalog gives every item a stable id.
//!
//! The two differ in scope on purpose. A favorite is one person's
//! opinion: in a family library, what someone stars is theirs. An album
//! is something people make together, so it belongs to the library and
//! every member sees it.

use axum::extract::{Path, State};
use axum::Json;
use homecloud_catalog::repository;
use homecloud_domain::identity::{ItemId, LibraryId, UserId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{authorize, catalog_error, parse_item, parse_library};
use crate::view::{self, ItemView};

/// Longest album name accepted. Long enough for "Wales, summer 2019",
/// short enough that a name cannot become a payload.
const MAX_ALBUM_NAME: usize = 96;

/// Most items one album may hold. An album is a curated set; past this
/// it is a folder, and folders already exist.
const MAX_ALBUM_ITEMS: i64 = 5_000;

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

// --- Favorites ---

/// `PUT /api/v1/items/{item}/favorite`
///
/// Idempotent: starring something twice is the same as starring it once,
/// which is what a client that retries needs.
pub async fn add_favorite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let item = readable_item(&state, user, &item).await?;

    sqlx::query(
        "INSERT INTO item_favorites (user_id, item_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(user.as_uuid())
    .bind(item.as_uuid())
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not record a favorite");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(serde_json::json!({ "favorite": true })))
}

/// `DELETE /api/v1/items/{item}/favorite`
pub async fn remove_favorite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let item = readable_item(&state, user, &item).await?;

    sqlx::query("DELETE FROM item_favorites WHERE user_id = $1 AND item_id = $2")
        .bind(user.as_uuid())
        .bind(item.as_uuid())
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not remove a favorite");
            ApiError::dependency_unavailable("database")
        })?;

    Ok(Json(serde_json::json!({ "favorite": false })))
}

/// `GET /api/v1/libraries/{library}/favorites`
///
/// One person's own, most recently starred first. Someone else's
/// favorites in the same library are not visible here, because they are
/// not this person's opinion to read.
pub async fn list_favorites(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT f.item_id FROM item_favorites f
         JOIN items i ON i.id = f.item_id
         WHERE f.user_id = $1
           AND i.library_id = $2
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
         ORDER BY f.created_at DESC",
    )
    .bind(user.as_uuid())
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not list favorites");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(view::items(&load_items(&state, library, &ids).await?)))
}

// --- Albums ---

#[derive(Debug, Deserialize)]
pub struct AlbumRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AlbumView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub item_count: i64,
    /// The first picture in the album, for a cover tile. Absent while
    /// the album is empty.
    pub cover_item_id: Option<String>,
}

/// `POST /api/v1/libraries/{library}/albums`
pub async fn create_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Json(request): Json<AlbumRequest>,
) -> Result<Json<AlbumView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let name = album_name(&request.name)?;

    let row: (uuid::Uuid, OffsetDateTime) = sqlx::query_as(
        "INSERT INTO albums (library_id, name, created_by) VALUES ($1, $2, $3)
         RETURNING id, created_at",
    )
    .bind(library.as_uuid())
    .bind(&name)
    .bind(user.as_uuid())
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not create an album");
        ApiError::internal()
    })?;

    Ok(Json(AlbumView {
        id: row.0.to_string(),
        name,
        created_at: rfc3339(row.1),
        item_count: 0,
        cover_item_id: None,
    }))
}

/// `GET /api/v1/libraries/{library}/albums`
pub async fn list_albums(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<AlbumView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    // The count and the cover come from the same pass, so listing albums
    // is one query rather than one per album.
    let rows: Vec<(uuid::Uuid, String, OffsetDateTime, i64, Option<uuid::Uuid>)> = sqlx::query_as(
        "SELECT a.id, a.name, a.created_at,
                    count(ai.item_id) AS item_count,
                    (SELECT ai2.item_id FROM album_items ai2
                     JOIN items i2 ON i2.id = ai2.item_id
                     WHERE ai2.album_id = a.id
                       AND i2.trashed_at IS NULL
                       AND i2.missing_since IS NULL
                     ORDER BY ai2.position
                     LIMIT 1) AS cover
             FROM albums a
             LEFT JOIN album_items ai ON ai.album_id = a.id
             WHERE a.library_id = $1
             GROUP BY a.id
             ORDER BY lower(a.name)",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not list albums");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, created_at, item_count, cover)| AlbumView {
                id: id.to_string(),
                name,
                created_at: rfc3339(created_at),
                item_count,
                cover_item_id: cover.map(|id| id.to_string()),
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
pub struct AlbumContentsView {
    pub album: AlbumView,
    pub items: Vec<ItemView>,
}

/// `GET /api/v1/albums/{album}` — the album and what is in it, in order.
pub async fn read_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(album): Path<String>,
) -> Result<Json<AlbumContentsView>, ApiError> {
    let (album, library, name, created_at) = readable_album(&state, user, &album).await?;

    let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT ai.item_id FROM album_items ai
         JOIN items i ON i.id = ai.item_id
         WHERE ai.album_id = $1
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
         ORDER BY ai.position",
    )
    .bind(album)
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not read an album");
        ApiError::dependency_unavailable("database")
    })?;

    let items = load_items(&state, library, &ids).await?;

    Ok(Json(AlbumContentsView {
        album: AlbumView {
            id: album.to_string(),
            name,
            created_at: rfc3339(created_at),
            item_count: items.len() as i64,
            cover_item_id: items.first().map(|item| item.id.to_string()),
        },
        items: view::items(&items),
    }))
}

/// `PATCH /api/v1/albums/{album}` — rename.
pub async fn rename_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(album): Path<String>,
    Json(request): Json<AlbumRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (album, _, _, _) = readable_album(&state, user, &album).await?;
    let name = album_name(&request.name)?;

    sqlx::query("UPDATE albums SET name = $2, updated_at = now() WHERE id = $1")
        .bind(album)
        .bind(&name)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not rename an album");
            ApiError::internal()
        })?;

    Ok(Json(serde_json::json!({ "name": name })))
}

/// `DELETE /api/v1/albums/{album}`
///
/// Removes the arrangement, never the pictures: an album is a way of
/// looking at a library, and deleting one must not lose anything.
pub async fn delete_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(album): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (album, _, _, _) = readable_album(&state, user, &album).await?;

    sqlx::query("DELETE FROM albums WHERE id = $1")
        .bind(album)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not delete an album");
            ApiError::internal()
        })?;

    tracing::info!("an album was deleted");

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Debug, Deserialize)]
pub struct AlbumItemsRequest {
    pub items: Vec<String>,
}

/// `POST /api/v1/albums/{album}/items` — add pictures to the end.
pub async fn add_to_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(album): Path<String>,
    Json(request): Json<AlbumItemsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (album, library, _, _) = readable_album(&state, user, &album).await?;

    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM album_items WHERE album_id = $1")
        .bind(album)
        .fetch_one(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not count an album");
            ApiError::dependency_unavailable("database")
        })?;

    if existing + request.items.len() as i64 > MAX_ALBUM_ITEMS {
        return Err(ApiError::bad_request(format!(
            "An album holds at most {MAX_ALBUM_ITEMS} items."
        )));
    }

    let mut added = 0u64;
    let mut next = existing;

    for raw in &request.items {
        let item = parse_item(raw)?;

        // Every item is checked against the album's own library, so an
        // id from somewhere else cannot be smuggled into an album that a
        // link might later make public.
        repository::item_in_library(state.db(), library, item)
            .await
            .map_err(catalog_error)?;

        let inserted = sqlx::query(
            "INSERT INTO album_items (album_id, item_id, position) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(album)
        .bind(item.as_uuid())
        .bind(next)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not add to an album");
            ApiError::dependency_unavailable("database")
        })?
        .rows_affected();

        added += inserted;
        next += inserted as i64;
    }

    touch(&state, album).await;

    Ok(Json(serde_json::json!({ "added": added })))
}

/// `DELETE /api/v1/albums/{album}/items/{item}`
pub async fn remove_from_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((album, item)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (album, _, _, _) = readable_album(&state, user, &album).await?;
    let item = parse_item(&item)?;

    sqlx::query("DELETE FROM album_items WHERE album_id = $1 AND item_id = $2")
        .bind(album)
        .bind(item.as_uuid())
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not remove from an album");
            ApiError::dependency_unavailable("database")
        })?;

    touch(&state, album).await;

    Ok(Json(serde_json::json!({ "removed": true })))
}

// --- Shared checks ---

/// Validates an album name the way every other name in the product is.
fn album_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();

    if name.is_empty() {
        return Err(ApiError::bad_request("An album needs a name."));
    }
    if name.chars().count() > MAX_ALBUM_NAME {
        return Err(ApiError::bad_request(format!(
            "An album name is at most {MAX_ALBUM_NAME} characters."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "That name contains control characters.",
        ));
    }

    Ok(name.to_owned())
}

/// An item the caller may read, by id.
async fn readable_item(state: &AppState, user: UserId, item: &str) -> Result<ItemId, ApiError> {
    let item = parse_item(item)?;

    repository::item_for_user(state.db(), user, item)
        .await
        .map(|item| item.id)
        .map_err(catalog_error)
}

/// An album in a library the caller belongs to.
///
/// A caller who is not a member gets "not found" rather than
/// "forbidden": whether an album exists is itself something only the
/// library's members should learn.
async fn readable_album(
    state: &AppState,
    user: UserId,
    album: &str,
) -> Result<(uuid::Uuid, LibraryId, String, OffsetDateTime), ApiError> {
    let album = uuid::Uuid::parse_str(album).map_err(|_| ApiError::not_found())?;

    let row: Option<(uuid::Uuid, uuid::Uuid, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT a.id, a.library_id, a.name, a.created_at FROM albums a
         WHERE a.id = $1
           AND EXISTS (
               SELECT 1 FROM library_members m
               WHERE m.library_id = a.library_id AND m.user_id = $2
           )",
    )
    .bind(album)
    .bind(user.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "album lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    match row {
        Some((id, library, name, created_at)) => {
            Ok((id, LibraryId::from_uuid(library), name, created_at))
        }
        None => Err(ApiError::not_found()),
    }
}

/// Loads items by id, keeping the order they were asked for.
async fn load_items(
    state: &AppState,
    library: LibraryId,
    ids: &[uuid::Uuid],
) -> Result<Vec<homecloud_catalog::Item>, ApiError> {
    let mut items = Vec::with_capacity(ids.len());

    for id in ids {
        // An item that has gone missing between the two queries is
        // simply not shown, rather than failing the whole request.
        if let Ok(item) =
            repository::item_in_library(state.db(), library, ItemId::from_uuid(*id)).await
        {
            items.push(item);
        }
    }

    Ok(items)
}

/// Records that an album changed, for the list's ordering and for a
/// client deciding whether to re-fetch.
async fn touch(state: &AppState, album: uuid::Uuid) {
    if let Err(error) = sqlx::query("UPDATE albums SET updated_at = now() WHERE id = $1")
        .bind(album)
        .execute(state.db())
        .await
    {
        tracing::debug!(error = %error, "could not touch an album");
    }
}
