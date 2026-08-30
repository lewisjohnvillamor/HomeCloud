//! Sharing an album: a set someone arranged, not the folder its pictures
//! happen to sit in.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// An album with two pictures in it, and one file left outside.
async fn album_with_photos(app: &TestApp, library: &str) -> (String, String, String) {
    let inside = app
        .upload(library, "inside.png", b"\x89PNG\r\n\x1a\none")
        .await;
    let also = app
        .upload(library, "also.png", b"\x89PNG\r\n\x1a\ntwo")
        .await;
    let outside = app
        .upload(library, "private.png", b"\x89PNG\r\n\x1a\nno")
        .await;

    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/albums"),
            json!({ "name": "Wales, summer 2019" }),
        )
        .await;
    let album = created.json()["id"].as_str().expect("id").to_owned();

    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [
            inside.json()["id"].as_str().expect("id"),
            also.json()["id"].as_str().expect("id"),
        ] }),
    )
    .await;

    (
        album,
        inside.json()["id"].as_str().expect("id").to_owned(),
        outside.json()["id"].as_str().expect("id").to_owned(),
    )
}

#[tokio::test]
async fn an_album_can_be_shared_and_read_without_signing_in() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, _, _) = album_with_photos(&app, &library).await;

    let shared = app
        .post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
        .await;
    assert_eq!(shared.status, StatusCode::OK, "{}", shared.text());
    let token = shared.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();

    let public = app.get(&format!("/api/v1/public/{token}")).await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.text());
    assert_eq!(public.json()["album"]["name"], "Wales, summer 2019");
    assert_eq!(public.json()["album"]["item_count"], 2);
    assert_eq!(public.json()["items"].as_array().expect("items").len(), 2);
}

#[tokio::test]
async fn a_shared_album_reaches_its_own_pictures_and_nothing_else() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, inside, outside) = album_with_photos(&app, &library).await;

    let shared = app
        .post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
        .await;
    let token = shared.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();

    // A picture in the album downloads.
    let allowed = app
        .get(&format!("/api/v1/public/{token}/content?item={inside}"))
        .await;
    assert_eq!(allowed.status, StatusCode::OK, "{}", allowed.text());

    // One that merely sits in the same library does not. Membership is
    // the boundary, not the folder the pictures happen to be in.
    let refused = app
        .get(&format!("/api/v1/public/{token}/content?item={outside}"))
        .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{}", refused.text());
}

#[tokio::test]
async fn a_shared_album_never_reveals_where_its_pictures_live() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.post_json(
        &format!("/api/v1/libraries/{library}/folders"),
        json!({ "path": "Private/Family/2019" }),
    )
    .await;
    let photo = app
        .upload(
            &library,
            "Private%2FFamily%2F2019%2Fbeach.png",
            b"\x89PNG\r\n\x1a\n",
        )
        .await;

    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/albums"),
            json!({ "name": "Holiday" }),
        )
        .await;
    let album = created.json()["id"].as_str().expect("id").to_owned();
    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [photo.json()["id"].as_str().expect("id")] }),
    )
    .await;

    let shared = app
        .post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
        .await;
    let token = shared.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();
    let public = app.get(&format!("/api/v1/public/{token}")).await;

    // The visitor was given an album, not a tour of somebody's folders.
    assert!(
        !public.text().contains("Private"),
        "a shared album leaked its folder: {}",
        public.text()
    );
    assert_eq!(public.json()["items"][0]["path"], "beach.png");
}

#[tokio::test]
async fn a_revoked_album_link_stops_working() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, _, _) = album_with_photos(&app, &library).await;

    let shared = app
        .post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
        .await;
    let token = shared.json()["token"].as_str().expect("token").to_owned();
    let id = shared.json()["id"].as_str().expect("id").to_owned();

    app.delete(&format!("/api/v1/shares/{id}")).await;

    app.forget_session();
    assert_eq!(
        app.get(&format!("/api/v1/public/{token}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_album_link_can_carry_a_password() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, _, _) = album_with_photos(&app, &library).await;

    let shared = app
        .post_json(
            &format!("/api/v1/albums/{album}/shares"),
            // A phrase rather than a word-and-digits, which is both the
            // convention in these tests and the shape a secret scanner
            // correctly treats as suspicious.
            json!({ "password": "the blue front door" }),
        )
        .await;
    let token = shared.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();

    // Until the password is given the link discloses nothing — not even
    // the album's name.
    let locked = app.get(&format!("/api/v1/public/{token}")).await;
    assert_eq!(locked.status, StatusCode::UNAUTHORIZED, "{}", locked.text());
    assert!(!locked.text().contains("Wales"), "{}", locked.text());

    let unlocked = app
        .post_json(
            &format!("/api/v1/public/{token}/unlock"),
            // A phrase rather than a word-and-digits, which is both the
            // convention in these tests and the shape a secret scanner
            // correctly treats as suspicious.
            json!({ "password": "the blue front door" }),
        )
        .await;
    assert_eq!(unlocked.status, StatusCode::OK, "{}", unlocked.text());
    let key = unlocked.json()["key"].as_str().expect("key").to_owned();

    let opened = app.get(&format!("/api/v1/public/{token}?key={key}")).await;
    assert_eq!(opened.json()["album"]["name"], "Wales, summer 2019");
}

#[tokio::test]
async fn only_a_member_can_share_an_album() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, _, _) = album_with_photos(&app, &library).await;

    app.forget_session();

    // Not "forbidden": whether an album exists is something only the
    // library's own members should learn.
    assert_eq!(
        app.post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_picture_taken_out_of_a_shared_album_stops_being_reachable() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let (album, inside, _) = album_with_photos(&app, &library).await;

    let shared = app
        .post_json(&format!("/api/v1/albums/{album}/shares"), json!({}))
        .await;
    let token = shared.json()["token"].as_str().expect("token").to_owned();

    app.delete(&format!("/api/v1/albums/{album}/items/{inside}"))
        .await;

    app.forget_session();

    // The link follows the album, so removing a picture from it removes
    // it from everyone holding the link.
    assert_eq!(
        app.get(&format!("/api/v1/public/{token}/content?item={inside}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}
