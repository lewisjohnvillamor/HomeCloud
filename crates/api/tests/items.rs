//! Copying a file: the one file operation that leaves the original
//! where it is.

mod support;

use axum::http::StatusCode;
use support::TestApp;

#[tokio::test]
async fn a_file_can_be_copied_and_both_copies_stay() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let uploaded = app.upload(&library, "report.pdf", b"the contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let copied = app
        .post_json(
            &format!("/api/v1/items/{id}/copy"),
            serde_json::json!({ "path": "archive/report.pdf" }),
        )
        .await;
    assert_eq!(copied.status, StatusCode::OK, "{}", copied.text());
    assert_eq!(copied.json()["path"], "archive/report.pdf");

    // The copy has the same bytes, and the original is untouched.
    let copy_id = copied.json()["id"].as_str().expect("id").to_owned();
    assert_eq!(
        app.get(&format!("/api/v1/items/{copy_id}/content"))
            .await
            .body,
        b"the contents"
    );
    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/content")).await.body,
        b"the contents"
    );
}

#[tokio::test]
async fn copying_onto_a_taken_name_keeps_both() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let uploaded = app.upload(&library, "notes.txt", b"first").await;
    app.upload(&library, "copy.txt", b"something else").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let copied = app
        .post_json(
            &format!("/api/v1/items/{id}/copy"),
            serde_json::json!({ "path": "copy.txt" }),
        )
        .await;

    // Never overwrite, here as anywhere else.
    assert_eq!(copied.json()["name"], "copy (2).txt");
    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/content")).await.body,
        b"first"
    );
}

#[tokio::test]
async fn a_folder_cannot_be_copied_yet_and_says_so() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            serde_json::json!({ "path": "trip" }),
        )
        .await;
    let id = created.json()["id"].as_str().expect("id").to_owned();

    let refused = app
        .post_json(
            &format!("/api/v1/items/{id}/copy"),
            serde_json::json!({ "path": "trip-copy" }),
        )
        .await;

    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.text().contains("folder"), "{}", refused.text());
}

#[tokio::test]
async fn a_copy_cannot_be_written_outside_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let uploaded = app.upload(&library, "notes.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let refused = app
        .post_json(
            &format!("/api/v1/items/{id}/copy"),
            serde_json::json!({ "path": "../escaped.txt" }),
        )
        .await;

    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
}
