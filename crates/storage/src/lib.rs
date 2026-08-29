//! Storage backends.
//!
//! Only read-only primitives exist today: containment behaviour is
//! proven before any code can mutate a user's files.

pub mod filesystem;
pub mod path;

use std::future::Future;
use std::time::SystemTime;

pub use filesystem::FilesystemStorage;
pub use path::{LibraryPath, PathError};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("the path is not valid: {0}")]
    InvalidPath(#[from] PathError),
    #[error("no such file or directory")]
    NotFound,
    #[error("the path is not a directory")]
    NotADirectory,
    #[error("the path escapes the library root")]
    OutsideRoot,
    #[error("symbolic links are not followed")]
    SymlinkNotFollowed,
    #[error("permission denied")]
    PermissionDenied,
    #[error("storage is unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
}

/// What a read-only backend can say about one entry. Deliberately small:
/// hashes, thumbnails, and media metadata belong to the catalog, not to
/// the storage boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: LibraryPath,
    pub name: String,
    pub kind: EntryKind,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

impl Entry {
    pub fn is_directory(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// The read-only half of a storage backend.
///
/// Mutation (upload, move, delete) is intentionally absent: it will be
/// added with its own tests and security review.
pub trait ReadOnlyStorage {
    /// Describes a single entry.
    fn stat(&self, path: &LibraryPath) -> impl Future<Output = Result<Entry, StorageError>> + Send;

    /// Lists the direct children of a directory. Order is unspecified;
    /// callers that display results sort them.
    fn list(
        &self,
        path: &LibraryPath,
    ) -> impl Future<Output = Result<Vec<Entry>, StorageError>> + Send;
}
