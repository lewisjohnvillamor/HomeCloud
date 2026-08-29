//! Storage backends.
//!
//! Only read-only primitives exist today: containment behaviour is
//! proven before any code can mutate a user's files.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod filesystem;
pub mod path;

use std::future::Future;
use std::time::SystemTime;

pub use filesystem::FilesystemStorage;
pub use path::{LibraryPath, PathError};

/// Directory holding files removed through the app. Trashing moves a
/// file here; nothing unlinks a user's data implicitly.
pub const TRASH_DIRECTORY: &str = ".homecloud-trash";

/// Directory holding partially written uploads. Inside the root so the
/// final rename is atomic, and skipped by scans.
pub const UPLOAD_DIRECTORY: &str = ".homecloud-incoming";

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
    #[error("something already exists at that path")]
    AlreadyExists,
    #[error("a folder cannot be moved inside itself")]
    WouldMoveIntoItself,
    #[error("the library root cannot be modified")]
    RootIsNotAnEntry,
    #[error("the entry is larger than the configured limit")]
    TooLarge,
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

/// The mutating half of a storage backend.
///
/// Every operation is expressed in terms of [`LibraryPath`], so nothing
/// here can be pointed outside the library root, and no operation
/// unlinks user data: removal means moving into the trash directory.
pub trait MutableStorage {
    /// Creates a folder, including any missing parents.
    fn create_folder(
        &self,
        path: &LibraryPath,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Renames or moves an entry within the library.
    fn move_entry(
        &self,
        from: &LibraryPath,
        to: &LibraryPath,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Moves an entry into the trash directory, returning where it went
    /// so a restore can find it.
    fn move_to_trash(
        &self,
        path: &LibraryPath,
    ) -> impl Future<Output = Result<LibraryPath, StorageError>> + Send;

    /// Returns a path in the same folder that nothing occupies yet,
    /// derived from `desired` — `report.pdf` becomes `report (2).pdf`.
    /// Uploading must never silently overwrite an existing file.
    fn available_path(
        &self,
        desired: &LibraryPath,
    ) -> impl Future<Output = Result<LibraryPath, StorageError>> + Send;
}
