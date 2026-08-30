//! Thumbnail generation.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits};

/// Largest source file accepted. A file bigger than this is almost
/// certainly not a photo anyone wants a thumbnail of, and reading it
/// would cost more than the result is worth.
pub const MAX_SOURCE_BYTES: u64 = 96 * 1024 * 1024;

/// Largest source image accepted, in pixels.
///
/// This is the decompression-bomb bound: a few kilobytes of PNG can
/// describe a 50,000 × 50,000 canvas, which would need gigabytes of RAM
/// once decoded. 80 megapixels leaves room for high-end cameras.
pub const MAX_SOURCE_PIXELS: u64 = 80_000_000;

/// Memory the decoder may allocate for one image.
const MAX_DECODE_ALLOCATION: u64 = 512 * 1024 * 1024;

/// JPEG quality for derivatives. High enough that a thumbnail does not
/// look degraded, low enough to be worth caching.
const JPEG_QUALITY: u8 = 82;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MediaError {
    #[error("the file is not an image this server can read")]
    UnsupportedFormat,
    #[error("the image is too large to process")]
    TooLarge,
    #[error("the image data is damaged or incomplete")]
    Damaged,
    #[error("the thumbnail could not be encoded")]
    Encoding,
}

/// The sizes the product asks for. A closed set, so a client cannot ask
/// the server to render arbitrary dimensions on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailSize {
    /// Grid tiles and list rows.
    Small,
    /// Larger grids and phone-sized previews.
    Medium,
    /// A detail view, still far cheaper than the original.
    Large,
}

impl ThumbnailSize {
    /// Longest edge of the result, in pixels.
    pub const fn max_edge(self) -> u32 {
        match self {
            ThumbnailSize::Small => 320,
            ThumbnailSize::Medium => 640,
            ThumbnailSize::Large => 1280,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ThumbnailSize::Small => "small",
            ThumbnailSize::Medium => "medium",
            ThumbnailSize::Large => "large",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "small" => Some(ThumbnailSize::Small),
            "medium" => Some(ThumbnailSize::Medium),
            "large" => Some(ThumbnailSize::Large),
            _ => None,
        }
    }
}

/// Whether a content type is one this crate can render a thumbnail for.
///
/// Advisory only: it saves reading a file that certainly will not work.
/// The real decision is made by the decoder, from the bytes.
pub fn is_thumbnailable(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some(
            "image/jpeg"
                | "image/png"
                | "image/gif"
                | "image/webp"
                | "image/bmp"
                | "image/tiff"
                | "image/x-tiff"
        )
    )
}

/// Decodes `source` and produces a JPEG thumbnail.
///
/// Blocking and CPU-bound by nature: callers must run it on a blocking
/// pool, never on an async request executor.
pub fn generate_thumbnail(source: &[u8], size: ThumbnailSize) -> Result<Vec<u8>, MediaError> {
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(MediaError::TooLarge);
    }

    // Format is guessed from the bytes. A file called `holiday.jpg` that
    // is really something else must not decide which decoder runs.
    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| MediaError::Damaged)?;

    if reader.format().is_none() {
        return Err(MediaError::UnsupportedFormat);
    }

    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOCATION);
    reader.limits(limits);

    // Dimensions come from the header, before any pixel data is
    // allocated, so a decompression bomb is refused rather than decoded.
    match reader.into_dimensions() {
        Ok((width, height)) if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS => {
            tracing::warn!(width, height, "refused an oversized image");
            return Err(MediaError::TooLarge);
        }
        Ok(_) => {}
        Err(error) => return Err(classify(error)),
    }

    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| MediaError::Damaged)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOCATION);
    reader.limits(limits);

    let image = reader.decode().map_err(classify)?;

    // `thumbnail` is a cheap box filter; `resize` with Lanczos is what
    // makes a downscaled photo still look like the photo.
    let edge = size.max_edge();
    let scaled = if image.width().max(image.height()) <= edge {
        image
    } else {
        image.resize(edge, edge, FilterType::Lanczos3)
    };

    // JPEG: universally supported, and a thumbnail has no transparency
    // worth preserving. Flattening to RGB8 avoids an encoder error on
    // images that carry an alpha channel.
    let rgb = scaled.to_rgb8();
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| {
            tracing::warn!(error = %error, "thumbnail encoding failed");
            MediaError::Encoding
        })?;

    Ok(output)
}

/// Content type of everything this crate produces.
pub const DERIVATIVE_CONTENT_TYPE: &str = "image/jpeg";

fn classify(error: image::ImageError) -> MediaError {
    match error {
        image::ImageError::Unsupported(_) => MediaError::UnsupportedFormat,
        image::ImageError::Limits(_) => MediaError::TooLarge,
        // Everything else is a damaged or truncated file, which is
        // common enough in a real library to be an expected outcome.
        _ => MediaError::Damaged,
    }
}

/// Format tag used when naming a cached derivative.
pub fn source_format(source: &[u8]) -> Option<ImageFormat> {
    image::guess_format(source).ok()
}
