//! Reading text out of pictures of text.
//!
//! Backed by Tesseract, found on `PATH` at runtime exactly as FFmpeg is
//! for video posters. A deployment without it reports the capability as
//! absent and the rest of the product is unaffected — which is the same
//! contract every provider here signs.
//!
//! Tesseract rather than a vision-language model on purpose. The job is
//! narrow, a general model is a few gigabytes and wants a GPU, and this
//! is a few tens of megabytes on any processor. When there is a job no
//! small specialised tool does well, that is the moment to reach for
//! something bigger — not before.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use crate::AiError;

/// Longest a single page may take. A scanned page is a second or two;
/// past this something is wrong and the queue matters more than the
/// page.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Most text kept from one image. A page of prose is a few thousand
/// characters; this is generous for that and bounded against a poster
/// that recognises as noise.
pub const MAX_TEXT_BYTES: usize = 32 * 1024;

/// Whether this machine can read text out of images.
pub async fn is_available() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Reads the text in an image.
///
/// Takes a path rather than bytes: Tesseract opens the file itself, so a
/// forty-megapixel scan costs no memory here — the same reasoning as the
/// video poster path.
///
/// Returns the empty string for a picture with no text in it. That is an
/// ordinary answer about a photograph of a beach, not a failure.
pub async fn read_text(path: &Path) -> Result<String, AiError> {
    if !path.is_file() {
        return Err(AiError::Unreadable(
            "the file is not there any more".to_owned(),
        ));
    }

    let mut command = Command::new("tesseract");
    command
        .arg(path)
        // `-` writes to standard output instead of a file, so nothing is
        // left behind if this is interrupted.
        .arg("-")
        .arg("--psm")
        // Page segmentation 3: automatic, no orientation detection.
        // Orientation detection needs a data file that is packaged
        // separately, and its absence would fail every call.
        .arg("3")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = match tokio::time::timeout(TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AiError::Unavailable("text recognition"))
        }
        Ok(Err(error)) => return Err(AiError::Failed(error.to_string())),
        Err(_) => return Err(AiError::Failed("timed out".to_owned())),
    };

    if !output.status.success() {
        // Tesseract's own diagnostics go to the log, never to a caller:
        // they name paths on the server.
        tracing::debug!(
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "text recognition did not succeed"
        );

        return Err(AiError::Failed("the image could not be read".to_owned()));
    }

    Ok(tidy(&String::from_utf8_lossy(&output.stdout)))
}

/// Reduces recognised text to something worth storing.
///
/// Recognition of a photograph that is not text produces scattered
/// punctuation and single letters. Collapsing whitespace and dropping
/// the result when almost nothing survives keeps that out of the index,
/// where it would match searches for nothing in particular.
fn tidy(raw: &str) -> String {
    let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated = match collapsed.char_indices().nth(MAX_TEXT_BYTES) {
        Some((index, _)) => collapsed[..index].to_owned(),
        None => collapsed,
    };

    // Fewer than this many letters and digits means the picture was not
    // of text, whatever the recogniser thought it saw.
    let meaningful = truncated
        .chars()
        .filter(|character| character.is_alphanumeric())
        .count();

    if meaningful < 8 {
        return String::new();
    }

    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_collapsed_so_a_scan_reads_as_prose() {
        assert_eq!(
            tidy("  Invoice   88213\n\n for  one  generator \n"),
            "Invoice 88213 for one generator"
        );
    }

    #[test]
    fn a_photograph_that_is_not_text_produces_nothing() {
        // What recognising a beach actually returns: stray marks the
        // recogniser took for letters. In the index this would match
        // searches for nothing in particular.
        assert_eq!(tidy(". , : |  '  ~"), "");
        assert_eq!(tidy("a b"), "");
        assert_eq!(tidy(""), "");
    }

    #[test]
    fn a_short_but_real_line_is_kept() {
        assert_eq!(tidy("Gate 42B open"), "Gate 42B open");
    }

    #[test]
    fn a_wall_of_text_is_bounded_rather_than_refused() {
        let long = "word ".repeat(20_000);
        let tidied = tidy(&long);

        assert!(tidied.chars().count() <= MAX_TEXT_BYTES);
        assert!(tidied.starts_with("word word"));
    }
}
