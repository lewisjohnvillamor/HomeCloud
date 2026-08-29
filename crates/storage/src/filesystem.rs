//! Local filesystem backend.
//!
//! The root is canonicalised once at construction. Every resolved path is
//! checked to be inside that root, and symbolic links are never followed,
//! so a link planted inside the library cannot be used to read the rest
//! of the host.

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs;

use crate::path::LibraryPath;
use crate::{Entry, EntryKind, ReadOnlyStorage, StorageError};

#[derive(Debug, Clone)]
pub struct FilesystemStorage {
    root: PathBuf,
}

impl FilesystemStorage {
    /// Opens a library root. The directory must already exist: creating
    /// it implicitly would turn a typo in configuration into a silently
    /// empty library.
    pub async fn open(root: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root = fs::canonicalize(root.as_ref())
            .await
            .map_err(map_io_error)?;

        let metadata = fs::metadata(&root).await.map_err(map_io_error)?;
        if !metadata.is_dir() {
            return Err(StorageError::NotADirectory);
        }

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a library path to a filesystem path.
    ///
    /// Each component is inspected with `symlink_metadata`, which does
    /// not follow links, so a symlink anywhere along the path is refused
    /// rather than traversed. The result is then compared against the
    /// canonical root as a second, independent check.
    async fn resolve(&self, path: &LibraryPath) -> Result<PathBuf, StorageError> {
        let mut resolved = self.root.clone();

        for component in path.as_path().components() {
            resolved.push(component);

            let metadata = fs::symlink_metadata(&resolved)
                .await
                .map_err(map_io_error)?;
            if metadata.is_symlink() {
                tracing::debug!("refused to follow a symbolic link inside the library root");
                return Err(StorageError::SymlinkNotFollowed);
            }
        }

        if !resolved.starts_with(&self.root) {
            return Err(StorageError::OutsideRoot);
        }

        Ok(resolved)
    }

    async fn entry_for(
        &self,
        path: LibraryPath,
        resolved: &Path,
        metadata: &std::fs::Metadata,
    ) -> Result<Entry, StorageError> {
        let name = match resolved.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            // Only the root has no file name of its own.
            None => String::new(),
        };

        Ok(Entry {
            path,
            name,
            kind: if metadata.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size_bytes: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

impl ReadOnlyStorage for FilesystemStorage {
    async fn stat(&self, path: &LibraryPath) -> Result<Entry, StorageError> {
        let resolved = self.resolve(path).await?;
        let metadata = fs::symlink_metadata(&resolved)
            .await
            .map_err(map_io_error)?;

        self.entry_for(path.clone(), &resolved, &metadata).await
    }

    async fn list(&self, path: &LibraryPath) -> Result<Vec<Entry>, StorageError> {
        let resolved = self.resolve(path).await?;

        let metadata = fs::symlink_metadata(&resolved)
            .await
            .map_err(map_io_error)?;
        if !metadata.is_dir() {
            return Err(StorageError::NotADirectory);
        }

        let mut reader = fs::read_dir(&resolved).await.map_err(map_io_error)?;
        let mut entries = Vec::new();

        while let Some(child) = reader.next_entry().await.map_err(map_io_error)? {
            let file_name = child.file_name();
            let Some(name) = file_name.to_str() else {
                // A name the platform cannot represent as UTF-8 cannot be
                // addressed by the API; skipping it is safer than
                // inventing a lossy name that resolves elsewhere.
                tracing::warn!("skipping directory entry with a non-UTF-8 name");
                continue;
            };

            // Symlinks are omitted rather than described: reporting one
            // would invite a caller to follow it, and its target may sit
            // outside the library root.
            let metadata = child.metadata().await.map_err(map_io_error)?;
            if metadata.is_symlink() {
                continue;
            }

            let child_path = path.join_segment(name)?;
            entries.push(self.entry_for(child_path, &child.path(), &metadata).await?);
        }

        Ok(entries)
    }
}

/// Maps filesystem errors to the storage vocabulary. Raw `io::Error`
/// messages can contain absolute host paths, so they are not propagated.
fn map_io_error(error: io::Error) -> StorageError {
    match error.kind() {
        io::ErrorKind::NotFound => StorageError::NotFound,
        io::ErrorKind::PermissionDenied => StorageError::PermissionDenied,
        io::ErrorKind::NotADirectory => StorageError::NotADirectory,
        _ => {
            tracing::warn!(kind = ?error.kind(), "unexpected storage error");
            StorageError::Unavailable
        }
    }
}
