//! Item routes: inspect, create folders, rename, move, trash, restore.
//!
//! Every handler resolves the item through the caller's membership, so
//! an id from another library reads as "not found" rather than as
//! "forbidden".

use axum::extract::{Path, State};
use axum::Json;
use homecloud_catalog::item::ItemKind;
use homecloud_catalog::repository::{self};
use homecloud_catalog::{mutation, Item};
use homecloud_storage::{LibraryPath, MutableStorage, ReadOnlyStorage, StorageError};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{authorize, catalog_error, parse_item, parse_library, storage_for};
use crate::view::ItemView;

/// `GET /api/v1/items/{item}`
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<ItemView>, ApiError> {
    let item = load(&state, user, &item).await?;

    Ok(Json(ItemView::from(&item)))
}

/// `GET /api/v1/items/{item}/children`
pub async fn children(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let item = load(&state, user, &item).await?;
    if !item.is_folder() {
        return Err(ApiError::bad_request("That item is not a folder."));
    }

    let children = repository::children(state.db(), item.library, Some(item.id))
        .await
        .map_err(catalog_error)?;

    Ok(Json(crate::view::items(&children)))
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    /// Library-relative path of the new folder.
    pub path: String,
}

/// `POST /api/v1/libraries/{library}/folders`
pub async fn create_folder(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<Json<ItemView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let path = parse_path(&request.path)?;
    let storage = storage_for(&state, library).await?;

    storage.create_folder(&path).await.map_err(storage_error)?;

    // The filesystem changed first; the catalog now records what is
    // already true on disk.
    mutation::record_entry(state.db(), library, &path, ItemKind::Folder, 0, None)
        .await
        .map_err(catalog_error)?;

    let item = repository::item_at_path(state.db(), library, &path)
        .await
        .map_err(catalog_error)?;

    Ok(Json(ItemView::from(&item)))
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    /// New library-relative path, including the (possibly new) name.
    pub path: String,
}

/// `POST /api/v1/items/{item}/move` — also does renames, which are the
/// same operation with the same parent.
pub async fn move_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    Json(request): Json<MoveRequest>,
) -> Result<Json<ItemView>, ApiError> {
    let item = load(&state, user, &item).await?;
    if item.trashed_at.is_some() {
        return Err(ApiError::conflict("Restore the item before moving it."));
    }

    let destination = parse_path(&request.path)?;
    let storage = storage_for(&state, item.library).await?;

    storage
        .move_entry(&item.path, &destination)
        .await
        .map_err(storage_error)?;

    mutation::record_move(state.db(), item.library, item.id, &item.path, &destination)
        .await
        .map_err(catalog_error)?;

    let moved = repository::item_at_path(state.db(), item.library, &destination)
        .await
        .map_err(catalog_error)?;

    Ok(Json(ItemView::from(&moved)))
}

/// `DELETE /api/v1/items/{item}` — moves the item to the trash.
///
/// Nothing here unlinks user data: the file is moved into an
/// application-managed folder inside the library root and can be
/// restored or removed by hand.
pub async fn trash_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<ItemView>, ApiError> {
    let item = load(&state, user, &item).await?;
    if item.trashed_at.is_some() {
        return Ok(Json(ItemView::from(&item)));
    }

    let storage = storage_for(&state, item.library).await?;
    let trash_path = storage
        .move_to_trash(&item.path)
        .await
        .map_err(storage_error)?;

    mutation::record_trash(state.db(), item.library, item.id, &item.path, &trash_path)
        .await
        .map_err(catalog_error)?;

    let refreshed = repository::item_for_user(state.db(), user, item.id)
        .await
        .map_err(catalog_error)?;

    Ok(Json(ItemView::from(&refreshed)))
}

/// `POST /api/v1/items/{item}/restore`
pub async fn restore_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<ItemView>, ApiError> {
    let item = load(&state, user, &item).await?;
    if item.trashed_at.is_none() {
        return Ok(Json(ItemView::from(&item)));
    }

    let Some(trash_path) = mutation::trash_location(state.db(), item.library, item.id)
        .await
        .map_err(catalog_error)?
    else {
        return Err(ApiError::conflict(
            "This item was inside a trashed folder. Restore that folder instead.",
        ));
    };

    let storage = storage_for(&state, item.library).await?;

    // The original path may have been taken by something else in the
    // meantime; restoring must not overwrite whatever is there now.
    let destination = storage
        .available_path(&item.path)
        .await
        .map_err(storage_error)?;

    storage
        .move_entry(&trash_path, &destination)
        .await
        .map_err(storage_error)?;

    mutation::record_restore(state.db(), item.library, item.id, &item.path)
        .await
        .map_err(catalog_error)?;
    if destination != item.path {
        mutation::record_move(state.db(), item.library, item.id, &item.path, &destination)
            .await
            .map_err(catalog_error)?;
    }

    let restored = repository::item_for_user(state.db(), user, item.id)
        .await
        .map_err(catalog_error)?;

    Ok(Json(ItemView::from(&restored)))
}

/// Records a file that has just been written to disk.
pub async fn record_uploaded_file(
    state: &AppState,
    library: homecloud_domain::identity::LibraryId,
    path: &LibraryPath,
) -> Result<Item, ApiError> {
    let storage = storage_for(state, library).await?;
    let entry = storage.stat(path).await.map_err(storage_error)?;

    mutation::record_entry(
        state.db(),
        library,
        path,
        ItemKind::File,
        entry.size_bytes as i64,
        entry.modified.map(OffsetDateTime::from),
    )
    .await
    .map_err(catalog_error)?;

    let item = repository::item_at_path(state.db(), library, path)
        .await
        .map_err(catalog_error)?;

    // A photo uploaded from a phone should land in the timeline under
    // the day it was taken, not today. Waiting for the next scan would
    // put it in the wrong month until then.
    let item = crate::photometa::describe_one(state.db(), &storage, item).await;

    Ok(item)
}

/// Loads an item the caller is allowed to see.
async fn load(
    state: &AppState,
    user: homecloud_domain::identity::UserId,
    item: &str,
) -> Result<Item, ApiError> {
    let item = parse_item(item)?;

    repository::item_for_user(state.db(), user, item)
        .await
        .map_err(catalog_error)
}

/// Validates a client-supplied path and refuses HomeCloud's own
/// directories, which are not library content.
pub fn parse_path(raw: &str) -> Result<LibraryPath, ApiError> {
    let path = LibraryPath::parse(raw).map_err(|error| ApiError::bad_request(error.to_string()))?;

    if path.is_root() {
        return Err(ApiError::bad_request("A path is required."));
    }

    let first = path
        .as_path()
        .iter()
        .next()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if crate::library::RESERVED_DIRECTORIES.contains(&first) {
        return Err(ApiError::bad_request(
            "That location is reserved by HomeCloud.",
        ));
    }

    Ok(path)
}

/// Maps storage failures to the API's vocabulary. Filesystem detail —
/// absolute paths, errno text — never reaches the client.
pub fn storage_error(error: StorageError) -> ApiError {
    match error {
        StorageError::NotFound => ApiError::not_found(),
        StorageError::AlreadyExists => {
            ApiError::conflict("Something with that name already exists here.")
        }
        StorageError::WouldMoveIntoItself => {
            ApiError::bad_request("A folder cannot be moved inside itself.")
        }
        StorageError::NotADirectory => ApiError::bad_request("That item is not a folder."),
        StorageError::RootIsNotAnEntry => {
            ApiError::bad_request("The library root cannot be changed.")
        }
        StorageError::InvalidPath(error) => ApiError::bad_request(error.to_string()),
        StorageError::TooLarge => ApiError::new(
            crate::error::ErrorCode::PayloadTooLarge,
            "That file is larger than this server accepts.",
        ),
        StorageError::SymlinkNotFollowed => {
            ApiError::bad_request("Symbolic links are not followed.")
        }
        StorageError::OutsideRoot | StorageError::PermissionDenied => {
            tracing::warn!("a storage operation was refused");
            ApiError::forbidden("That location is not accessible.")
        }
        StorageError::Unavailable => {
            tracing::error!("storage is unavailable");
            ApiError::dependency_unavailable("library storage")
        }
    }
}
