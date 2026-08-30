//! Background content hashing.
//!
//! Runs after a library scan, like text extraction: reconciliation
//! decides what exists, then this works through the files whose hash is
//! missing or stale. Bounded per pass, so a library of a hundred
//! thousand photos hashes over many scans rather than pinning the disk
//! for an hour — and smallest first, so a person sees duplicates
//! appearing early instead of waiting behind one enormous video.

use homecloud_catalog::hashing::{self, HashSummary};
use homecloud_domain::identity::LibraryId;
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;

/// Files hashed in one pass.
const MAX_FILES_PER_PASS: i64 = 2_000;

/// Hashes files whose hash is missing or stale.
pub async fn hash_library(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
) -> HashSummary {
    let mut summary = HashSummary::default();

    let pending = match hashing::pending(pool, library, MAX_FILES_PER_PASS).await {
        Ok(pending) => pending,
        Err(error) => {
            tracing::warn!(error = %error, "could not list files to hash");
            return summary;
        }
    };

    for file in pending {
        let Ok(path) = LibraryPath::parse(&file.path) else {
            tracing::warn!("skipping a file whose stored path is not valid");
            summary.failed += 1;
            continue;
        };

        match hashing::hash_file(storage, &path).await {
            Ok(hash) => {
                // Recorded with the size and time it was hashed at, so a
                // file edited afterwards is queued again rather than
                // carrying a hash that no longer describes it.
                if let Err(error) =
                    hashing::record(pool, file.id, &hash, file.size_bytes, file.modified_at).await
                {
                    tracing::warn!(error = %error, "could not record a content hash");
                    summary.failed += 1;
                } else {
                    summary.hashed += 1;
                }
            }
            Err(error) => {
                // A file that cannot be read is not an error worth
                // stopping for: it may have been removed since the scan.
                tracing::debug!(error = %error, "could not hash a file");
                summary.failed += 1;
            }
        }
    }

    if summary.hashed > 0 || summary.failed > 0 {
        tracing::info!(
            hashed = summary.hashed,
            failed = summary.failed,
            "content hashed"
        );
    }

    summary
}
