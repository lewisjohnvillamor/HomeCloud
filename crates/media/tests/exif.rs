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

/// Builds a JPEG whose header records where it was taken.
fn jpeg_with_location(
    latitude: [(u32, u32); 3],
    latitude_ref: &str,
    longitude: [(u32, u32); 3],
    longitude_ref: &str,
) -> Vec<u8> {
    use std::io::Write;

    // GPS lives in its own IFD, pointed at from IFD0.
    let mut gps_heap: Vec<u8> = Vec::new();
    let gps_entries = 4u16;
    let gps_size = 2 + gps_entries as usize * 12 + 4;

    const HEADER: usize = 8;
    let ifd0_size = 2 + 12 + 4;
    let gps_at = HEADER + ifd0_size;
    let gps_heap_at = gps_at + gps_size;

    let mut gps = Vec::new();
    gps.write_all(&gps_entries.to_le_bytes()).unwrap();

    // GPSLatitudeRef / GPSLongitudeRef are one-character ASCII.
    let ascii_ref = |tag: u16, text: &str, gps: &mut Vec<u8>| {
        let value = format!("{text}\0");
        gps.write_all(&tag.to_le_bytes()).unwrap();
        gps.write_all(&2u16.to_le_bytes()).unwrap();
        gps.write_all(&(value.len() as u32).to_le_bytes()).unwrap();
        let mut padded = value.into_bytes();
        padded.resize(4, 0);
        gps.write_all(&padded).unwrap();
    };

    let rational = |tag: u16, parts: [(u32, u32); 3], gps: &mut Vec<u8>, heap: &mut Vec<u8>| {
        gps.write_all(&tag.to_le_bytes()).unwrap();
        gps.write_all(&5u16.to_le_bytes()).unwrap();
        gps.write_all(&3u32.to_le_bytes()).unwrap();
        gps.write_all(&((gps_heap_at + heap.len()) as u32).to_le_bytes())
            .unwrap();
        for (numerator, denominator) in parts {
            heap.write_all(&numerator.to_le_bytes()).unwrap();
            heap.write_all(&denominator.to_le_bytes()).unwrap();
        }
    };

    ascii_ref(0x0001, latitude_ref, &mut gps);
    rational(0x0002, latitude, &mut gps, &mut gps_heap);
    ascii_ref(0x0003, longitude_ref, &mut gps);
    rational(0x0004, longitude, &mut gps, &mut gps_heap);
    gps.write_all(&0u32.to_le_bytes()).unwrap();

    let mut ifd0 = Vec::new();
    ifd0.write_all(&1u16.to_le_bytes()).unwrap();
    // GPSInfoIFDPointer.
    ifd0.write_all(&0x8825u16.to_le_bytes()).unwrap();
    ifd0.write_all(&4u16.to_le_bytes()).unwrap();
    ifd0.write_all(&1u32.to_le_bytes()).unwrap();
    ifd0.write_all(&(gps_at as u32).to_le_bytes()).unwrap();
    ifd0.write_all(&0u32.to_le_bytes()).unwrap();

    let mut tiff = Vec::new();
    tiff.write_all(b"II").unwrap();
    tiff.write_all(&42u16.to_le_bytes()).unwrap();
    tiff.write_all(&(HEADER as u32).to_le_bytes()).unwrap();
    tiff.write_all(&ifd0).unwrap();
    tiff.write_all(&gps).unwrap();
    tiff.write_all(&gps_heap).unwrap();

    let mut app1 = Vec::new();
    app1.write_all(b"Exif\0\0").unwrap();
    app1.write_all(&tiff).unwrap();

    let mut jpeg = Vec::new();
    jpeg.write_all(&[0xFF, 0xD8, 0xFF, 0xE1]).unwrap();
    jpeg.write_all(&((app1.len() + 2) as u16).to_be_bytes())
        .unwrap();
    jpeg.write_all(&app1).unwrap();
    jpeg.write_all(&[0xFF, 0xD9]).unwrap();

    jpeg
}

#[test]
fn a_photo_says_where_it_was_taken() {
    // 51°28'40.8"N, 0°0'2.4"W — Greenwich.
    let photo = jpeg_with_location(
        [(51, 1), (28, 1), (408, 10)],
        "N",
        [(0, 1), (0, 1), (24, 10)],
        "W",
    );

    let metadata = homecloud_media::exif::read(&photo);

    let latitude = metadata.latitude.expect("a latitude");
    let longitude = metadata.longitude.expect("a longitude");
    assert!((latitude - 51.478).abs() < 0.001, "{latitude}");
    assert!((longitude - -0.000_666).abs() < 0.001, "{longitude}");
}

#[test]
fn a_southern_and_western_photo_is_negative() {
    // 33°51'S, 151°12'E — Sydney.
    let photo = jpeg_with_location(
        [(33, 1), (51, 1), (0, 1)],
        "S",
        [(151, 1), (12, 1), (0, 1)],
        "E",
    );

    let metadata = homecloud_media::exif::read(&photo);

    assert!(metadata.latitude.expect("a latitude") < 0.0);
    assert!(metadata.longitude.expect("a longitude") > 0.0);
}

#[test]
fn a_camera_with_no_fix_is_not_placed_at_null_island() {
    // Zeroes are what a camera writes when it never got a fix. Trusting
    // them puts every such photo in the Gulf of Guinea.
    let photo = jpeg_with_location([(0, 1), (0, 1), (0, 1)], "N", [(0, 1), (0, 1), (0, 1)], "E");

    let metadata = homecloud_media::exif::read(&photo);

    assert_eq!(metadata.latitude, None);
    assert_eq!(metadata.longitude, None);
}

#[test]
fn a_photo_with_only_half_a_coordinate_has_no_location() {
    // Half a coordinate is not a place.
    let mut photo = jpeg_with_location(
        [(51, 1), (28, 1), (0, 1)],
        "N",
        [(0, 1), (0, 1), (0, 1)],
        "W",
    );
    // Break the longitude tag so only latitude survives.
    if let Some(index) = photo
        .windows(2)
        .position(|pair| pair == 0x0004u16.to_le_bytes())
    {
        photo[index] = 0xEE;
        photo[index + 1] = 0xEE;
    }

    let metadata = homecloud_media::exif::read(&photo);

    assert_eq!(metadata.longitude, None);
    assert_eq!(metadata.latitude, None);
}
