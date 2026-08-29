//! Catalog writes.
//!
//! These functions keep the catalog in step with filesystem operations
//! that have already happened. The filesystem moves first; if a catalog
//! write then fails, the next scan reconciles the difference — the
//! reverse order would leave the catalog claiming things that are not
//! true on disk.

use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::LibraryPath;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::item::{guess_content_type, ItemKind};
use crate::repository::{self, CatalogError};

/// Records an entry that now exists on disk, returning its id.
pub async fn record_entry(
    pool: &PgPool,
    library: LibraryId,
    path: &LibraryPath,
    kind: ItemKind,
    size_bytes: i64,
    modified_at: Option<OffsetDateTime>,
) -> Result<ItemId, CatalogError> {
    let name = path
        .as_path()
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned();

    let parent = parent_id(pool, library, path).await?;
    let content_type = match kind {
        ItemKind::File => guess_content_type(&name),
        ItemKind::Folder => None,
    };

    repository::upsert(
        pool,
        library,
        parent,
        path,
        &name,
        kind,
        size_bytes,
        content_type.as_deref(),
        modified_at,
    )
    .await
}

/// Updates the catalog after an entry has been renamed or moved.
///
/// Descendants keep their ids and their position in the tree; only the
/// stored path text changes, because a path is data, not identity.
pub async fn record_move(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
    from: &LibraryPath,
    to: &LibraryPath,
) -> Result<(), CatalogError> {
    let from = from.to_string();
    let to = to.to_string();
    let name = to.rsplit('/').next().unwrap_or(&to).to_owned();

    let mut tx = pool.begin().await?;

    let parent = parent_id_in(&mut tx, library, &to).await?;

    sqlx::query(
        "UPDATE items
         SET relative_path = $3, name = $4, parent_id = $5
         WHERE library_id = $1 AND id = $2",
    )
    .bind(library.as_uuid())
    .bind(item.as_uuid())
    .bind(&to)
    .bind(&name)
    .bind(parent.map(|id| id.as_uuid()))
    .execute(&mut *tx)
    .await?;

    // Descendants: rewrite the prefix. `left(...) = prefix` rather than
    // `LIKE prefix || '%'` so a path containing `%` or `_` cannot match
    // more than it should.
    let prefix = format!("{from}/");
    sqlx::query(
        "UPDATE items
         SET relative_path = $4 || substring(relative_path from (length($3) + 1))
         WHERE library_id = $1
           AND id <> $2
           AND left(relative_path, length($3)) = $3",
    )
    .bind(library.as_uuid())
    .bind(item.as_uuid())
    .bind(&prefix)
    .bind(format!("{to}/"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Marks an item, and everything inside it, as trashed.
pub async fn record_trash(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
    original: &LibraryPath,
    trash_path: &LibraryPath,
) -> Result<(), CatalogError> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE items SET trashed_at = now(), trash_path = $3
         WHERE library_id = $1 AND id = $2",
    )
    .bind(library.as_uuid())
    .bind(item.as_uuid())
    .bind(trash_path.to_string())
    .execute(&mut *tx)
    .await?;

    let prefix = format!("{original}/");
    sqlx::query(
        "UPDATE items SET trashed_at = now()
         WHERE library_id = $1
           AND trashed_at IS NULL
           AND left(relative_path, length($2)) = $2",
    )
    .bind(library.as_uuid())
    .bind(&prefix)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Clears the trashed mark from an item and its descendants.
pub async fn record_restore(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
    original: &LibraryPath,
) -> Result<(), CatalogError> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE items SET trashed_at = NULL, trash_path = NULL
         WHERE library_id = $1 AND id = $2",
    )
    .bind(library.as_uuid())
    .bind(item.as_uuid())
    .execute(&mut *tx)
    .await?;

    let prefix = format!("{original}/");
    sqlx::query(
        "UPDATE items SET trashed_at = NULL
         WHERE library_id = $1
           AND left(relative_path, length($2)) = $2",
    )
    .bind(library.as_uuid())
    .bind(&prefix)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Where an item's bytes are while it is in the trash.
pub async fn trash_location(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
) -> Result<Option<LibraryPath>, CatalogError> {
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT trash_path FROM items WHERE library_id = $1 AND id = $2")
            .bind(library.as_uuid())
            .bind(item.as_uuid())
            .fetch_optional(pool)
            .await?;

    match stored.flatten() {
        Some(path) => Ok(Some(LibraryPath::parse(&path)?)),
        None => Ok(None),
    }
}

/// Resolves the parent item of a path, or `None` when it sits directly
/// in the library root.
async fn parent_id(
    pool: &PgPool,
    library: LibraryId,
    path: &LibraryPath,
) -> Result<Option<ItemId>, CatalogError> {
    let mut tx = pool.begin().await?;
    let parent = parent_id_in(&mut tx, library, &path.to_string()).await?;
    tx.commit().await?;

    Ok(parent)
}

async fn parent_id_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    library: LibraryId,
    path: &str,
) -> Result<Option<ItemId>, CatalogError> {
    let Some((parent_path, _)) = path.rsplit_once('/') else {
        return Ok(None);
    };

    let id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM items
         WHERE library_id = $1 AND relative_path = $2 AND trashed_at IS NULL",
    )
    .bind(library.as_uuid())
    .bind(parent_path)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(id.map(ItemId::from_uuid))
}
