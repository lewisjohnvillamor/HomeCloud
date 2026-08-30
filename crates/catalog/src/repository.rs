//! Catalog queries.
//!
//! Every read is scoped by library id, and callers must have already
//! established that the user is a member of that library: the catalog
//! never widens access on its own.

use homecloud_domain::identity::{ItemId, LibraryId, UserId};
use homecloud_storage::LibraryPath;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::item::{Item, ItemKind};

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("the item does not exist")]
    NotFound,
    #[error("the path is not valid: {0}")]
    InvalidPath(#[from] homecloud_storage::PathError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

const ITEM_COLUMNS: &str = "id, library_id, parent_id, relative_path, name, kind, size_bytes, \
                            content_type, modified_at, trashed_at, missing_since";

fn item_from_row(row: &PgRow) -> Result<Item, CatalogError> {
    let path: String = row.try_get("relative_path")?;
    let kind: String = row.try_get("kind")?;

    Ok(Item {
        id: ItemId::from_uuid(row.try_get("id")?),
        library: LibraryId::from_uuid(row.try_get("library_id")?),
        parent: row
            .try_get::<Option<uuid::Uuid>, _>("parent_id")?
            .map(ItemId::from_uuid),
        path: LibraryPath::parse(&path)?,
        name: row.try_get("name")?,
        kind: ItemKind::parse(&kind).unwrap_or(ItemKind::File),
        size_bytes: row.try_get("size_bytes")?,
        content_type: row.try_get("content_type")?,
        modified_at: row.try_get("modified_at")?,
        trashed_at: row.try_get("trashed_at")?,
        missing_since: row.try_get("missing_since")?,
    })
}

fn items_from_rows(rows: Vec<PgRow>) -> Result<Vec<Item>, CatalogError> {
    rows.iter().map(item_from_row).collect()
}

/// A library the user can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySummary {
    pub id: LibraryId,
    pub name: String,
    pub root_path: Option<String>,
    pub role: String,
}

/// Libraries the user is a member of. A user who is a member of nothing
/// sees nothing; there is no implicit access to any library.
pub async fn libraries_for_user(
    pool: &PgPool,
    user: UserId,
) -> Result<Vec<LibrarySummary>, CatalogError> {
    let rows = sqlx::query(
        "SELECT l.id, l.name, l.root_path, m.role
         FROM libraries l
         JOIN library_members m ON m.library_id = l.id
         WHERE m.user_id = $1
         ORDER BY l.name",
    )
    .bind(user.as_uuid())
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            Ok(LibrarySummary {
                id: LibraryId::from_uuid(row.try_get("id")?),
                name: row.try_get("name")?,
                root_path: row.try_get("root_path")?,
                role: row.try_get("role")?,
            })
        })
        .collect()
}

/// Whether the user may act inside this library. The single place that
/// answers that question for catalog and transfer routes alike.
pub async fn is_member(
    pool: &PgPool,
    user: UserId,
    library: LibraryId,
) -> Result<bool, CatalogError> {
    let member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM library_members WHERE user_id = $1 AND library_id = $2)",
    )
    .bind(user.as_uuid())
    .bind(library.as_uuid())
    .fetch_one(pool)
    .await?;

    Ok(member)
}

/// Loads an item the user is allowed to see. Membership is part of the
/// query rather than a separate check, so a missing item and an item in
/// someone else's library are indistinguishable to the caller.
pub async fn item_for_user(
    pool: &PgPool,
    user: UserId,
    item: ItemId,
) -> Result<Item, CatalogError> {
    let row = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS}
         FROM items i
         WHERE i.id = $1
           AND EXISTS (
               SELECT 1 FROM library_members m
               WHERE m.library_id = i.library_id AND m.user_id = $2
           )"
    ))
    .bind(item.as_uuid())
    .bind(user.as_uuid())
    .fetch_optional(pool)
    .await?
    .ok_or(CatalogError::NotFound)?;

    item_from_row(&row)
}

/// Loads an item by id within a library, without a membership check.
///
/// For callers that have already established access another way — a
/// share capability, for instance — and must not widen it to the whole
/// library.
pub async fn item_in_library(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
) -> Result<Item, CatalogError> {
    let row = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items WHERE id = $1 AND library_id = $2"
    ))
    .bind(item.as_uuid())
    .bind(library.as_uuid())
    .fetch_optional(pool)
    .await?
    .ok_or(CatalogError::NotFound)?;

    item_from_row(&row)
}

/// Looks an item up by its current path.
pub async fn item_at_path(
    pool: &PgPool,
    library: LibraryId,
    path: &LibraryPath,
) -> Result<Item, CatalogError> {
    let row = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items
         WHERE library_id = $1 AND relative_path = $2 AND trashed_at IS NULL"
    ))
    .bind(library.as_uuid())
    .bind(path.to_string())
    .fetch_optional(pool)
    .await?
    .ok_or(CatalogError::NotFound)?;

    item_from_row(&row)
}

/// Direct children of a folder, or of the library root when `parent` is
/// `None`. Folders first, then by name, which is the order the file list
/// renders in; sorting in SQL keeps it stable across pages.
pub async fn children(
    pool: &PgPool,
    library: LibraryId,
    parent: Option<ItemId>,
) -> Result<Vec<Item>, CatalogError> {
    let rows = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items
         WHERE library_id = $1
           AND parent_id IS NOT DISTINCT FROM $2
           AND trashed_at IS NULL
           AND missing_since IS NULL
         ORDER BY (kind = 'folder') DESC, lower(name)"
    ))
    .bind(library.as_uuid())
    .bind(parent.map(|id| id.as_uuid()))
    .fetch_all(pool)
    .await?;

    items_from_rows(rows)
}

/// Images, newest first, for the Photos view.
pub async fn images(
    pool: &PgPool,
    library: LibraryId,
    limit: i64,
    offset: i64,
) -> Result<Vec<Item>, CatalogError> {
    let rows = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items
         WHERE library_id = $1
           AND content_type LIKE 'image/%'
           AND trashed_at IS NULL
           AND missing_since IS NULL
         ORDER BY modified_at DESC NULLS LAST, lower(name)
         LIMIT $2 OFFSET $3"
    ))
    .bind(library.as_uuid())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    items_from_rows(rows)
}

/// Name search.
///
/// The query is a user-supplied string: it is passed as a bind parameter
/// and matched with `websearch_to_tsquery`, which never fails on weird
/// input, rather than being interpolated into SQL.
pub async fn search(
    pool: &PgPool,
    library: LibraryId,
    query: &str,
    limit: i64,
) -> Result<Vec<Item>, CatalogError> {
    let rows = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items
         WHERE library_id = $1
           AND trashed_at IS NULL
           AND missing_since IS NULL
           AND (
             to_tsvector('simple', replace(name, '.', ' ')) @@ websearch_to_tsquery('simple', $2)
             OR name ILIKE '%' || $2 || '%'
           )
         ORDER BY (kind = 'folder') DESC, lower(name)
         LIMIT $3"
    ))
    .bind(library.as_uuid())
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    items_from_rows(rows)
}

/// Items currently in the trash, most recently trashed first.
pub async fn trashed(pool: &PgPool, library: LibraryId) -> Result<Vec<Item>, CatalogError> {
    let rows = sqlx::query(&format!(
        "SELECT {ITEM_COLUMNS} FROM items
         WHERE library_id = $1 AND trashed_at IS NOT NULL
         ORDER BY trashed_at DESC"
    ))
    .bind(library.as_uuid())
    .fetch_all(pool)
    .await?;

    items_from_rows(rows)
}

/// Records a file or folder found on disk, keyed by its path.
///
/// Re-running a scan updates size and timestamps in place and clears any
/// previous "missing" mark, so an item that comes back keeps its id and
/// everything attached to it.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    executor: impl sqlx::PgExecutor<'_>,
    library: LibraryId,
    parent: Option<ItemId>,
    path: &LibraryPath,
    name: &str,
    kind: ItemKind,
    size_bytes: i64,
    content_type: Option<&str>,
    modified_at: Option<OffsetDateTime>,
) -> Result<ItemId, CatalogError> {
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO items
             (library_id, parent_id, relative_path, name, kind, size_bytes, content_type, modified_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (library_id, relative_path) WHERE trashed_at IS NULL
         DO UPDATE SET
             parent_id = EXCLUDED.parent_id,
             name = EXCLUDED.name,
             kind = EXCLUDED.kind,
             size_bytes = EXCLUDED.size_bytes,
             content_type = EXCLUDED.content_type,
             modified_at = EXCLUDED.modified_at,
             indexed_at = now(),
             missing_since = NULL
         RETURNING id",
    )
    .bind(library.as_uuid())
    .bind(parent.map(|id| id.as_uuid()))
    .bind(path.to_string())
    .bind(name)
    .bind(kind.as_str())
    .bind(size_bytes)
    .bind(content_type)
    .bind(modified_at)
    .fetch_one(executor)
    .await?;

    Ok(ItemId::from_uuid(id))
}

/// Marks everything a scan did not see as missing.
///
/// Missing is not deleted: the row, its id, and anything attached to it
/// survive, because a disconnected drive must not destroy metadata.
///
/// `scan_started_at` excludes items recorded *during* the scan — an
/// upload or a new folder created while the walk was in progress was
/// never going to be seen by it, and must not be marked missing.
pub async fn mark_missing_except(
    pool: &PgPool,
    library: LibraryId,
    seen: &[String],
    scan_started_at: OffsetDateTime,
) -> Result<u64, CatalogError> {
    let result = sqlx::query(
        "UPDATE items
         SET missing_since = now()
         WHERE library_id = $1
           AND trashed_at IS NULL
           AND missing_since IS NULL
           AND indexed_at < $3
           AND NOT (relative_path = ANY($2))",
    )
    .bind(library.as_uuid())
    .bind(seen)
    .bind(scan_started_at)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Total live items in a library, for the scan summary.
pub async fn count_live(pool: &PgPool, library: LibraryId) -> Result<i64, CatalogError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM items
         WHERE library_id = $1 AND trashed_at IS NULL AND missing_since IS NULL",
    )
    .bind(library.as_uuid())
    .fetch_one(pool)
    .await?;

    Ok(count)
}
