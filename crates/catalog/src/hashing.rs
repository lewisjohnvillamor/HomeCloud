//! Content hashing, for finding exact duplicates.
//!
//! A personal library accumulates the same photo several times: from the
//! camera, from a message, from a backup someone copied in "just in
//! case". Names and sizes are not enough to be sure two files are the
//! same — a hash is, and BLAKE3 is fast enough to work through a library
//! in the background without the machine noticing.
//!
//! Hashing is deliberately incremental and bounded. One pass takes a
//! limited number of files, so a library of a hundred thousand photos
//! hashes over many passes instead of pinning a disk for an hour, and a
//! file whose size or modification time has moved on since it was hashed
//! is queued again rather than trusted.

use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::CatalogError;

/// Bytes read at a time. Large enough that hashing is not syscall-bound,
/// small enough that a huge file never sits in memory.
const CHUNK_BYTES: usize = 256 * 1024;

/// What one pass did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HashSummary {
    pub hashed: u64,
    pub failed: u64,
}

/// A file waiting to be hashed.
#[derive(Debug)]
pub struct Pending {
    pub id: ItemId,
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: Option<OffsetDateTime>,
}

/// Files whose hash is missing or stale, oldest first.
pub async fn pending(
    pool: &PgPool,
    library: LibraryId,
    limit: i64,
) -> Result<Vec<Pending>, CatalogError> {
    let rows: Vec<(uuid::Uuid, String, i64, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, relative_path, size_bytes, modified_at FROM items
         WHERE library_id = $1
           AND kind = 'file'
           AND trashed_at IS NULL
           AND missing_since IS NULL
           AND (
               content_hash IS NULL
               OR hashed_size IS DISTINCT FROM size_bytes
               OR hashed_modified_at IS DISTINCT FROM modified_at
           )
         ORDER BY size_bytes
         LIMIT $2",
    )
    .bind(library.as_uuid())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, path, size_bytes, modified_at)| Pending {
            id: ItemId::from_uuid(id),
            path,
            size_bytes,
            modified_at,
        })
        .collect())
}

/// Hashes one file, streaming it rather than reading it into memory.
///
/// A 40 GB video must cost the same memory as a text file.
pub async fn hash_file(
    storage: &FilesystemStorage,
    path: &LibraryPath,
) -> Result<[u8; 32], std::io::Error> {
    use tokio::io::AsyncReadExt;

    let (mut file, _) = storage
        .open_file(path)
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; CHUNK_BYTES];

    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Records a file's hash, along with what it looked like at the time.
pub async fn record(
    pool: &PgPool,
    item: ItemId,
    hash: &[u8; 32],
    size_bytes: i64,
    modified_at: Option<OffsetDateTime>,
) -> Result<(), CatalogError> {
    sqlx::query(
        "UPDATE items
         SET content_hash = $2, hashed_size = $3, hashed_modified_at = $4, hashed_at = now()
         WHERE id = $1",
    )
    .bind(item.as_uuid())
    .bind(hash.as_slice())
    .bind(size_bytes)
    .bind(modified_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// One set of files that are byte-for-byte the same.
#[derive(Debug)]
pub struct DuplicateGroup {
    pub size_bytes: i64,
    pub items: Vec<crate::Item>,
}

/// Groups of exact duplicates in a library, biggest waste first.
///
/// Ordered by what removing the extras would actually reclaim — a
/// hundred copies of a small icon matter less than two copies of a
/// video, and the point of the list is space.
pub async fn duplicates(
    pool: &PgPool,
    library: LibraryId,
    limit: i64,
) -> Result<Vec<DuplicateGroup>, CatalogError> {
    let hashes: Vec<(Vec<u8>, i64)> = sqlx::query_as(
        "SELECT content_hash, size_bytes FROM items
         WHERE library_id = $1
           AND content_hash IS NOT NULL
           AND trashed_at IS NULL
           AND missing_since IS NULL
         GROUP BY content_hash, size_bytes
         HAVING count(*) > 1
         ORDER BY size_bytes * (count(*) - 1) DESC
         LIMIT $2",
    )
    .bind(library.as_uuid())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut groups = Vec::with_capacity(hashes.len());

    for (hash, size_bytes) in hashes {
        let items = crate::repository::items_with_hash(pool, library, &hash).await?;

        // A group of one is not a duplicate: something may have been
        // trashed between the two queries.
        if items.len() > 1 {
            groups.push(DuplicateGroup { size_bytes, items });
        }
    }

    Ok(groups)
}
