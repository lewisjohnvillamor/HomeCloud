//! Adversarial tests for the filesystem backend.
//!
//! Containment is the property that keeps a personal cloud from becoming
//! a file server for the whole host, so it is tested with the inputs an
//! attacker would actually send.

use std::fs;
use std::os::unix::fs as unix_fs;

use homecloud_storage::path::PathError;
use homecloud_storage::{
    EntryKind, FilesystemStorage, LibraryPath, MutableStorage, ReadOnlyStorage, StorageError,
};
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

// --- Mutation ---

#[tokio::test]
async fn a_folder_can_be_created_and_then_listed() {
    let (_temp, storage) = library().await;

    storage
        .create_folder(&path("documents/2026"))
        .await
        .expect("create folder");

    let entry = storage.stat(&path("documents/2026")).await.expect("stat");
    assert_eq!(entry.kind, EntryKind::Directory);
}

#[tokio::test]
async fn creating_a_folder_that_exists_is_refused() {
    let (_temp, storage) = library().await;

    let error = storage.create_folder(&path("photos")).await;

    assert!(
        matches!(error, Err(StorageError::AlreadyExists)),
        "{error:?}"
    );
}

#[tokio::test]
async fn the_library_root_itself_cannot_be_modified() {
    let (_temp, storage) = library().await;

    let error = storage.create_folder(&LibraryPath::root()).await;

    assert!(
        matches!(error, Err(StorageError::RootIsNotAnEntry)),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_upload_is_only_visible_once_it_completes() {
    let (temp, storage) = library().await;
    let destination = path("upload.txt");

    let mut staged = storage.begin_upload(1024).await.expect("begin upload");
    staged.write_chunk(b"hello ").await.expect("write");
    staged.write_chunk(b"world").await.expect("write");

    // Still nothing at the destination while the upload is in flight.
    assert!(storage.stat(&destination).await.is_err());

    storage
        .finish_upload(staged, &destination)
        .await
        .expect("finish upload");

    let entry = storage.stat(&destination).await.expect("stat");
    assert_eq!(entry.size_bytes, 11);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/upload.txt")).expect("read"),
        "hello world"
    );
}

#[tokio::test]
async fn an_upload_beyond_the_limit_is_refused_and_leaves_nothing_behind() {
    let (temp, storage) = library().await;

    let mut staged = storage.begin_upload(4).await.expect("begin upload");
    let error = staged.write_chunk(b"too many bytes").await;

    assert!(matches!(error, Err(StorageError::TooLarge)), "{error:?}");
    staged.abort().await;

    let staging = temp.path().join("library/.homecloud-incoming");
    let leftovers = std::fs::read_dir(&staging)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(
        leftovers, 0,
        "an aborted upload left a temporary file behind"
    );
}

#[tokio::test]
async fn an_upload_never_overwrites_an_existing_file() {
    let (_temp, storage) = library().await;

    let staged = storage.begin_upload(1024).await.expect("begin upload");
    let error = storage.finish_upload(staged, &path("notes.txt")).await;

    assert!(
        matches!(error, Err(StorageError::AlreadyExists)),
        "{error:?}"
    );
    // The original is untouched.
    assert_eq!(
        storage
            .stat(&path("notes.txt"))
            .await
            .expect("stat")
            .size_bytes,
        "hello".len() as u64
    );
}

#[tokio::test]
async fn a_free_name_is_derived_when_one_is_taken() {
    let (_temp, storage) = library().await;

    let free = storage
        .available_path(&path("notes.txt"))
        .await
        .expect("available path");

    assert_eq!(free.to_string(), "notes (2).txt");
}

#[tokio::test]
async fn an_unused_name_is_returned_unchanged() {
    let (_temp, storage) = library().await;

    let free = storage
        .available_path(&path("brand-new.txt"))
        .await
        .expect("available path");

    assert_eq!(free.to_string(), "brand-new.txt");
}

#[tokio::test]
async fn an_entry_can_be_renamed_and_moved() {
    let (_temp, storage) = library().await;

    storage
        .move_entry(&path("notes.txt"), &path("photos/notes.txt"))
        .await
        .expect("move");

    assert!(storage.stat(&path("notes.txt")).await.is_err());
    assert!(storage.stat(&path("photos/notes.txt")).await.is_ok());
}

#[tokio::test]
async fn a_move_never_overwrites_the_destination() {
    let (_temp, storage) = library().await;
    std::fs::write(
        storage.root().join("photos").join("notes.txt"),
        b"different",
    )
    .expect("write");

    let error = storage
        .move_entry(&path("notes.txt"), &path("photos/notes.txt"))
        .await;

    assert!(
        matches!(error, Err(StorageError::AlreadyExists)),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_folder_cannot_be_moved_inside_itself() {
    let (_temp, storage) = library().await;

    let error = storage
        .move_entry(&path("photos"), &path("photos/2024/photos"))
        .await;

    assert!(
        matches!(error, Err(StorageError::WouldMoveIntoItself)),
        "{error:?}"
    );
}

#[tokio::test]
async fn trashing_moves_the_file_rather_than_deleting_it() {
    let (temp, storage) = library().await;

    let trashed = storage
        .move_to_trash(&path("notes.txt"))
        .await
        .expect("trash");

    assert!(storage.stat(&path("notes.txt")).await.is_err());
    let contents = std::fs::read_to_string(temp.path().join("library").join(trashed.to_string()))
        .expect("the trashed file still exists");
    assert_eq!(contents, "hello");
}

#[tokio::test]
async fn a_trashed_file_can_be_moved_back() {
    let (_temp, storage) = library().await;
    let trashed = storage
        .move_to_trash(&path("notes.txt"))
        .await
        .expect("trash");

    storage
        .move_entry(&trashed, &path("notes.txt"))
        .await
        .expect("restore");

    assert!(storage.stat(&path("notes.txt")).await.is_ok());
}

#[tokio::test]
async fn writes_do_not_follow_a_symlinked_directory() {
    let (temp, storage) = library().await;
    unix_fs::symlink(
        temp.path().join("outside"),
        temp.path().join("library/elsewhere"),
    )
    .expect("create symlink");

    let error = storage.create_folder(&path("elsewhere/new")).await;

    assert!(
        matches!(error, Err(StorageError::SymlinkNotFollowed)),
        "{error:?}"
    );
}

#[tokio::test]
async fn opening_a_file_returns_its_size() {
    let (_temp, storage) = library().await;

    let (_file, size) = storage.open_file(&path("notes.txt")).await.expect("open");

    assert_eq!(size, "hello".len() as u64);
}
