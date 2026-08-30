//! Keeping extracted text in step with the library.

use homecloud_domain::identity::{ItemId, LibraryId};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::extract::{Extraction, Status};

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A file the index does not yet have current text for.
#[derive(Debug, Clone)]
pub struct Pending {
    pub item: ItemId,
    pub path: String,
    pub name: String,
    pub content_type: Option<String>,
    pub size_bytes: i64,
    pub modified_at: Option<OffsetDateTime>,
}

/// One row of the pending query, named so the type stays readable.
type PendingRow = (
    uuid::Uuid,
    String,
    String,
    Option<String>,
    i64,
    Option<OffsetDateTime>,
);

/// Finds files whose text is missing or stale.
///
/// Stale means the size or timestamp changed since extraction — the same
/// cheap comparison the scan uses, so re-indexing a library costs a query
/// rather than a re-read of every document.
pub async fn pending(
    pool: &PgPool,
    library: LibraryId,
    limit: i64,
) -> Result<Vec<Pending>, IndexError> {
    let rows: Vec<PendingRow> = sqlx::query_as(
        "SELECT i.id, i.relative_path, i.name, i.content_type, i.size_bytes, i.modified_at
             FROM items i
             LEFT JOIN item_text t ON t.item_id = i.id
             WHERE i.library_id = $1
               AND i.kind = 'file'
               AND i.trashed_at IS NULL
               AND i.missing_since IS NULL
               AND (
                 t.item_id IS NULL
                 OR t.source_size <> i.size_bytes
                 OR t.source_modified_at IS DISTINCT FROM i.modified_at
               )
             ORDER BY i.indexed_at DESC
             LIMIT $2",
    )
    .bind(library.as_uuid())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, path, name, content_type, size_bytes, modified_at)| Pending {
                item: ItemId::from_uuid(id),
                path,
                name,
                content_type,
                size_bytes,
                modified_at,
            },
        )
        .collect())
}

/// Records what extraction produced, including the failures.
pub async fn record(
    pool: &PgPool,
    library: LibraryId,
    item: ItemId,
    extraction: &Extraction,
    source_size: i64,
    source_modified_at: Option<OffsetDateTime>,
) -> Result<(), IndexError> {
    sqlx::query(
        "INSERT INTO item_text
             (item_id, library_id, content, status, source_size, source_modified_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (item_id) DO UPDATE SET
             library_id = EXCLUDED.library_id,
             content = EXCLUDED.content,
             status = EXCLUDED.status,
             source_size = EXCLUDED.source_size,
             source_modified_at = EXCLUDED.source_modified_at,
             extracted_at = now()",
    )
    .bind(item.as_uuid())
    .bind(library.as_uuid())
    .bind(&extraction.text)
    .bind(extraction.status.as_str())
    .bind(source_size)
    .bind(source_modified_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// How much of a library has readable text, for the scan summary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct IndexSummary {
    /// Documents whose text was extracted in this pass.
    pub indexed: u64,
    /// Files this server cannot read text from.
    pub skipped: u64,
    /// Files that should have been readable and were not.
    pub failed: u64,
}

impl IndexSummary {
    pub fn record(&mut self, status: Status) {
        match status {
            Status::Indexed => self.indexed += 1,
            Status::Unsupported | Status::TooLarge => self.skipped += 1,
            Status::Failed => self.failed += 1,
        }
    }
}
