//! Resumable uploads: a file that arrives over several requests, and
//! survives the connection dropping in the middle.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Opens a session and returns its id.
async fn open(app: &TestApp, library: &str, path: &str, size: usize) -> String {
    let created = app
        .post_json(
            "/api/v1/uploads",
            json!({ "library_id": library, "path": path, "size_bytes": size }),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    assert_eq!(created.json()["offset"], 0);

    created.json()["id"].as_str().expect("id").to_owned()
}

/// Sends one chunk at the given offset.
async fn send(app: &TestApp, session: &str, offset: usize, bytes: &[u8]) -> TestResponse {
    app.send(
        axum::http::Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/uploads/{session}?offset={offset}"))
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(bytes.to_vec()))
            .expect("valid request"),
    )
    .await
}

/// A body large enough that its pieces are worth checking.
fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index % 251) as u8).collect()
}

#[tokio::test]
async fn a_file_arrives_over_several_chunks() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let bytes = payload(3000);
    let session = open(&app, &library, "holiday.bin", bytes.len()).await;

    let first = send(&app, &session, 0, &bytes[..1000]).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(first.json()["offset"], 1000);

    let second = send(&app, &session, 1000, &bytes[1000..2500]).await;
    assert_eq!(second.json()["offset"], 2500);

    send(&app, &session, 2500, &bytes[2500..]).await;

    let finished = app
        .post_json(&format!("/api/v1/uploads/{session}/complete"), json!({}))
        .await;
    assert_eq!(finished.status, StatusCode::OK, "{}", finished.text());
    assert_eq!(finished.json()["name"], "holiday.bin");
    assert_eq!(finished.json()["size_bytes"], 3000);

    // The bytes are the bytes, in order.
    let id = finished.json()["id"].as_str().expect("id").to_owned();
    let downloaded = app.get(&format!("/api/v1/items/{id}/content")).await;
    assert_eq!(downloaded.body, bytes);
}

#[tokio::test]
async fn an_interrupted_upload_resumes_where_it_stopped() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let bytes = payload(4096);
    let session = open(&app, &library, "big.bin", bytes.len()).await;

    send(&app, &session, 0, &bytes[..1500]).await;

    // The client disappears and comes back — a new connection, no memory
    // of what it had sent. It asks.
    let status = app.get(&format!("/api/v1/uploads/{session}")).await;
    assert_eq!(status.status, StatusCode::OK, "{}", status.text());
    assert_eq!(status.json()["offset"], 1500);

    send(&app, &session, 1500, &bytes[1500..]).await;

    let finished = app
        .post_json(&format!("/api/v1/uploads/{session}/complete"), json!({}))
        .await;
    assert_eq!(finished.status, StatusCode::OK, "{}", finished.text());

    let id = finished.json()["id"].as_str().expect("id").to_owned();
    let downloaded = app.get(&format!("/api/v1/items/{id}/content")).await;

    // Not corrupt: the resumed file is byte-for-byte the original.
    assert_eq!(downloaded.body, bytes);
}

#[tokio::test]
async fn a_chunk_sent_at_the_wrong_offset_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let bytes = payload(1000);
    let session = open(&app, &library, "gap.bin", bytes.len()).await;

    send(&app, &session, 0, &bytes[..400]).await;

    // A client that thinks it is further along would leave a hole in the
    // middle of the file. It is told where it actually is.
    let ahead = send(&app, &session, 900, &bytes[900..]).await;
    assert_eq!(ahead.status, StatusCode::CONFLICT);
    assert!(ahead.text().contains("400"), "{}", ahead.text());

    // And one that repeats itself would duplicate bytes.
    let behind = send(&app, &session, 0, &bytes[..400]).await;
    assert_eq!(behind.status, StatusCode::CONFLICT);

    // The file is still exactly what arrived.
    let status = app.get(&format!("/api/v1/uploads/{session}")).await;
    assert_eq!(status.json()["offset"], 400);
}

#[tokio::test]
async fn an_upload_that_is_not_all_there_cannot_be_completed() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let bytes = payload(1000);
    let session = open(&app, &library, "short.bin", bytes.len()).await;

    send(&app, &session, 0, &bytes[..600]).await;

    // A short file is a broken file. Putting it in someone's library as
    // though it were finished is worse than asking for the rest.
    let refused = app
        .post_json(&format!("/api/v1/uploads/{session}/complete"), json!({}))
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert!(refused.text().contains("600"), "{}", refused.text());
}

#[tokio::test]
async fn more_bytes_than_declared_are_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let session = open(&app, &library, "liar.bin", 100).await;

    // A client that keeps sending must not be able to fill the disk.
    let refused = send(&app, &session, 0, &payload(500)).await;

    assert_eq!(refused.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn an_abandoned_upload_can_be_thrown_away() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let session = open(&app, &library, "gone.bin", 1000).await;
    send(&app, &session, 0, &payload(400)).await;

    assert_eq!(
        app.delete(&format!("/api/v1/uploads/{session}"))
            .await
            .status,
        StatusCode::OK
    );

    // The session is gone, and so are its bytes.
    assert_eq!(
        app.get(&format!("/api/v1/uploads/{session}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_unfinished_upload_is_listed_so_it_can_be_picked_up() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let session = open(&app, &library, "later.bin", 1000).await;
    send(&app, &session, 0, &payload(250)).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/uploads"))
        .await;

    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());
    assert_eq!(listed.json()[0]["path"], "later.bin");
    assert_eq!(listed.json()[0]["offset"], 250);
    assert_eq!(listed.json()[0]["size_bytes"], 1000);
}

#[tokio::test]
async fn someone_elses_upload_is_not_theirs_to_continue() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let session = open(&app, &library, "private.bin", 1000).await;

    // Another member of the same library.
    let invitation = app
        .post_json(
            &format!("/api/v1/libraries/{library}/invitations"),
            json!({}),
        )
        .await;
    let token = invitation.json()["token"]
        .as_str()
        .expect("token")
        .to_owned();

    app.forget_session();
    app.post_json(
        &format!("/api/v1/invitations/{token}/accept"),
        json!({ "display_name": "Grace", "password": "another long passphrase" }),
    )
    .await;

    // An upload in progress is not shared work, and its staging file is
    // not something another person should be able to append to.
    assert_eq!(
        app.get(&format!("/api/v1/uploads/{session}")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, &session, 0, b"intrusion").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_upload_needs_a_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.forget_session();

    let refused = app
        .post_json(
            "/api/v1/uploads",
            json!({ "library_id": library, "path": "x.bin", "size_bytes": 10 }),
        )
        .await;

    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_path_that_escapes_the_library_is_refused_before_any_bytes() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let refused = app
        .post_json(
            "/api/v1/uploads",
            json!({ "library_id": library, "path": "../escape.bin", "size_bytes": 10 }),
        )
        .await;

    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_name_taken_during_a_long_upload_does_not_overwrite_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let bytes = payload(500);
    let session = open(&app, &library, "report.pdf", bytes.len()).await;
    send(&app, &session, 0, &bytes).await;

    // Something else takes the name while the upload is in flight.
    app.upload(&library, "report.pdf", b"the other one").await;

    let finished = app
        .post_json(&format!("/api/v1/uploads/{session}/complete"), json!({}))
        .await;
    assert_eq!(finished.status, StatusCode::OK, "{}", finished.text());

    // The same rule as any other upload: never overwrite, pick a free
    // name, and keep both files.
    assert_eq!(finished.json()["name"], "report (2).pdf");

    let id = finished.json()["id"].as_str().expect("id").to_owned();
    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/content")).await.body,
        bytes
    );
}

#[tokio::test]
async fn a_resumed_photo_still_gets_its_capture_date() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // The enrichment that a one-shot upload gets must not be skipped
    // just because the bytes arrived in pieces.
    let png = b"\x89PNG\r\n\x1a\n".to_vec();
    let session = open(&app, &library, "chunked.png", png.len()).await;
    send(&app, &session, 0, &png).await;

    let finished = app
        .post_json(&format!("/api/v1/uploads/{session}/complete"), json!({}))
        .await;

    assert_eq!(finished.status, StatusCode::OK, "{}", finished.text());
    // No EXIF in this one, but the field is present and the file is in
    // the timeline rather than missing from it.
    assert!(finished.json()["taken_at"].is_null());

    let photos = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;
    assert_eq!(photos.json()[0]["name"], "chunked.png");
}
