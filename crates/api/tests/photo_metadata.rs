//! What photos say about themselves, and what the timeline does with it.

mod support;

use axum::http::StatusCode;
use support::TestApp;

/// A JPEG carrying a real EXIF header with the given capture time.
///
/// Assembled here rather than committed as a fixture so what is being
/// parsed is visible in the test.
fn photo_taken_at(taken: &str, make: &str, model: &str) -> Vec<u8> {
    const ASCII: u16 = 2;
    const LONG: u16 = 4;
    const HEADER: usize = 8;

    fn ascii_entry(tag: u16, text: &str, heap_at: usize, heap: &mut Vec<u8>) -> Vec<u8> {
        let mut value = text.as_bytes().to_vec();
        value.push(0);

        let mut entry = Vec::new();
        entry.extend_from_slice(&tag.to_le_bytes());
        entry.extend_from_slice(&ASCII.to_le_bytes());
        entry.extend_from_slice(&(value.len() as u32).to_le_bytes());
        entry.extend_from_slice(&((heap_at + heap.len()) as u32).to_le_bytes());
        heap.extend_from_slice(&value);

        entry
    }

    let ifd0_size = 2 + 3 * 12 + 4;
    let ifd0_at = HEADER;
    let ifd0_heap_at = ifd0_at + ifd0_size;
    let ifd0_heap_len = make.len() + 1 + model.len() + 1;
    let exif_at = ifd0_heap_at + ifd0_heap_len;
    let exif_size = 2 + 12 + 4;
    let exif_heap_at = exif_at + exif_size;

    let mut ifd0_heap = Vec::new();
    let mut ifd0 = Vec::new();
    ifd0.extend_from_slice(&3u16.to_le_bytes());
    ifd0.extend_from_slice(&ascii_entry(0x010f, make, ifd0_heap_at, &mut ifd0_heap));
    ifd0.extend_from_slice(&ascii_entry(0x0110, model, ifd0_heap_at, &mut ifd0_heap));
    // Pointer to the Exif sub-directory, which holds the capture time.
    ifd0.extend_from_slice(&0x8769u16.to_le_bytes());
    ifd0.extend_from_slice(&LONG.to_le_bytes());
    ifd0.extend_from_slice(&1u32.to_le_bytes());
    ifd0.extend_from_slice(&(exif_at as u32).to_le_bytes());
    ifd0.extend_from_slice(&0u32.to_le_bytes());

    let mut exif_heap = Vec::new();
    let mut exif_ifd = Vec::new();
    exif_ifd.extend_from_slice(&1u16.to_le_bytes());
    exif_ifd.extend_from_slice(&ascii_entry(0x9003, taken, exif_heap_at, &mut exif_heap));
    exif_ifd.extend_from_slice(&0u32.to_le_bytes());

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

    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    jpeg.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(&app1);
    jpeg.extend_from_slice(&[0xff, 0xd9]);

    jpeg
}

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

#[tokio::test]
async fn an_uploaded_photo_carries_the_date_the_camera_recorded() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let uploaded = app
        .upload(
            &library,
            "wedding.jpg",
            &photo_taken_at("2019:07:04 12:30:45", "Fujifilm", "X100V"),
        )
        .await;

    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.text());

    // Immediately, not a scan later: a photo sent from a phone should
    // land in the right month straight away.
    let taken = uploaded.json()["taken_at"]
        .as_str()
        .expect("a capture date")
        .to_owned();
    assert!(taken.starts_with("2019-07-04T12:30:45"), "{taken}");
    assert_eq!(uploaded.json()["camera"], "Fujifilm X100V");
}

#[tokio::test]
async fn a_photo_found_by_a_scan_is_described_too() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.put_on_disk(
        "holiday.jpg",
        &photo_taken_at("2015:08:21 09:15:00", "Canon", "EOS R6"),
    );
    app.scan(&library).await;

    let photos = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;
    let first = &photos.json()[0];

    assert_eq!(first["name"], "holiday.jpg");
    assert!(
        first["taken_at"]
            .as_str()
            .expect("a capture date")
            .starts_with("2015-08-21T09:15:00"),
        "{first}"
    );
    assert_eq!(first["camera"], "Canon EOS R6");
}

#[tokio::test]
async fn the_timeline_is_ordered_by_when_pictures_were_taken() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // Uploaded in one order, taken in another. Every file's modification
    // time is now, which is exactly the case that makes file times
    // useless for a timeline.
    app.upload(
        &library,
        "older.jpg",
        &photo_taken_at("2015:01:02 08:00:00", "Canon", "EOS R6"),
    )
    .await;
    app.upload(
        &library,
        "newest.jpg",
        &photo_taken_at("2024:12:25 18:00:00", "Canon", "EOS R6"),
    )
    .await;
    app.upload(
        &library,
        "middle.jpg",
        &photo_taken_at("2019:07:04 12:30:45", "Canon", "EOS R6"),
    )
    .await;

    let photos = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;
    let names: Vec<String> = photos
        .json()
        .as_array()
        .expect("photos")
        .iter()
        .map(|photo| photo["name"].as_str().unwrap_or_default().to_owned())
        .collect();

    assert_eq!(names, vec!["newest.jpg", "middle.jpg", "older.jpg"]);
}

#[tokio::test]
async fn a_picture_that_says_nothing_still_appears_by_its_file_date() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // A PNG with no EXIF at all, which is most screenshots.
    let uploaded = app
        .upload(&library, "screenshot.png", b"\x89PNG\r\n\x1a\n")
        .await;

    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.text());
    assert!(uploaded.json()["taken_at"].is_null());
    assert!(uploaded.json()["camera"].is_null());

    let photos = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;
    assert_eq!(photos.json()[0]["name"], "screenshot.png");
}

#[tokio::test]
async fn a_file_that_only_claims_to_be_a_photo_is_not_a_failure() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // A shell script with a camera's extension. Reading its metadata
    // must be a non-event, not an error that fails an upload.
    let uploaded = app
        .upload(&library, "trojan.jpg", b"#!/bin/sh\nrm -rf /\n")
        .await;

    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.text());
    assert!(uploaded.json()["taken_at"].is_null());
}

#[tokio::test]
async fn memories_use_the_day_the_picture_was_taken() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // On this day, some years ago. The file itself is from today.
    let today = time::OffsetDateTime::now_utc();
    let taken = format!(
        "{}:{:02}:{:02} 14:00:00",
        today.year() - 3,
        u8::from(today.month()),
        today.day()
    );
    app.upload(
        &library,
        "anniversary.jpg",
        &photo_taken_at(&taken, "Canon", "EOS R6"),
    )
    .await;

    let memories = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;

    let groups = memories.json();
    let on_this_day = groups
        .as_array()
        .expect("groups")
        .iter()
        .find(|group| group["title"] == "On this day")
        .expect("an 'on this day' group");

    assert_eq!(on_this_day["items"][0]["name"], "anniversary.jpg");
}
