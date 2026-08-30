//! What a photo says about itself.
//!
//! A file's modification time is not when the picture was taken: copy a
//! folder of holiday photos onto a new disk and every one of them claims
//! to be from today. The camera wrote the real date into the file, so
//! the timeline should use that.
//!
//! As everywhere else in this crate the input is treated as hostile.
//! Only the header is read, the parser is pure Rust, and a file that is
//! damaged or says something absurd produces "no metadata" rather than
//! an error worth surfacing — an unreadable EXIF block is a completely
//! normal thing to find in a real library.

use std::io::Cursor;

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

/// How much of a file is worth reading to find its metadata block.
///
/// EXIF lives near the start. A camera with a large embedded preview can
/// push it further in, but not past this, and the cost of being wrong is
/// one photo dated by its file time.
pub const MAX_HEADER_BYTES: usize = 4 * 1024 * 1024;

/// Longest camera description kept, so a crafted file cannot store an
/// essay in the catalog.
const MAX_CAMERA_LENGTH: usize = 96;

/// The earliest date treated as real. Digital cameras did not exist, so
/// anything before this is a clock that was never set.
const EARLIEST_YEAR: i32 = 1900;

/// What was worth keeping from a photo's header.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PhotoMetadata {
    /// When the picture was taken, as the camera recorded it.
    pub taken_at: Option<OffsetDateTime>,
    /// Make and model, as one line — "Fujifilm X100V".
    pub camera: Option<String>,
    /// How the camera was held, as an EXIF orientation value (1–8).
    pub orientation: Option<u16>,
}

impl PhotoMetadata {
    /// Whether anything at all was found. Used to tell "no metadata in
    /// this file" from "not looked at yet".
    pub fn is_empty(&self) -> bool {
        self.taken_at.is_none() && self.camera.is_none() && self.orientation.is_none()
    }
}

/// Reads the metadata block at the start of a file.
///
/// Never fails: every way this can go wrong — not an image, no EXIF, a
/// truncated header, a nonsense date — means the same thing to the
/// caller, which is that this photo has nothing to say about itself.
pub fn read(source: &[u8]) -> PhotoMetadata {
    let header = &source[..source.len().min(MAX_HEADER_BYTES)];
    let mut cursor = Cursor::new(header);

    let Ok(exif) = exif::Reader::new().read_from_container(&mut cursor) else {
        return PhotoMetadata::default();
    };

    PhotoMetadata {
        taken_at: taken_at(&exif),
        camera: camera(&exif),
        orientation: orientation(&exif),
    }
}

/// The capture time, preferring the moment the shutter opened over the
/// moment the file was written.
fn taken_at(exif: &exif::Exif) -> Option<OffsetDateTime> {
    // In order of trustworthiness: when the picture was taken, when the
    // camera digitised it, when the file was last changed.
    const FIELDS: [exif::Tag; 3] = [
        exif::Tag::DateTimeOriginal,
        exif::Tag::DateTimeDigitized,
        exif::Tag::DateTime,
    ];

    for tag in FIELDS {
        let field = exif.get_field(tag, exif::In::PRIMARY);
        let Some(field) = field else {
            continue;
        };

        let exif::Value::Ascii(ref values) = field.value else {
            continue;
        };

        let Some(raw) = values.first() else {
            continue;
        };

        if let Some(taken) = parse_datetime(raw) {
            return Some(taken);
        }
    }

    None
}

/// Parses `YYYY:MM:DD HH:MM:SS`, the only form EXIF defines.
///
/// The value carries no time zone, so it is read as UTC: a photo's date
/// is what the camera's clock said, and inventing an offset would be
/// less true, not more.
fn parse_datetime(raw: &[u8]) -> Option<OffsetDateTime> {
    let parsed = exif::DateTime::from_ascii(raw).ok()?;

    if i32::from(parsed.year) < EARLIEST_YEAR {
        return None;
    }

    let month = Month::try_from(parsed.month).ok()?;
    let date = Date::from_calendar_date(i32::from(parsed.year), month, parsed.day).ok()?;
    // Leap seconds are reported as :60, which `Time` refuses; clamping
    // costs a second and keeps the photo in the right minute.
    let time = Time::from_hms(parsed.hour, parsed.minute, parsed.second.min(59)).ok()?;

    Some(PrimitiveDateTime::new(date, time).assume_utc())
}

/// "Make Model", with the make dropped when the model already repeats
/// it — cameras write "NIKON CORPORATION" and "NIKON D750".
fn camera(exif: &exif::Exif) -> Option<String> {
    let make = ascii_field(exif, exif::Tag::Make);
    let model = ascii_field(exif, exif::Tag::Model);

    let camera = match (make, model) {
        (Some(make), Some(model)) => {
            let first = make.split_whitespace().next().unwrap_or_default();

            if !first.is_empty() && model.to_lowercase().contains(&first.to_lowercase()) {
                model
            } else {
                format!("{make} {model}")
            }
        }
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return None,
    };

    let camera: String = camera.chars().take(MAX_CAMERA_LENGTH).collect();
    let camera = camera.trim();

    (!camera.is_empty()).then(|| camera.to_owned())
}

/// A printable, trimmed ASCII field, or nothing.
///
/// Control characters are dropped rather than escaped: this string is
/// shown to a person and stored in a database, and a camera name has no
/// business containing them.
fn ascii_field(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    let exif::Value::Ascii(ref values) = field.value else {
        return None;
    };

    let raw = values.first()?;
    let text: String = raw
        .iter()
        .copied()
        .filter(|byte| byte.is_ascii_graphic() || *byte == b' ')
        .map(char::from)
        .collect();
    let text = text.trim();

    (!text.is_empty()).then(|| text.to_owned())
}

/// The EXIF orientation, if it is one of the eight defined values.
fn orientation(exif: &exif::Exif) -> Option<u16> {
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let exif::Value::Short(ref values) = field.value else {
        return None;
    };
    let value = *values.first()?;

    (1..=8).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_no_metadata_says_nothing() {
        assert!(read(b"not an image at all").is_empty());
        assert!(read(&[]).is_empty());
    }

    #[test]
    fn a_truncated_header_is_not_an_error() {
        // The start of a JPEG and then nothing: common in a real
        // library, and must not be treated as a failure.
        assert!(read(&[0xff, 0xd8, 0xff, 0xe1, 0x00]).is_empty());
    }

    #[test]
    fn a_camera_that_repeats_its_own_make_is_not_said_twice() {
        assert_eq!(
            join_camera("NIKON CORPORATION", "NIKON D750"),
            "NIKON D750".to_owned()
        );
        assert_eq!(
            join_camera("Fujifilm", "X100V"),
            "Fujifilm X100V".to_owned()
        );
    }

    #[test]
    fn a_date_before_photography_is_a_clock_that_was_never_set() {
        assert!(parse_datetime(b"1899:12:31 23:59:59").is_none());
        assert!(parse_datetime(b"2019:07:04 12:30:00").is_some());
    }

    #[test]
    fn a_leap_second_keeps_the_photo_in_the_right_minute() {
        let taken = parse_datetime(b"2016:12:31 23:59:60").expect("a time");

        assert_eq!(taken.second(), 59);
        assert_eq!(taken.minute(), 59);
    }

    #[test]
    fn nonsense_dates_are_refused_rather_than_wrapped() {
        assert!(parse_datetime(b"2019:13:01 00:00:00").is_none());
        assert!(parse_datetime(b"2019:02:30 00:00:00").is_none());
        assert!(parse_datetime(b"not a date at all").is_none());
    }

    /// The make/model rule on its own, without building an EXIF block.
    fn join_camera(make: &str, model: &str) -> String {
        let first = make.split_whitespace().next().unwrap_or_default();

        if !first.is_empty() && model.to_lowercase().contains(&first.to_lowercase()) {
            model.to_owned()
        } else {
            format!("{make} {model}")
        }
    }
}
