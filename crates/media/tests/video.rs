//! Video poster frames, tested against real files produced by FFmpeg
//! itself — and against the files an attacker would leave in a library.

use std::path::Path;
use std::process::Command;

use homecloud_media::video::{self, VideoError};
use tempfile::TempDir;

/// Renders a short test video. Skips the whole test when FFmpeg is not
/// installed, the same way the server degrades.
fn make_video(directory: &Path, name: &str, seconds: u32) -> Option<std::path::PathBuf> {
    let path = directory.join(name);

    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=size=640x480:rate=10:duration={seconds}"),
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .status()
        .ok()?;

    status.success().then_some(path)
}

fn skip_without_ffmpeg() -> bool {
    let available = Command::new("ffmpeg").arg("-version").output().is_ok();

    if !available {
        eprintln!("skipping video test: ffmpeg is not installed");
    }

    !available
}

#[tokio::test]
async fn a_video_produces_a_readable_poster_frame() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "clip.mp4", 2) else {
        return;
    };

    let poster = video::poster_frame(&video, 320).await.expect("a poster");

    let image = image::load_from_memory(&poster).expect("the poster is a readable image");
    assert_eq!(image.width(), 320, "the long edge is scaled to the request");
    assert!(image.height() > 0);
}

#[tokio::test]
async fn a_small_video_is_not_scaled_up() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "clip.mp4", 1) else {
        return;
    };

    // The source is 640 wide; asking for 1280 must not enlarge it.
    let poster = video::poster_frame(&video, 1280).await.expect("a poster");

    let image = image::load_from_memory(&poster).expect("a readable image");
    assert_eq!(image.width(), 640);
}

#[tokio::test]
async fn a_file_that_is_not_a_video_is_refused_cleanly() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("trojan.mp4");
    std::fs::write(&path, b"#!/bin/sh\nrm -rf /\n").expect("write file");

    let error = video::poster_frame(&path, 320)
        .await
        .expect_err("not a video");

    assert_eq!(error, VideoError::Unreadable);
}

#[tokio::test]
async fn a_truncated_video_does_not_hang_or_panic() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "clip.mp4", 2) else {
        return;
    };
    let bytes = std::fs::read(&video).expect("read");
    let truncated = temp.path().join("truncated.mp4");
    std::fs::write(&truncated, &bytes[..bytes.len() / 4]).expect("write");

    // Either a frame comes back from the surviving header, or it is
    // refused. What must not happen is a hang or a panic.
    let outcome = video::poster_frame(&truncated, 320).await;

    assert!(
        outcome.is_ok() || outcome == Err(VideoError::Unreadable),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_missing_file_is_refused() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");

    let error = video::poster_frame(&temp.path().join("nothing.mp4"), 320)
        .await
        .expect_err("missing file");

    assert_eq!(error, VideoError::Unreadable);
}

#[tokio::test]
async fn a_hostile_file_name_is_an_argument_not_a_command() {
    if skip_without_ffmpeg() {
        return;
    }
    let temp = TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "source.mp4", 1) else {
        return;
    };

    // A name that would run two extra commands if it ever reached a
    // shell. No slashes: this is a file name, not a path.
    let hostile = temp.path().join("clip; touch pwned; echo $(whoami).mp4");
    std::fs::copy(&video, &hostile).expect("copy");

    let poster = video::poster_frame(&hostile, 320).await;

    assert!(
        poster.is_ok(),
        "the file should still be readable: {poster:?}"
    );
    // Nothing ran: no marker in the video's directory, and none in the
    // working directory the test itself runs from.
    assert!(
        !temp.path().join("pwned").exists(),
        "the file name was executed as a command"
    );
    assert!(
        !Path::new("pwned").exists(),
        "a command escaped into the working directory"
    );
}

#[tokio::test]
async fn availability_is_reported_rather_than_assumed() {
    // True in this environment; the point of the check is that the
    // server asks instead of assuming.
    assert!(video::is_available().await == Command::new("ffmpeg").arg("-version").output().is_ok());
}
