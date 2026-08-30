//! Reading a real EXIF header, not a hand-waved one.
//!
//! The block is assembled byte by byte here rather than committing a
//! binary fixture, so what is being parsed is visible in the test.

use homecloud_media::exif;

/// Tags used below, by their EXIF numbers.
const MAKE: u16 = 0x010f;
const MODEL: u16 = 0x0110;
const ORIENTATION: u16 = 0x0112;
const EXIF_IFD_POINTER: u16 = 0x8769;
const DATE_TIME_ORIGINAL: u16 = 0x9003;

const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;

/// One IFD entry, with its value either inline or in the heap that
/// follows the directory.
struct Entry {
    tag: u16,
    format: u16,
    /// Value bytes. Four or fewer live inside the entry itself.
    value: Vec<u8>,
    /// Number of components, which is not always the byte length.
    count: u32,
}

fn ascii(tag: u16, text: &str) -> Entry {
    let mut value = text.as_bytes().to_vec();
    value.push(0);
    let count = value.len() as u32;

    Entry {
        tag,
        format: ASCII,
        value,
        count,
    }
}

fn short(tag: u16, number: u16) -> Entry {
    Entry {
        tag,
        format: SHORT,
        value: number.to_le_bytes().to_vec(),
        count: 1,
    }
}

fn long(tag: u16, number: u32) -> Entry {
    Entry {
        tag,
        format: LONG,
        value: number.to_le_bytes().to_vec(),
        count: 1,
    }
}

/// Serialises one image file directory at `offset` within the TIFF
/// block, returning the directory and the heap it points into.
fn directory(entries: &[Entry], heap_start: usize) -> (Vec<u8>, Vec<u8>) {
    let mut body = Vec::new();
    let mut heap = Vec::new();

    body.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    for entry in entries {
        body.extend_from_slice(&entry.tag.to_le_bytes());
        body.extend_from_slice(&entry.format.to_le_bytes());
        body.extend_from_slice(&entry.count.to_le_bytes());

        if entry.value.len() <= 4 {
            let mut inline = entry.value.clone();
            inline.resize(4, 0);
            body.extend_from_slice(&inline);
        } else {
            let at = heap_start + heap.len();
            body.extend_from_slice(&(at as u32).to_le_bytes());
            heap.extend_from_slice(&entry.value);
        }
    }

    // No next directory.
    body.extend_from_slice(&0u32.to_le_bytes());

    (body, heap)
}

/// A JPEG whose APP1 segment carries the given camera and capture time.
fn jpeg_with_exif(make: &str, model: &str, taken: &str, orientation: u16) -> Vec<u8> {
    // The Exif IFD comes after IFD0. Sizes are fixed by the entry
    // counts, so both offsets can be computed up front.
    const HEADER: usize = 8;
    let ifd0_entries = 4;
    let exif_entries = 1;

    let ifd0_size = 2 + ifd0_entries * 12 + 4;
    let exif_size = 2 + exif_entries * 12 + 4;

    let ifd0_at = HEADER;
    let ifd0_heap_at = ifd0_at + ifd0_size;

    // IFD0's heap holds the make and model strings.
    let ifd0_heap_len = make.len() + 1 + model.len() + 1;
    let exif_at = ifd0_heap_at + ifd0_heap_len;
    let exif_heap_at = exif_at + exif_size;

    let (ifd0, ifd0_heap) = directory(
        &[
            ascii(MAKE, make),
            ascii(MODEL, model),
            short(ORIENTATION, orientation),
            long(EXIF_IFD_POINTER, exif_at as u32),
        ],
        ifd0_heap_at,
    );

    let (exif_ifd, exif_heap) = directory(&[ascii(DATE_TIME_ORIGINAL, taken)], exif_heap_at);

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&(ifd0_at as u32).to_le_bytes());
    tiff.extend_from_slice(&ifd0);
    tiff.extend_from_slice(&ifd0_heap);
    tiff.extend_from_slice(&exif_ifd);
    tiff.extend_from_slice(&exif_heap);

    let mut app1 = Vec::new();
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&tiff);

    let mut jpeg = vec![0xff, 0xd8];
    jpeg.extend_from_slice(&[0xff, 0xe1]);
    jpeg.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(&app1);
    jpeg.extend_from_slice(&[0xff, 0xd9]);

    jpeg
}

#[test]
fn a_photo_reports_when_it_was_taken() {
    let photo = jpeg_with_exif("Fujifilm", "X100V", "2019:07:04 12:30:45", 1);
    let metadata = exif::read(&photo);

    let taken = metadata.taken_at.expect("a capture time");
    assert_eq!(taken.year(), 2019);
    assert_eq!(taken.month() as u8, 7);
    assert_eq!(taken.day(), 4);
    assert_eq!(taken.hour(), 12);
    assert_eq!(taken.minute(), 30);
    assert_eq!(taken.second(), 45);
}

#[test]
fn a_camera_is_named_once() {
    let repeated = exif::read(&jpeg_with_exif(
        "NIKON CORPORATION",
        "NIKON D750",
        "2019:07:04 12:30:45",
        1,
    ));
    assert_eq!(repeated.camera.as_deref(), Some("NIKON D750"));

    let distinct = exif::read(&jpeg_with_exif(
        "Fujifilm",
        "X100V",
        "2019:07:04 12:30:45",
        1,
    ));
    assert_eq!(distinct.camera.as_deref(), Some("Fujifilm X100V"));
}

#[test]
fn the_way_the_camera_was_held_is_kept() {
    let sideways = exif::read(&jpeg_with_exif(
        "Apple",
        "iPhone 15",
        "2024:01:01 09:00:00",
        6,
    ));

    assert_eq!(sideways.orientation, Some(6));
}

#[test]
fn an_absurd_orientation_is_ignored_rather_than_trusted() {
    let nonsense = exif::read(&jpeg_with_exif(
        "Apple",
        "iPhone 15",
        "2024:01:01 09:00:00",
        99,
    ));

    assert_eq!(nonsense.orientation, None);
}

#[test]
fn a_photo_with_a_header_but_no_dates_is_still_read() {
    let photo = jpeg_with_exif("Canon", "EOS R6", "not a date", 1);
    let metadata = exif::read(&photo);

    assert!(metadata.taken_at.is_none());
    assert_eq!(metadata.camera.as_deref(), Some("Canon EOS R6"));
}

/// Splices an EXIF block into a real JPEG, so the decoder has both
/// pixels and a header to disagree about.
fn jpeg_with_pixels_and_exif(width: u32, height: u32, orientation: u16) -> Vec<u8> {
    use std::io::Cursor;

    let mut buffer = image::RgbImage::new(width, height);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]);
    }

    let mut encoded = Vec::new();
    image::DynamicImage::ImageRgb8(buffer)
        .write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Jpeg)
        .expect("encode test image");

    let header = jpeg_with_exif("Apple", "iPhone 15", "2024:01:01 09:00:00", orientation);
    // Everything between the start marker and the end marker of the
    // header-only file is the APP1 segment.
    let app1 = &header[2..header.len() - 2];

    let mut spliced = Vec::new();
    spliced.extend_from_slice(&encoded[..2]);
    spliced.extend_from_slice(app1);
    spliced.extend_from_slice(&encoded[2..]);

    spliced
}

#[test]
fn a_photo_taken_sideways_is_turned_the_right_way_up() {
    use homecloud_media::{generate_thumbnail, ThumbnailSize};

    // A landscape sensor image that the camera says was shot in
    // portrait: orientation 6 means "rotate 90° clockwise to view".
    let sideways = jpeg_with_pixels_and_exif(400, 200, 6);
    let thumbnail = generate_thumbnail(&sideways, ThumbnailSize::Small).expect("a thumbnail");

    let rendered = image::load_from_memory(&thumbnail).expect("a readable thumbnail");
    assert!(
        rendered.height() > rendered.width(),
        "expected a portrait thumbnail, got {}x{}",
        rendered.width(),
        rendered.height()
    );
}

#[test]
fn a_photo_held_normally_is_left_alone() {
    use homecloud_media::{generate_thumbnail, ThumbnailSize};

    let upright = jpeg_with_pixels_and_exif(400, 200, 1);
    let thumbnail = generate_thumbnail(&upright, ThumbnailSize::Small).expect("a thumbnail");

    let rendered = image::load_from_memory(&thumbnail).expect("a readable thumbnail");
    assert!(
        rendered.width() > rendered.height(),
        "expected a landscape thumbnail, got {}x{}",
        rendered.width(),
        rendered.height()
    );
}
