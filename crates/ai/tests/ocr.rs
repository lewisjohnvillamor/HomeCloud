//! Text recognition against the real tool, when it is installed.
//!
//! Skipped with a note when it is not, the same way the database tests
//! skip without PostgreSQL: a contributor without Tesseract still gets a
//! useful `cargo test`, and a deployment that has it gets the real
//! thing exercised.

use std::path::Path;
use std::process::Stdio;

/// A font every mainstream distribution ships. The fixture needs *a*
/// real typeface; which one does not matter.
const FONTS: [&str; 3] = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
];

/// Renders black text on white — which is what a scanned page is.
///
/// Drawn with FFmpeg, which this project already depends on for video
/// posters, rather than by hand: an earlier version of this test drew
/// its own blocky font and Tesseract read "INVOICE 88213" as "THUDICE
/// Bae14". That measured the fixture, not the code.
async fn scan_of(path: &Path, text: &str) -> bool {
    let Some(font) = FONTS
        .iter()
        .find(|candidate| Path::new(candidate).is_file())
    else {
        return false;
    };

    tokio::process::Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error"])
        .args(["-f", "lavfi", "-i", "color=white:s=900x200"])
        .arg("-vf")
        .arg(format!(
            "drawtext=fontfile={font}:text='{text}':fontcolor=black:fontsize=72:x=40:y=60"
        ))
        .args(["-frames:v", "1"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[tokio::test]
async fn text_in_a_picture_is_read_back() {
    if !homecloud_ai::ocr::is_available().await {
        eprintln!("skipping: tesseract is not installed");
        return;
    }

    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("scan.png");

    if !scan_of(&path, "INVOICE 88213").await {
        eprintln!("skipping: no ffmpeg or no font to render the fixture with");
        return;
    }

    let text = homecloud_ai::ocr::read_text(&path).await.expect("text");

    // What a person would actually search for, rather than the exact
    // string: recognition is never guaranteed to be perfect and this
    // test is about the integration, not about Tesseract's accuracy.
    assert!(
        text.contains("88213"),
        "the invoice number did not survive: {text:?}"
    );
}

#[tokio::test]
async fn a_picture_with_no_text_reads_as_empty_rather_than_failing() {
    if !homecloud_ai::ocr::is_available().await {
        eprintln!("skipping: tesseract is not installed");
        return;
    }

    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("beach.png");

    // A gradient: a photograph, as far as a recogniser is concerned.
    // Finding nothing is the correct answer about a picture of a beach,
    // and the pipeline must not treat it as a failure.
    let mut canvas = image::RgbImage::new(400, 300);
    for (x, y, pixel) in canvas.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 180]);
    }
    canvas.save(&path).expect("write the test image");

    let text = homecloud_ai::ocr::read_text(&path)
        .await
        .expect("an answer");

    assert_eq!(text, "");
}

#[tokio::test]
async fn a_file_that_is_not_there_is_reported_plainly() {
    let missing = Path::new("/nonexistent/scan.png");

    let error = homecloud_ai::ocr::read_text(missing).await.unwrap_err();

    assert!(matches!(error, homecloud_ai::AiError::Unreadable(_)));
}
