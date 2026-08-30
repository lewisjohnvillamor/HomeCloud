//! Pairing a television, and the limits of what a paired one can read.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

/// A small but genuinely valid PNG, so the library has real pictures.
fn png_bytes() -> Vec<u8> {
    use std::io::Cursor;

    let mut buffer = image::RgbImage::new(64, 48);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]);
    }

    let mut output = Vec::new();
    image::DynamicImage::ImageRgb8(buffer)
        .write_to(&mut Cursor::new(&mut output), image::ImageFormat::Png)
        .expect("encode test image");

    output
}

async fn start_pairing(app: &TestApp) -> (String, String) {
    let response = app.post_json("/api/v1/tv/pairings", json!({})).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());

    (
        response.json()["code"].as_str().expect("code").to_owned(),
        response.json()["poll_token"]
            .as_str()
            .expect("poll token")
            .to_owned(),
    )
}

async fn approve(app: &TestApp, code: &str, library: &str) -> TestResponse {
    app.post_json(
        &format!("/api/v1/tv/pairings/{code}/approve"),
        json!({ "library_id": library, "name": "Living room" }),
    )
    .await
}

/// Pairs a screen and returns its device token.
async fn paired_screen(app: &TestApp, library: &str) -> String {
    let (code, poll) = start_pairing(app).await;
    let approved = approve(app, &code, library).await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.text());

    let collected = app.get(&format!("/api/v1/tv/pairings/{poll}")).await;
    assert_eq!(collected.status, StatusCode::OK, "{}", collected.text());

    collected.json()["token"]
        .as_str()
        .expect("a device token")
        .to_owned()
}

#[tokio::test]
async fn a_television_pairs_when_someone_signed_in_approves_the_code() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let (code, poll) = start_pairing(&app).await;

    // Before anyone approves it, the screen is told to keep waiting.
    let waiting = app.get(&format!("/api/v1/tv/pairings/{poll}")).await;
    assert_eq!(waiting.status, StatusCode::OK);
    assert_eq!(waiting.json()["status"], "pending");
    assert!(waiting.json()["token"].is_null());

    let approved = approve(&app, &code, &library).await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.text());
    assert_eq!(approved.json()["name"], "Living room");

    let collected = app.get(&format!("/api/v1/tv/pairings/{poll}")).await;
    assert_eq!(collected.json()["status"], "approved");
    assert_eq!(collected.json()["library_name"], "Home");
    assert!(collected.json()["token"].as_str().is_some());
}

#[tokio::test]
async fn a_code_is_read_the_way_it_is_typed() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let (code, _) = start_pairing(&app).await;

    // Lower case, and without the separator a person may not copy.
    let typed = code.to_lowercase().replace('-', "");
    assert_eq!(approve(&app, &typed, &library).await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_token_is_handed_over_only_once() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let (code, poll) = start_pairing(&app).await;
    approve(&app, &code, &library).await;

    assert_eq!(
        app.get(&format!("/api/v1/tv/pairings/{poll}")).await.status,
        StatusCode::OK
    );
    // A second collection — someone who copied the screen's secret —
    // gets nothing.
    assert_eq!(
        app.get(&format!("/api/v1/tv/pairings/{poll}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn knowing_the_code_is_not_enough_to_collect_the_token() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let (code, _poll) = start_pairing(&app).await;
    approve(&app, &code, &library).await;

    // Someone who photographed the television polls with what they can
    // see. The code is not the screen's secret.
    let attempt = app.get(&format!("/api/v1/tv/pairings/{code}")).await;
    assert_eq!(attempt.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn approving_needs_a_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let (code, _) = start_pairing(&app).await;

    app.forget_session();

    assert_eq!(
        approve(&app, &code, &library).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_unknown_code_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    assert_eq!(
        approve(&app, "ZZZZ-ZZZZ", &library).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_code_cannot_be_approved_twice() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let (code, _) = start_pairing(&app).await;
    assert_eq!(approve(&app, &code, &library).await.status, StatusCode::OK);
    assert_eq!(
        approve(&app, &code, &library).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_paired_screen_reads_the_memories_of_its_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    app.upload(&library, "beach.png", &png_bytes()).await;

    let token = paired_screen(&app, &library).await;

    // No session at all from here on: the token is the whole credential.
    app.forget_session();

    let memories = app.get(&format!("/api/v1/tv/memories?token={token}")).await;
    assert_eq!(memories.status, StatusCode::OK, "{}", memories.text());
    let names: Vec<String> = memories.json()[0]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(names.contains(&"beach.png".to_owned()), "{names:?}");
}

#[tokio::test]
async fn a_paired_screen_cannot_read_a_document() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let secret = app.upload(&library, "taxes.txt", b"income").await;
    let secret_id = secret.json()["id"].as_str().expect("id").to_owned();

    let token = paired_screen(&app, &library).await;
    app.forget_session();

    // A screen in a living room shows pictures. Anything else in the
    // same library is not there as far as it is concerned.
    let attempt = app
        .get(&format!(
            "/api/v1/tv/content?token={token}&item={secret_id}"
        ))
        .await;
    assert_eq!(attempt.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_paired_screen_shows_pictures() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let photo = app.upload(&library, "beach.png", &png_bytes()).await;
    let photo_id = photo.json()["id"].as_str().expect("id").to_owned();

    let token = paired_screen(&app, &library).await;
    app.forget_session();

    let full = app
        .get(&format!("/api/v1/tv/content?token={token}&item={photo_id}"))
        .await;
    assert_eq!(full.status, StatusCode::OK, "{}", full.text());

    let preview = app
        .get(&format!(
            "/api/v1/tv/thumbnail?token={token}&item={photo_id}"
        ))
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.text());
}

#[tokio::test]
async fn an_unpaired_token_reads_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    app.upload(&library, "beach.png", &png_bytes()).await;
    app.forget_session();

    let attempt = app.get("/api/v1/tv/memories?token=not-a-real-token").await;
    assert_eq!(attempt.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unpairing_takes_effect_on_the_next_request() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    app.upload(&library, "beach.png", &png_bytes()).await;

    let token = paired_screen(&app, &library).await;

    let listed = app.get(&format!("/api/v1/libraries/{library}/tv")).await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());
    let device = listed.json()[0]["id"].as_str().expect("id").to_owned();
    assert_eq!(listed.json()[0]["name"], "Living room");

    assert_eq!(
        app.get(&format!("/api/v1/tv/memories?token={token}"))
            .await
            .status,
        StatusCode::OK
    );

    assert_eq!(
        app.delete(&format!("/api/v1/tv/devices/{device}"))
            .await
            .status,
        StatusCode::OK
    );

    app.forget_session();
    assert_eq!(
        app.get(&format!("/api/v1/tv/memories?token={token}"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_screen_paired_with_one_library_cannot_see_another() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let token = paired_screen(&app, &library).await;

    // A member of another library uploads something. The paired screen
    // holds a capability for one library only, so an id from elsewhere
    // is a "not found" rather than a picture.
    let elsewhere = uuid::Uuid::new_v4();
    app.forget_session();

    let attempt = app
        .get(&format!(
            "/api/v1/tv/content?token={token}&item={elsewhere}"
        ))
        .await;
    assert_eq!(attempt.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_non_member_cannot_list_or_unpair_screens() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let token = paired_screen(&app, &library).await;
    let listed = app.get(&format!("/api/v1/libraries/{library}/tv")).await;
    let device = listed.json()[0]["id"].as_str().expect("id").to_owned();

    app.forget_session();

    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/tv"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.delete(&format!("/api/v1/tv/devices/{device}"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    // And the screen still works, because nothing was revoked.
    assert_eq!(
        app.get(&format!("/api/v1/tv/memories?token={token}"))
            .await
            .status,
        StatusCode::OK
    );
}
