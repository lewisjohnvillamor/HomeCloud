//! Local filesystem backend.
//!
//! The root is canonicalised once at construction. Every resolved path is
//! checked to be inside that root, and symbolic links are never followed,
//! so a link planted inside the library cannot be used to read the rest
//! of the host.

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::path::LibraryPath;
use crate::{
    Entry, EntryKind, MutableStorage, ReadOnlyStorage, StorageError, TRASH_DIRECTORY,
    UPLOAD_DIRECTORY,
};

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

    fn entry_for(path: LibraryPath, resolved: &Path, metadata: &std::fs::Metadata) -> Entry {
        let name = match resolved.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            // Only the root has no file name of its own.
            None => String::new(),
        };

        Entry {
            path,
            name,
            kind: if metadata.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size_bytes: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

impl ReadOnlyStorage for FilesystemStorage {
    async fn stat(&self, path: &LibraryPath) -> Result<Entry, StorageError> {
        let resolved = self.resolve(path).await?;
        let metadata = fs::symlink_metadata(&resolved)
            .await
            .map_err(map_io_error)?;

        Ok(Self::entry_for(path.clone(), &resolved, &metadata))
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
            entries.push(Self::entry_for(child_path, &child.path(), &metadata));
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

/// Upload staging.
///
/// Bytes land in a temporary file inside the root and are renamed into
/// place only once the whole upload has arrived, so an interrupted
/// transfer never leaves a half-written file where a real one belongs.
pub struct StagedUpload {
    file: fs::File,
    temporary: PathBuf,
    written: u64,
    limit: u64,
}

impl StagedUpload {
    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), StorageError> {
        self.written = self.written.saturating_add(chunk.len() as u64);
        if self.written > self.limit {
            return Err(StorageError::TooLarge);
        }

        self.file.write_all(chunk).await.map_err(map_io_error)
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    /// Abandons the upload and removes the temporary file.
    pub async fn abort(self) {
        drop(self.file);
        let _ = fs::remove_file(&self.temporary).await;
    }
}

impl FilesystemStorage {
    /// Resolves a path for writing.
    ///
    /// Unlike [`Self::resolve`], the final component may not exist yet —
    /// that is the point of a write. Every component that *does* exist is
    /// still checked for symlinks, so a planted link cannot redirect a
    /// write outside the root.
    async fn resolve_for_write(&self, path: &LibraryPath) -> Result<PathBuf, StorageError> {
        if path.is_root() {
            return Err(StorageError::RootIsNotAnEntry);
        }

        let mut resolved = self.root.clone();
        for component in path.as_path().components() {
            resolved.push(component);

            match fs::symlink_metadata(&resolved).await {
                Ok(metadata) if metadata.is_symlink() => {
                    return Err(StorageError::SymlinkNotFollowed);
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(map_io_error(error)),
            }
        }

        if !resolved.starts_with(&self.root) {
            return Err(StorageError::OutsideRoot);
        }

        Ok(resolved)
    }

    /// Begins an upload of at most `limit` bytes.
    pub async fn begin_upload(&self, limit: u64) -> Result<StagedUpload, StorageError> {
        let staging = self.root.join(UPLOAD_DIRECTORY);
        fs::create_dir_all(&staging).await.map_err(map_io_error)?;

        // A random name: two concurrent uploads of the same file must not
        // write to the same temporary path.
        let temporary = staging.join(format!("upload-{}", uuid_like()));
        let file = fs::File::create(&temporary).await.map_err(map_io_error)?;

        Ok(StagedUpload {
            file,
            temporary,
            written: 0,
            limit,
        })
    }

    /// Completes an upload by moving the staged file into place.
    ///
    /// The destination must not already exist; callers pick a free name
    /// with [`MutableStorage::available_path`] first.
    pub async fn finish_upload(
        &self,
        mut staged: StagedUpload,
        destination: &LibraryPath,
    ) -> Result<(), StorageError> {
        staged.file.flush().await.map_err(map_io_error)?;
        // Durability before visibility: the rename must not expose a file
        // whose contents are still only in the page cache.
        staged.file.sync_all().await.map_err(map_io_error)?;
        drop(staged.file);

        let target = self.resolve_for_write(destination).await?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.map_err(map_io_error)?;
        }
        if fs::symlink_metadata(&target).await.is_ok() {
            let _ = fs::remove_file(&staged.temporary).await;
            return Err(StorageError::AlreadyExists);
        }

        match fs::rename(&staged.temporary, &target).await {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&staged.temporary).await;
                Err(map_io_error(error))
            }
        }
    }

    /// Opens a file for reading, refusing to follow symlinks.
    pub async fn open_file(&self, path: &LibraryPath) -> Result<(fs::File, u64), StorageError> {
        let resolved = self.resolve(path).await?;

        let metadata = fs::symlink_metadata(&resolved)
            .await
            .map_err(map_io_error)?;
        if metadata.is_dir() {
            return Err(StorageError::NotADirectory);
        }

        let file = fs::File::open(&resolved).await.map_err(map_io_error)?;

        Ok((file, metadata.len()))
    }
}

impl MutableStorage for FilesystemStorage {
    async fn create_folder(&self, path: &LibraryPath) -> Result<(), StorageError> {
        let target = self.resolve_for_write(path).await?;

        if fs::symlink_metadata(&target).await.is_ok() {
            return Err(StorageError::AlreadyExists);
        }

        fs::create_dir_all(&target).await.map_err(map_io_error)
    }

    async fn move_entry(&self, from: &LibraryPath, to: &LibraryPath) -> Result<(), StorageError> {
        // Moving a folder into its own subtree would detach it from the
        // library; the filesystem reports this inconsistently, so it is
        // rejected here.
        if to.as_path().starts_with(from.as_path()) {
            return Err(StorageError::WouldMoveIntoItself);
        }

        let source = self.resolve(from).await?;
        let target = self.resolve_for_write(to).await?;

        if fs::symlink_metadata(&target).await.is_ok() {
            return Err(StorageError::AlreadyExists);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.map_err(map_io_error)?;
        }

        fs::rename(&source, &target).await.map_err(map_io_error)
    }

    async fn move_to_trash(&self, path: &LibraryPath) -> Result<LibraryPath, StorageError> {
        let source = self.resolve(path).await?;
        let name = path
            .as_path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(StorageError::RootIsNotAnEntry)?;

        let trash_root = self.root.join(TRASH_DIRECTORY);
        fs::create_dir_all(&trash_root)
            .await
            .map_err(map_io_error)?;

        // Prefixed with a unique token so two files with the same name,
        // trashed from different folders, cannot collide.
        let trash_path = LibraryPath::parse(&format!("{TRASH_DIRECTORY}/{}-{name}", uuid_like()))?;
        let target = self.resolve_for_write(&trash_path).await?;

        fs::rename(&source, &target).await.map_err(map_io_error)?;

        Ok(trash_path)
    }

    async fn available_path(&self, desired: &LibraryPath) -> Result<LibraryPath, StorageError> {
        let target = self.resolve_for_write(desired).await?;
        if fs::symlink_metadata(&target).await.is_err() {
            return Ok(desired.clone());
        }

        let path = desired.as_path();
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("file");
        let extension = path.extension().and_then(|value| value.to_str());
        let parent = path.parent().and_then(|value| value.to_str()).unwrap_or("");

        // Bounded: a folder with thousands of same-named files should
        // report a conflict rather than spin.
        for suffix in 2..=1000 {
            let candidate_name = match extension {
                Some(extension) => format!("{stem} ({suffix}).{extension}"),
                None => format!("{stem} ({suffix})"),
            };
            let candidate = if parent.is_empty() {
                LibraryPath::parse(&candidate_name)?
            } else {
                LibraryPath::parse(&format!("{parent}/{candidate_name}"))?
            };

            let resolved = self.resolve_for_write(&candidate).await?;
            if fs::symlink_metadata(&resolved).await.is_err() {
                return Ok(candidate);
            }
        }

        Err(StorageError::AlreadyExists)
    }
}

/// A short random token for temporary and trashed file names.
///
/// Uniqueness is what matters here, not unpredictability, so this avoids
/// pulling a UUID dependency into the storage crate.
fn uuid_like() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("{nanos:x}-{sequence:x}")
}
