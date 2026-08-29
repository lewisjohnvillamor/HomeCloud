//! Adversarial tests for the filesystem backend.
//!
//! Containment is the property that keeps a personal cloud from becoming
//! a file server for the whole host, so it is tested with the inputs an
//! attacker would actually send.

use std::fs;
use std::os::unix::fs as unix_fs;

use homecloud_storage::path::PathError;
use homecloud_storage::{EntryKind, FilesystemStorage, LibraryPath, ReadOnlyStorage, StorageError};
use tempfile::TempDir;

/// Builds a library root with a small tree, plus a sibling directory
/// outside the root that hostile paths will try to reach.
async fn library() -> (TempDir, FilesystemStorage) {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().join("library");
    let outside = temp.path().join("outside");

    fs::create_dir_all(root.join("photos/2024")).expect("create tree");
    fs::create_dir_all(&outside).expect("create outside dir");
    fs::write(root.join("photos/2024/img.jpg"), b"jpeg-bytes").expect("write file");
    fs::write(root.join("notes.txt"), b"hello").expect("write file");
    fs::write(outside.join("secret.txt"), b"private").expect("write secret");

    let storage = FilesystemStorage::open(&root)
        .await
        .expect("open library root");

    (temp, storage)
}

fn path(raw: &str) -> LibraryPath {
    LibraryPath::parse(raw).expect("valid test path")
}

#[tokio::test]
async fn stat_describes_a_file() {
    let (_temp, storage) = library().await;

    let entry = storage
        .stat(&path("photos/2024/img.jpg"))
        .await
        .expect("stat the file");

    assert_eq!(entry.kind, EntryKind::File);
    assert_eq!(entry.name, "img.jpg");
    assert_eq!(entry.size_bytes, "jpeg-bytes".len() as u64);
    assert!(entry.modified.is_some());
}

#[tokio::test]
async fn listing_the_root_returns_direct_children_only() {
    let (_temp, storage) = library().await;

    let mut names: Vec<String> = storage
        .list(&LibraryPath::root())
        .await
        .expect("list root")
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();

    assert_eq!(names, vec!["notes.txt", "photos"]);
}

#[tokio::test]
async fn listing_a_file_is_refused() {
    let (_temp, storage) = library().await;

    let error = storage
        .list(&path("notes.txt"))
        .await
        .expect_err("not a dir");

    assert!(matches!(error, StorageError::NotADirectory));
}

#[tokio::test]
async fn missing_entries_report_not_found() {
    let (_temp, storage) = library().await;

    let error = storage
        .stat(&path("photos/2023/img.jpg"))
        .await
        .expect_err("missing");

    assert!(matches!(error, StorageError::NotFound));
}

#[tokio::test]
async fn traversal_never_reaches_the_path_layer() {
    for hostile in [
        "../outside/secret.txt",
        "photos/../../outside/secret.txt",
        "..",
    ] {
        assert_eq!(LibraryPath::parse(hostile), Err(PathError::Traversal));
    }
}

#[tokio::test]
async fn absolute_paths_are_rejected() {
    assert_eq!(
        LibraryPath::parse("/etc/passwd"),
        Err(PathError::NotRelative)
    );
}

#[tokio::test]
async fn a_symlink_to_a_file_outside_the_root_is_not_followed() {
    let (temp, storage) = library().await;
    let secret = temp.path().join("outside/secret.txt");
    unix_fs::symlink(&secret, temp.path().join("library/escape.txt")).expect("create symlink");

    let error = storage.stat(&path("escape.txt")).await;

    assert!(
        matches!(error, Err(StorageError::SymlinkNotFollowed)),
        "symlink was traversed: {error:?}"
    );
}

#[tokio::test]
async fn a_symlinked_directory_cannot_be_used_as_a_path_prefix() {
    let (temp, storage) = library().await;
    unix_fs::symlink(
        temp.path().join("outside"),
        temp.path().join("library/elsewhere"),
    )
    .expect("create symlink");

    let error = storage.stat(&path("elsewhere/secret.txt")).await;

    assert!(
        matches!(error, Err(StorageError::SymlinkNotFollowed)),
        "symlinked directory was traversed: {error:?}"
    );
}

#[tokio::test]
async fn symlinks_are_omitted_from_listings() {
    let (temp, storage) = library().await;
    unix_fs::symlink(
        temp.path().join("outside/secret.txt"),
        temp.path().join("library/escape.txt"),
    )
    .expect("create symlink");

    let names: Vec<String> = storage
        .list(&LibraryPath::root())
        .await
        .expect("list root")
        .into_iter()
        .map(|entry| entry.name)
        .collect();

    assert!(!names.contains(&"escape.txt".to_owned()), "{names:?}");
}

#[tokio::test]
async fn unicode_filenames_round_trip() {
    let (temp, storage) = library().await;
    let name = "naïve 写真 🌅.jpg";
    fs::write(temp.path().join("library").join(name), b"x").expect("write unicode file");

    let entry = storage.stat(&path(name)).await.expect("stat unicode name");

    assert_eq!(entry.name, name);
    assert_eq!(entry.path.to_string(), name);
}

#[tokio::test]
async fn opening_a_missing_root_fails() {
    let temp = TempDir::new().expect("temp dir");

    let error = FilesystemStorage::open(temp.path().join("does-not-exist")).await;

    assert!(matches!(error, Err(StorageError::NotFound)), "{error:?}");
}

#[tokio::test]
async fn opening_a_file_as_a_root_fails() {
    let temp = TempDir::new().expect("temp dir");
    let file = temp.path().join("not-a-dir");
    fs::write(&file, b"x").expect("write file");

    let error = FilesystemStorage::open(&file).await;

    assert!(
        matches!(error, Err(StorageError::NotADirectory)),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_symlinked_root_is_canonicalised_once() {
    let (temp, _) = library().await;
    let link = temp.path().join("library-link");
    unix_fs::symlink(temp.path().join("library"), &link).expect("link the root");

    // The root itself may be reached through a link; what must not happen
    // is following links found *inside* the library.
    let storage = FilesystemStorage::open(&link).await.expect("open via link");

    assert_eq!(
        storage.root(),
        temp.path()
            .join("library")
            .canonicalize()
            .expect("canonical")
    );
    assert!(storage.stat(&path("notes.txt")).await.is_ok());
}
