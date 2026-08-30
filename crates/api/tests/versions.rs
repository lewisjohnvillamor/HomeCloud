//! What a file used to be: replacing contents, and putting them back.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Replaces a file's contents, as the app itself does.
async fn replace(app: &TestApp, item: &str, bytes: &[u8]) -> TestResponse {
    app.send(
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/items/{item}/content"))
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(bytes.to_vec()))
            .expect("valid request"),
    )
    .await
}

#[tokio::test]
async fn replacing_a_file_keeps_what_it_used_to_be() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"the first draft").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let replaced = replace(&app, &id, b"the second draft, which is longer").await;
    assert_eq!(replaced.status, StatusCode::OK, "{}", replaced.text());
    assert_eq!(replaced.json()["size_bytes"], 33);

    // The current contents are the new ones.
    let current = app.get(&format!("/api/v1/items/{id}/content")).await;
    assert_eq!(current.body, b"the second draft, which is longer");

    // And the old ones are still there to read.
    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;
    assert_eq!(versions.status, StatusCode::OK, "{}", versions.text());
    assert_eq!(versions.json().as_array().expect("versions").len(), 1);
    assert_eq!(versions.json()[0]["size_bytes"], 15);

    let version = versions.json()[0]["id"].as_str().expect("id").to_owned();
    let old = app
        .get(&format!("/api/v1/items/{id}/versions/{version}/content"))
        .await;
    assert_eq!(old.status, StatusCode::OK);
    assert_eq!(old.body, b"the first draft");
}

#[tokio::test]
async fn an_earlier_version_can_be_put_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"the first draft").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    replace(&app, &id, b"a worse draft").await;

    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;
    let version = versions.json()[0]["id"].as_str().expect("id").to_owned();

    let restored = app
        .post_json(
            &format!("/api/v1/items/{id}/versions/{version}/restore"),
            json!({}),
        )
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{}", restored.text());

    let current = app.get(&format!("/api/v1/items/{id}/content")).await;
    assert_eq!(current.body, b"the first draft");
    assert_eq!(restored.json()["size_bytes"], 15);
}

#[tokio::test]
async fn restoring_keeps_the_version_it_replaced() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"first").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    replace(&app, &id, b"second").await;
    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;
    let first = versions.json()[0]["id"].as_str().expect("id").to_owned();

    app.post_json(
        &format!("/api/v1/items/{id}/versions/{first}/restore"),
        json!({}),
    )
    .await;

    // A restore must never be a way to lose the file you had: what was
    // current is now the version.
    let after = app.get(&format!("/api/v1/items/{id}/versions")).await;
    assert_eq!(after.json().as_array().expect("versions").len(), 1);

    let second = after.json()[0]["id"].as_str().expect("id").to_owned();
    let kept = app
        .get(&format!("/api/v1/items/{id}/versions/{second}/content"))
        .await;
    assert_eq!(kept.body, b"second");
}

#[tokio::test]
async fn a_history_reads_newest_first() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"one").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    replace(&app, &id, b"two-two").await;
    replace(&app, &id, b"three-three-three").await;

    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;
    let sizes: Vec<i64> = versions
        .json()
        .as_array()
        .expect("versions")
        .iter()
        .map(|version| version["size_bytes"].as_i64().unwrap_or_default())
        .collect();

    // Most recently replaced first.
    assert_eq!(sizes, vec![7, 3]);
}

#[tokio::test]
async fn a_folder_has_no_contents_to_replace() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({ "path": "trip" }),
        )
        .await;
    let id = created.json()["id"].as_str().expect("id").to_owned();

    assert_eq!(
        replace(&app, &id, b"nonsense").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn versions_are_not_visible_outside_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"private").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    replace(&app, &id, b"still private").await;

    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;
    let version = versions.json()[0]["id"].as_str().expect("id").to_owned();

    app.forget_session();

    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/versions"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/versions/{version}/content"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        replace(&app, &id, b"intrusion").await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_version_of_another_file_is_not_reachable_through_this_one() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let first = app.upload(&library, "one.txt", b"one").await;
    let first_id = first.json()["id"].as_str().expect("id").to_owned();
    replace(&app, &first_id, b"one again").await;

    let second = app.upload(&library, "two.txt", b"two").await;
    let second_id = second.json()["id"].as_str().expect("id").to_owned();

    let versions = app.get(&format!("/api/v1/items/{first_id}/versions")).await;
    let version = versions.json()[0]["id"].as_str().expect("id").to_owned();

    // A version belongs to its file: asking for it through another one
    // is a "not found", not a shortcut.
    assert_eq!(
        app.get(&format!(
            "/api/v1/items/{second_id}/versions/{version}/content"
        ))
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post_json(
            &format!("/api/v1/items/{second_id}/versions/{version}/restore"),
            json!({})
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_replaced_photo_is_re_read_rather_than_keeping_the_old_details() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"short").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let replaced = replace(&app, &id, b"a considerably longer body of text").await;

    // The response describes the file as it now is, not as it was.
    assert_eq!(replaced.json()["size_bytes"], 34);

    let listed = app.get(&format!("/api/v1/items/{id}")).await;
    assert_eq!(listed.json()["size_bytes"], 34);
}

#[tokio::test]
async fn a_file_with_no_history_says_so_plainly() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"only ever this").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let versions = app.get(&format!("/api/v1/items/{id}/versions")).await;

    assert_eq!(versions.status, StatusCode::OK);
    assert_eq!(versions.json().as_array().expect("versions").len(), 0);
}

#[tokio::test]
async fn a_trashed_file_cannot_have_its_contents_replaced() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"here").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    app.delete(&format!("/api/v1/items/{id}")).await;

    assert_eq!(
        replace(&app, &id, b"while it is in the bin").await.status,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn the_version_store_is_not_library_content() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"first").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    replace(&app, &id, b"second").await;

    app.scan(&library).await;

    // A scan walks past the app's own directories: an old version must
    // never turn up in someone's file list.
    let browse = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    let names: Vec<String> = browse.json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();

    assert_eq!(names, vec!["notes.txt"]);
}
