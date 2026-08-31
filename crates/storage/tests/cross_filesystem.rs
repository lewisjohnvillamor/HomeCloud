//! A library root is one path, but it need not be one disk.
//!
//! Mounting an external drive inside the library — `library/photos` on
//! the big spinning disk, everything else on the system SSD — is an
//! ordinary way to run this, and it is what home servers with a small
//! boot drive actually do. It also breaks the two syscalls the storage
//! layer leans on: `rename` and `hard_link` both refuse to cross a
//! filesystem boundary.
//!
//! These tests mount a real second filesystem rather than simulating
//! one, because the failure being guarded against is the kernel's answer
//! and not a condition this code can invent. Mounting needs privileges
//! the test process may not have, so they announce themselves and skip
//! when it is unavailable — the same choice the media tests make about
//! FFmpeg.

use std::path::{Path, PathBuf};
use std::process::Command;

use homecloud_storage::{
    FilesystemStorage, LibraryPath, MutableStorage, ReadOnlyStorage, StorageError,
};
use tempfile::TempDir;

/// A tmpfs that unmounts itself, so a failing assertion cannot leave a
/// mount behind on the machine that ran it.
struct Mounted(PathBuf);

impl Drop for Mounted {
    fn drop(&mut self) {
        let _ = Command::new("umount").arg(&self.0).status();
    }
}

/// Mounts a small tmpfs at `at`, or reports that this machine will not
/// let us. Returning `None` rather than failing keeps the suite honest
/// on an unprivileged runner: the test says it was skipped instead of
/// quietly passing.
fn mount_tmpfs(at: &Path) -> Option<Mounted> {
    let mounted = Command::new("mount")
        .args(["-t", "tmpfs", "-o", "size=16m", "tmpfs"])
        .arg(at)
        .status();

    match mounted {
        Ok(status) if status.success() => Some(Mounted(at.to_path_buf())),
        _ => None,
    }
}

/// Two devices, one library: the root on the temp directory's
/// filesystem, and `elsewhere/` on a filesystem of its own.
///
/// Returns `None` when the mount is not permitted.
async fn split_library() -> Option<(TempDir, Mounted, FilesystemStorage)> {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().join("library");
    let elsewhere = root.join("elsewhere");

    std::fs::create_dir_all(&elsewhere).expect("create tree");
    std::fs::write(root.join("here.txt"), b"on the first disk").expect("write file");

    let mount = mount_tmpfs(&elsewhere)?;

    // Written after mounting, so it genuinely lives on the second
    // filesystem rather than under it.
    std::fs::write(elsewhere.join("there.txt"), b"on the second disk").expect("write file");

    let storage = FilesystemStorage::open(&root).await.expect("open root");

    // The premise of every test in this file. If these ever match, the
    // mount silently did not happen and the tests below would pass
    // without exercising anything.
    let first = std::fs::metadata(&root).expect("stat root");
    let second = std::fs::metadata(&elsewhere).expect("stat mount");
    assert_ne!(
        std::os::unix::fs::MetadataExt::dev(&first),
        std::os::unix::fs::MetadataExt::dev(&second),
        "the two paths are on the same filesystem, so nothing here is being tested"
    );

    Some((temp, mount, storage))
}

/// Announces a skip in the one case where these tests cannot run.
macro_rules! library_or_skip {
    () => {
        match split_library().await {
            Some(parts) => parts,
            None => {
                eprintln!(
                    "skipped: mounting a tmpfs is not permitted here, \
                     so the cross-filesystem paths were not exercised"
                );
                return;
            }
        }
    };
}

fn path(raw: &str) -> LibraryPath {
    LibraryPath::parse(raw).expect("valid test path")
}

#[tokio::test]
async fn an_upload_finishes_onto_another_disk() {
    let (_temp, _mount, storage) = library_or_skip!();
    let destination = path("elsewhere/uploaded.txt");

    // Staging lives at the root, so finishing this upload has to cross
    // the boundary — which is exactly what `hard_link` refuses to do.
    let mut staged = storage.begin_upload(1024).await.expect("begin upload");
    staged
        .write_chunk(b"across the divide")
        .await
        .expect("write");

    storage
        .finish_upload(staged, &destination)
        .await
        .expect("an upload onto a second filesystem should still finish");

    let entry = storage.stat(&destination).await.expect("stat");
    assert_eq!(entry.size_bytes, 17);
}

#[tokio::test]
async fn an_upload_onto_another_disk_still_refuses_an_existing_name() {
    let (_temp, _mount, storage) = library_or_skip!();

    // The property the fallback most easily loses. `hard_link` refuses
    // an existing destination atomically; a copy that opened the file
    // for writing would overwrite somebody's photo instead.
    let staged = storage.begin_upload(1024).await.expect("begin upload");
    let error = storage
        .finish_upload(staged, &path("elsewhere/there.txt"))
        .await;

    assert!(
        matches!(error, Err(StorageError::AlreadyExists)),
        "{error:?}"
    );
    assert_eq!(
        std::fs::read_to_string(_temp.path().join("library/elsewhere/there.txt")).expect("read"),
        "on the second disk",
        "the existing file was overwritten"
    );
}

#[tokio::test]
async fn a_failed_upload_leaves_no_half_file_on_the_other_disk() {
    let (temp, _mount, storage) = library_or_skip!();

    // More bytes than the tmpfs can hold, so the copy fails partway.
    let mut staged = storage.begin_upload(64 * 1024 * 1024).await.expect("begin");
    let chunk = vec![b'x'; 1024 * 1024];
    for _ in 0..24 {
        if staged.write_chunk(&chunk).await.is_err() {
            break;
        }
    }

    let destination = path("elsewhere/too-big.bin");
    let result = storage.finish_upload(staged, &destination).await;

    if result.is_ok() {
        // The staging filesystem was too small to build the oversized
        // file in the first place; nothing to assert about the copy.
        return;
    }

    assert!(
        storage.stat(&destination).await.is_err(),
        "a failed copy left a partial file where a whole one belongs"
    );
    assert!(
        !temp.path().join("library/elsewhere/too-big.bin").exists(),
        "a failed copy left a partial file on disk"
    );
}

#[tokio::test]
async fn a_file_moves_between_disks() {
    let (temp, _mount, storage) = library_or_skip!();

    storage
        .move_entry(&path("elsewhere/there.txt"), &path("moved-here.txt"))
        .await
        .expect("a file should move off the second filesystem");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/moved-here.txt")).expect("read"),
        "on the second disk"
    );
    assert!(
        storage.stat(&path("elsewhere/there.txt")).await.is_err(),
        "the original outlived the move, so the file now exists twice"
    );
}

#[tokio::test]
async fn a_folder_moves_between_disks_with_everything_in_it() {
    let (temp, _mount, storage) = library_or_skip!();

    let nested = temp.path().join("library/elsewhere/trip/day-one");
    std::fs::create_dir_all(&nested).expect("create nested tree");
    std::fs::write(nested.join("beach.jpg"), b"jpeg").expect("write");
    std::fs::write(
        temp.path().join("library/elsewhere/trip/plans.txt"),
        b"plans",
    )
    .expect("write");

    storage
        .move_entry(&path("elsewhere/trip"), &path("trip"))
        .await
        .expect("a folder should move off the second filesystem");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/trip/day-one/beach.jpg")).expect("read"),
        "jpeg",
        "a file nested two deep did not survive the move"
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/trip/plans.txt")).expect("read"),
        "plans"
    );
    assert!(
        !temp.path().join("library/elsewhere/trip").exists(),
        "the original folder outlived the move"
    );
}

#[tokio::test]
async fn a_file_on_another_disk_can_be_trashed_and_restored() {
    let (temp, _mount, storage) = library_or_skip!();

    // The trash lives at the library root, so this crosses the boundary
    // in one direction and the restore crosses it back.
    let trashed = storage
        .move_to_trash(&path("elsewhere/there.txt"))
        .await
        .expect("a file on a second filesystem should be trashable");

    assert!(storage.stat(&path("elsewhere/there.txt")).await.is_err());

    storage
        .move_entry(&trashed, &path("elsewhere/there.txt"))
        .await
        .expect("restore");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/elsewhere/there.txt")).expect("read"),
        "on the second disk",
        "the file came back changed"
    );
}

#[tokio::test]
async fn a_version_survives_a_trip_to_another_disk_and_back() {
    let (temp, _mount, storage) = library_or_skip!();

    // Versions are kept at the root; the file being replaced is not.
    let name = storage
        .keep_version(&path("elsewhere/there.txt"))
        .await
        .expect("keep a version of a file on a second filesystem");

    assert!(
        storage.stat(&path("elsewhere/there.txt")).await.is_err(),
        "keeping a version is a move, so the original should be gone"
    );

    storage
        .restore_version(&name, &path("elsewhere/there.txt"))
        .await
        .expect("restore the version onto the second filesystem");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("library/elsewhere/there.txt")).expect("read"),
        "on the second disk",
        "the version came back changed"
    );
}
