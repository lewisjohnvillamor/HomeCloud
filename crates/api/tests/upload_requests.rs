//! Upload request links: writing into one folder, without a session and
//! without seeing what is already there.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Makes a folder and returns its id.
async fn folder(app: &TestApp, library: &str, name: &str) -> String {
    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({ "path": name }),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());

    created.json()["id"].as_str().expect("id").to_owned()
}

/// Creates a link and returns its token.
async fn link(app: &TestApp, item: &str, body: serde_json::Value) -> String {
    let created = app
        .post_json(&format!("/api/v1/items/{item}/upload-requests"), body)
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());

    created.json()["token"].as_str().expect("token").to_owned()
}

/// Sends a file through a link, as someone with no account.
async fn send(app: &TestApp, token: &str, name: &str, bytes: &[u8]) -> TestResponse {
    let encoded = name.replace(' ', "%20").replace('/', "%2F");

    app.send(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v1/public/upload-requests/{token}/files?name={encoded}"
            ))
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(bytes.to_vec()))
            .expect("valid request"),
    )
    .await
}

#[tokio::test]
async fn someone_without_an_account_can_send_a_file_to_one_folder() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "Wedding photos").await;
    let token = link(&app, &inbox, json!({})).await;

    app.forget_session();

    // What the link is for, without a word about what is in the folder.
    let opened = app
        .get(&format!("/api/v1/public/upload-requests/{token}"))
        .await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.text());
    assert_eq!(opened.json()["folder_name"], "Wedding photos");
    assert_eq!(opened.json()["title"], "Send files to Wedding photos");
    assert!(opened.json().get("items").is_none());

    let sent = send(&app, &token, "confetti.jpg", b"jpeg bytes").await;
    assert_eq!(sent.status, StatusCode::OK, "{}", sent.text());
    assert_eq!(sent.json()["name"], "confetti.jpg");
    assert_eq!(sent.json()["path"], "Wedding photos/confetti.jpg");
}

#[tokio::test]
async fn a_link_cannot_be_used_to_read_anything() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "private.txt", b"secret").await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({})).await;

    app.forget_session();

    // The link is not a share: nothing about it opens a reading route.
    assert_eq!(
        app.get(&format!("/api/v1/public/{token}")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/browse"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_name_from_a_stranger_is_a_name_and_never_a_path() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({})).await;

    app.forget_session();

    // Whatever the sender calls it, the file lands in the link's folder.
    let escape = send(&app, &token, "../../etc/passwd", b"root:x:0:0").await;
    assert_eq!(escape.status, StatusCode::OK, "{}", escape.text());
    assert_eq!(escape.json()["path"], "inbox/passwd");

    let windows = send(&app, &token, "C:\\Users\\me\\photo.jpg", b"bytes").await;
    assert_eq!(windows.json()["path"], "inbox/photo.jpg");
}

#[tokio::test]
async fn two_files_of_the_same_name_are_both_kept() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({})).await;

    app.forget_session();

    send(&app, &token, "photo.jpg", b"first").await;
    let second = send(&app, &token, "photo.jpg", b"second").await;

    // The same never-overwrite rule as any other upload: two people
    // sending "IMG_0001.jpg" must not lose one of them.
    assert_eq!(second.json()["name"], "photo (2).jpg");
}

#[tokio::test]
async fn a_link_stops_at_the_number_of_files_it_was_given() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({ "max_files": 2 })).await;

    app.forget_session();

    assert_eq!(
        send(&app, &token, "one.txt", b"a").await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, &token, "two.txt", b"b").await.status,
        StatusCode::OK
    );

    let refused = send(&app, &token, "three.txt", b"c").await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_link_stops_at_the_size_it_was_given() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({ "max_bytes": 10 })).await;

    app.forget_session();

    // A stranger with a link must not be able to fill the disk.
    let refused = send(&app, &token, "big.bin", &vec![0u8; 500]).await;
    assert!(
        refused.status.is_client_error(),
        "{} {}",
        refused.status,
        refused.text()
    );

    // And nothing was left behind in the folder.
    let listed = app
        .get(&format!("/api/v1/public/upload-requests/{token}"))
        .await;
    assert_eq!(listed.json()["files_left"], 50);
}

#[tokio::test]
async fn a_revoked_link_stops_accepting_files() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({})).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/upload-requests"))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());
    let id = listed.json()[0]["id"].as_str().expect("id").to_owned();

    assert_eq!(
        app.delete(&format!("/api/v1/upload-requests/{id}"))
            .await
            .status,
        StatusCode::OK
    );

    app.forget_session();

    // Revoked and unknown look the same: a visitor must not learn that a
    // link once existed.
    assert_eq!(
        app.get(&format!("/api/v1/public/upload-requests/{token}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, &token, "late.txt", b"too late").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_unknown_link_says_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    assert_eq!(
        app.get("/api/v1/public/upload-requests/not-a-real-token")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_link_points_at_a_folder_not_a_file() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let file = app.upload(&library, "notes.txt", b"hello").await;
    let id = file.json()["id"].as_str().expect("id").to_owned();

    let refused = app
        .post_json(&format!("/api/v1/items/{id}/upload-requests"), json!({}))
        .await;

    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn only_a_member_can_make_or_revoke_a_link() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    link(&app, &inbox, json!({})).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/upload-requests"))
        .await;
    let id = listed.json()[0]["id"].as_str().expect("id").to_owned();

    app.forget_session();

    assert_eq!(
        app.post_json(&format!("/api/v1/items/{inbox}/upload-requests"), json!({}))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.delete(&format!("/api/v1/upload-requests/{id}"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/upload-requests"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_absurd_limit_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;

    for body in [
        json!({ "max_files": 0 }),
        json!({ "max_files": 100_000 }),
        json!({ "max_bytes": 0 }),
        json!({ "expires_in_days": 0 }),
        json!({ "expires_in_days": 4000 }),
    ] {
        let refused = app
            .post_json(
                &format!("/api/v1/items/{inbox}/upload-requests"),
                body.clone(),
            )
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{body}");
    }
}

#[tokio::test]
async fn a_file_that_arrives_is_catalogued_like_any_other() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let inbox = folder(&app, &library, "inbox").await;
    let token = link(&app, &inbox, json!({})).await;

    app.forget_session();
    let sent = send(&app, &token, "gift.txt", b"a present").await;
    let id = sent.json()["id"].as_str().expect("id").to_owned();

    // The owner sees it as an ordinary file: downloadable, and counted
    // against the link that let it in.
    app.post_json(
        "/api/v1/auth/login",
        json!({ "display_name": "Ada", "password": "correct horse battery staple" }),
    )
    .await;

    let downloaded = app.get(&format!("/api/v1/items/{id}/content")).await;
    assert_eq!(downloaded.status, StatusCode::OK);
    assert_eq!(downloaded.body, b"a present");

    let counted = app
        .get(&format!("/api/v1/libraries/{library}/upload-requests"))
        .await;
    assert_eq!(counted.json()[0]["received_files"], 1);
    assert_eq!(counted.json()[0]["received_bytes"], 9);
}
