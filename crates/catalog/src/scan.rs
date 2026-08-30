//! Reconciliation.
//!
//! The filesystem is the source of truth. A scan walks the library root
//! and makes the catalog agree with what is actually on disk. It never
//! creates, moves, or deletes a file, and an entry that has vanished is
//! marked missing rather than removed: a disconnected drive must not
//! destroy metadata.

use std::time::Instant;

use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::{Entry, EntryKind, FilesystemStorage, LibraryPath, ReadOnlyStorage};
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::item::{guess_content_type, ItemKind};
use crate::repository::{self, CatalogError};

/// Deepest directory nesting followed. Bounds the work a pathological
/// tree (or a directory loop that survives the symlink policy) can cause.
const MAX_DEPTH: usize = 32;

/// Most entries recorded in one scan. A cap makes a runaway tree a
/// reported limit rather than an unbounded transaction backlog.
const MAX_ENTRIES: usize = 500_000;

/// Directory holding trashed files. Skipped while scanning so trashed
/// items are not re-indexed as live ones.
pub const TRASH_DIRECTORY: &str = ".homecloud-trash";

/// Temporary directory for in-flight uploads. Skipped for the same
/// reason: a half-written file is not library content.
pub const UPLOAD_DIRECTORY: &str = ".homecloud-incoming";

/// Generated derivatives. Skipped because they are rebuildable output,
/// not something a person put in their library.
pub const DERIVATIVES_DIRECTORY: &str = ".homecloud-derivatives";

/// Previous contents of replaced files. Managed by the app, never
/// library content, so a scan walks straight past it.
pub const VERSIONS_DIRECTORY: &str = ".homecloud-versions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ScanSummary {
    /// Entries seen on disk.
    pub scanned: u64,
    /// Entries that are now recorded as missing.
    pub missing: u64,
    /// Folders whose listing could not be read; the scan continues.
    pub unreadable: u64,
    pub duration_ms: u64,
    /// True when the entry cap stopped the walk early.
    pub truncated: bool,
}

/// Walks the library root and reconciles the catalog with it.
pub async fn reconcile(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
) -> Result<ScanSummary, CatalogError> {
    let started = Instant::now();
    // Recorded before the walk begins: anything indexed after this point
    // was created by someone else while the scan ran.
    let started_at = OffsetDateTime::now_utc();

    let mut queue: Vec<(LibraryPath, Option<ItemId>, usize)> = vec![(LibraryPath::root(), None, 0)];
    let mut seen: Vec<String> = Vec::new();
    let mut unreadable = 0u64;
    let mut truncated = false;

    while let Some((folder, parent, depth)) = queue.pop() {
        // Reading the directory happens outside any transaction: a
        // database transaction must never wait on filesystem I/O.
        let entries = match storage.list(&folder).await {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(error = %error, "skipping a folder that could not be listed");
                unreadable += 1;
                continue;
            }
        };

        let mut recorded = Vec::with_capacity(entries.len());

        // One short transaction per folder: small enough to stay out of
        // the way, large enough that a folder is recorded atomically.
        let mut tx = pool.begin().await?;
        for entry in &entries {
            if is_reserved(entry, depth) {
                continue;
            }
            if seen.len() + recorded.len() >= MAX_ENTRIES {
                truncated = true;
                break;
            }

            let kind = match entry.kind {
                EntryKind::Directory => ItemKind::Folder,
                EntryKind::File => ItemKind::File,
            };
            let content_type = match kind {
                ItemKind::File => guess_content_type(&entry.name),
                ItemKind::Folder => None,
            };

            let id = repository::upsert(
                &mut *tx,
                library,
                parent,
                &entry.path,
                &entry.name,
                kind,
                entry.size_bytes as i64,
                content_type.as_deref(),
                entry.modified.map(OffsetDateTime::from),
            )
            .await?;

            recorded.push((entry.path.clone(), id, kind));
        }
        tx.commit().await?;

        for (path, id, kind) in recorded {
            seen.push(path.to_string());

            if kind == ItemKind::Folder && depth + 1 < MAX_DEPTH {
                queue.push((path, Some(id), depth + 1));
            }
        }

        if truncated {
            break;
        }
    }

    let missing = repository::mark_missing_except(pool, library, &seen, started_at).await?;

    let summary = ScanSummary {
        scanned: seen.len() as u64,
        missing,
        unreadable,
        duration_ms: started.elapsed().as_millis() as u64,
        truncated,
    };

    tracing::info!(
        scanned = summary.scanned,
        missing = summary.missing,
        unreadable = summary.unreadable,
        duration_ms = summary.duration_ms,
        truncated = summary.truncated,
        "library scan finished"
    );

    Ok(summary)
}

/// HomeCloud's own directories live inside the root so everything a user
/// owns stays in one place, but they are not library content.
fn is_reserved(entry: &Entry, depth: usize) -> bool {
    depth == 0
        && matches!(
            entry.name.as_str(),
            TRASH_DIRECTORY | UPLOAD_DIRECTORY | DERIVATIVES_DIRECTORY | VERSIONS_DIRECTORY
        )
}
