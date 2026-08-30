//! Video poster frames.
//!
//! FFmpeg runs as a child process rather than a linked library: it is a
//! very large C codebase being fed files that arrived from cameras,
//! phones, and downloads, and a child process is something the server
//! can put a wall clock and a memory bound around — and kill.
//!
//! Nothing here builds a shell command line. Arguments are passed as an
//! argument vector, so a file named `; rm -rf ~` is just a file name.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

/// Longest a single poster extraction may take. A video that cannot
/// produce one frame in this long is not worth the server's time, and
/// the process is killed rather than waited on.
pub const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(20);

/// Where in the video the frame is taken from.
///
/// A fraction rather than a fixed timestamp: three seconds into a
/// two-second clip is nothing, and the first frame of most videos is
/// black. A quarter of the way in is usually the actual subject.
const SEEK_FRACTION: f64 = 0.25;

/// Fallback offset when the duration cannot be read.
const FALLBACK_SEEK_SECONDS: f64 = 1.0;

/// Largest poster FFmpeg may write. A bound on output as well as time,
/// so a pathological file cannot fill memory through the pipe.
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VideoError {
    #[error("this server has no video support installed")]
    Unavailable,
    #[error("the video could not be read")]
    Unreadable,
    #[error("reading the video took too long")]
    TimedOut,
}

/// Whether a content type is one a poster can be taken from.
pub fn is_video(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| value.starts_with("video/"))
}

/// Whether FFmpeg is present.
///
/// Checked once at startup and reported, so an operator learns that
/// video previews are off from a log line rather than from a puzzling
/// gap in their Photos view.
pub async fn is_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Extracts a single JPEG frame from a video file.
///
/// Takes a path rather than bytes: FFmpeg reads the file itself, so a
/// four-gigabyte video costs no memory here.
pub async fn poster_frame(path: &Path, max_edge: u32) -> Result<Vec<u8>, VideoError> {
    let seek = seek_offset(path).await;

    // `-nostdin` so a prompt cannot hang the child; `-map 0:v:0` so an
    // attached cover image or subtitle stream cannot be chosen instead
    // of the video; `-frames:v 1` so exactly one frame comes back.
    let output = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-ss",
            &format!("{seek:.3}"),
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            // Scale the long edge down, never up, and keep the aspect
            // ratio; `-2` keeps the other dimension even, which the JPEG
            // encoder requires.
            "-vf",
            &format!("scale='min({max_edge},iw)':-2"),
            "-f",
            "image2",
            "-vcodec",
            "mjpeg",
            "-q:v",
            "4",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The child dies with the task rather than outliving a cancelled
        // request.
        .kill_on_drop(true)
        .output();

    let output = match timeout(EXTRACTION_TIMEOUT, output).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(VideoError::Unavailable);
        }
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "ffmpeg could not be run");
            return Err(VideoError::Unreadable);
        }
        Err(_) => {
            tracing::warn!(
                timeout_s = EXTRACTION_TIMEOUT.as_secs(),
                "gave up extracting a video poster"
            );
            return Err(VideoError::TimedOut);
        }
    };

    if !output.status.success() || output.stdout.is_empty() {
        // FFmpeg's message goes to the log, never to the client: it
        // contains the host path.
        tracing::debug!(
            detail = %String::from_utf8_lossy(&output.stderr).chars().take(200).collect::<String>(),
            "a video produced no poster frame"
        );
        return Err(VideoError::Unreadable);
    }

    if output.stdout.len() > MAX_OUTPUT_BYTES {
        return Err(VideoError::Unreadable);
    }

    Ok(output.stdout)
}

/// Where to seek to, from the video's own duration.
///
/// A failure here is not fatal: an unreadable duration falls back to a
/// fixed offset, and the extraction itself decides whether the file is
/// usable.
async fn seek_offset(path: &Path) -> f64 {
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();

    let Ok(Ok(output)) = timeout(EXTRACTION_TIMEOUT, probe).await else {
        return FALLBACK_SEEK_SECONDS;
    };

    let duration: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0.0);

    if duration.is_finite() && duration > 0.0 {
        (duration * SEEK_FRACTION).min(duration)
    } else {
        FALLBACK_SEEK_SECONDS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_video_content_types_get_a_poster() {
        assert!(is_video(Some("video/mp4")));
        assert!(is_video(Some("video/quicktime")));
        assert!(!is_video(Some("image/jpeg")));
        assert!(!is_video(Some("application/pdf")));
        assert!(!is_video(None));
    }
}
