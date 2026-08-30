//! Text extraction.
//!
//! Every input here came from somewhere else — an email attachment, a
//! phone, a download — so extraction is treated as parsing hostile data:
//! bounded input, bounded output, no panics escaping, and no trust in
//! what a file claims to be.

/// Largest file read for extraction. A document larger than this is
/// almost never one whose text is worth having, and reading it costs
/// memory the server may need elsewhere.
pub const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;

/// Longest text stored per document.
///
/// Truncation rather than refusal: the beginning of a document is what
/// identifies it, and an unbounded column would let one file dominate
/// the index.
pub const MAX_TEXT_CHARS: usize = 200_000;

/// Why a file has no indexed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Indexed,
    Unsupported,
    TooLarge,
    Failed,
}

impl Status {
    pub const fn as_str(self) -> &'static str {
        match self {
            Status::Indexed => "indexed",
            Status::Unsupported => "unsupported",
            Status::TooLarge => "too_large",
            Status::Failed => "failed",
        }
    }
}

/// The outcome of trying to read a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    pub text: String,
    pub status: Status,
    /// True when the document was longer than [`MAX_TEXT_CHARS`].
    pub truncated: bool,
}

impl Extraction {
    fn empty(status: Status) -> Self {
        Self {
            text: String::new(),
            status,
            truncated: false,
        }
    }
}

/// Content types this crate can read.
///
/// Advisory: it saves reading a file that certainly will not work. The
/// bytes still decide.
pub fn is_extractable(content_type: Option<&str>, name: &str) -> bool {
    if matches!(content_type, Some("application/pdf")) {
        return true;
    }
    if content_type.is_some_and(|value| value.starts_with("text/")) {
        return true;
    }
    if matches!(
        content_type,
        Some("application/json" | "application/xml" | "application/x-yaml" | "text/yaml")
    ) {
        return true;
    }

    // Files a developer or a note-taker has plenty of, which browsers do
    // not always give a text content type.
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());

    matches!(
        extension.as_deref(),
        Some(
            "txt"
                | "md"
                | "markdown"
                | "csv"
                | "tsv"
                | "log"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "ini"
                | "conf"
                | "rst"
                | "org"
                | "tex"
        )
    )
}

/// Extracts text from a document.
///
/// Blocking and CPU-bound: callers must run it on a blocking pool.
/// Never returns an error — a file that cannot be read is a fact about
/// the file, recorded as a status so the scan does not retry it forever.
pub fn extract(source: &[u8], content_type: Option<&str>, name: &str) -> Extraction {
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Extraction::empty(Status::TooLarge);
    }
    if !is_extractable(content_type, name) {
        return Extraction::empty(Status::Unsupported);
    }

    // PDF is decided by its signature, not by the name: a file called
    // `.pdf` that is really something else must not reach the PDF parser.
    if source.starts_with(b"%PDF-") {
        return extract_pdf(source);
    }
    if matches!(content_type, Some("application/pdf")) {
        // Claimed to be a PDF and is not; nothing else will read it.
        return Extraction::empty(Status::Failed);
    }

    extract_plain_text(source)
}

/// Reads a text file.
///
/// Invalid UTF-8 is replaced rather than rejected: a log file with one
/// bad byte is still worth searching. A file that is mostly unprintable
/// is treated as binary and skipped, because indexing it would fill the
/// index with noise.
fn extract_plain_text(source: &[u8]) -> Extraction {
    if looks_binary(source) {
        return Extraction::empty(Status::Unsupported);
    }

    let text = String::from_utf8_lossy(source);

    finish(normalise(&text))
}

fn extract_pdf(source: &[u8]) -> Extraction {
    // `pdf-extract` parses untrusted structure and can panic on a
    // malformed file; a document nobody can read must not take the
    // indexing task down with it.
    let parsed = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(source));

    match parsed {
        Ok(Ok(text)) => finish(normalise(&text)),
        Ok(Err(error)) => {
            tracing::debug!(error = %error, "a pdf could not be read");
            Extraction::empty(Status::Failed)
        }
        Err(_) => {
            tracing::warn!("a pdf caused the extractor to panic; recorded as unreadable");
            Extraction::empty(Status::Failed)
        }
    }
}

/// Collapses runs of whitespace and drops control characters.
///
/// Extracted text is full of layout artefacts — form feeds, repeated
/// newlines, stray nulls — that add nothing to a search index and make
/// snippets ugly.
fn normalise(text: &str) -> String {
    let mut output = String::with_capacity(text.len().min(MAX_TEXT_CHARS * 2));
    let mut pending_space = false;

    for character in text.chars() {
        if character == '\0' {
            continue;
        }

        if character.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if character.is_control() {
            continue;
        }

        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);

        // Two characters of slack so the truncation check below sees a
        // string that is definitely long enough.
        if output.chars().count() >= MAX_TEXT_CHARS + 2 {
            break;
        }
    }

    output
}

fn finish(text: String) -> Extraction {
    let mut truncated = false;
    let text = if text.chars().count() > MAX_TEXT_CHARS {
        truncated = true;
        text.chars().take(MAX_TEXT_CHARS).collect()
    } else {
        text
    };

    if text.trim().is_empty() {
        // A file with no words in it — an empty note, or a scan of a
        // photograph. Recorded as read, with nothing to index.
        return Extraction {
            text: String::new(),
            status: Status::Indexed,
            truncated,
        };
    }

    Extraction {
        text,
        status: Status::Indexed,
        truncated,
    }
}

/// Whether a byte run looks like binary rather than text.
///
/// Judged on the first few kilobytes: a null byte, or a high proportion
/// of unprintable bytes, means this is not a document.
fn looks_binary(source: &[u8]) -> bool {
    let sample = &source[..source.len().min(8192)];
    if sample.is_empty() {
        return false;
    }
    if sample.contains(&0) {
        return true;
    }

    let unprintable = sample
        .iter()
        .filter(|byte| **byte < 0x09 || (**byte > 0x0D && **byte < 0x20))
        .count();

    unprintable * 100 / sample.len() > 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_read_and_normalised() {
        let extraction = extract(b"Router:   living room\n\n\nshelf.\n", None, "notes.txt");

        assert_eq!(extraction.status, Status::Indexed);
        assert_eq!(extraction.text, "Router: living room shelf.");
        assert!(!extraction.truncated);
    }

    #[test]
    fn a_long_document_is_truncated_rather_than_refused() {
        let source = "word ".repeat(MAX_TEXT_CHARS);

        let extraction = extract(source.as_bytes(), Some("text/plain"), "long.txt");

        assert_eq!(extraction.status, Status::Indexed);
        assert!(extraction.truncated);
        assert_eq!(extraction.text.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn an_enormous_file_is_not_read_at_all() {
        let source = vec![b'a'; MAX_SOURCE_BYTES as usize + 1];

        let extraction = extract(&source, Some("text/plain"), "huge.txt");

        assert_eq!(extraction.status, Status::TooLarge);
        assert!(extraction.text.is_empty());
    }

    #[test]
    fn binary_content_is_not_indexed_even_with_a_text_name() {
        let mut source = b"MZ\x90\x00".to_vec();
        source.extend_from_slice(&[0u8; 512]);

        let extraction = extract(&source, Some("text/plain"), "program.txt");

        assert_eq!(extraction.status, Status::Unsupported);
    }

    #[test]
    fn invalid_utf8_is_recovered_rather_than_rejected() {
        let source = b"caf\xC3\xA9 and a bad byte \xFF here";

        let extraction = extract(source, Some("text/plain"), "notes.txt");

        assert_eq!(extraction.status, Status::Indexed);
        assert!(extraction.text.contains("café"), "{}", extraction.text);
    }

    #[test]
    fn a_file_claiming_to_be_a_pdf_but_is_not_is_recorded_as_unreadable() {
        let extraction = extract(b"just some words", Some("application/pdf"), "invoice.pdf");

        assert_eq!(extraction.status, Status::Failed);
    }

    #[test]
    fn a_truncated_pdf_does_not_panic() {
        let extraction = extract(
            b"%PDF-1.7\n1 0 obj\n<< /Type",
            Some("application/pdf"),
            "x.pdf",
        );

        assert!(matches!(
            extraction.status,
            Status::Failed | Status::Indexed
        ));
    }

    #[test]
    fn formats_without_text_are_skipped() {
        assert!(!is_extractable(Some("image/jpeg"), "beach.jpg"));
        assert!(!is_extractable(Some("video/mp4"), "clip.mp4"));
        assert!(!is_extractable(None, "archive.zip"));

        assert!(is_extractable(Some("application/pdf"), "invoice.pdf"));
        assert!(is_extractable(Some("text/plain"), "notes.txt"));
        assert!(is_extractable(None, "README.md"));
        assert!(is_extractable(None, "data.CSV"));
    }

    #[test]
    fn an_empty_file_is_read_with_nothing_to_index() {
        let extraction = extract(b"", Some("text/plain"), "empty.txt");

        assert_eq!(extraction.status, Status::Indexed);
        assert!(extraction.text.is_empty());
    }
}
