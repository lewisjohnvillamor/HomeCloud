//! Exact duplicate detection: same bytes, same hash.

mod support;

use axum::http::StatusCode;
use support::TestApp;

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

#[tokio::test]
async fn the_same_file_under_two_names_is_found() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // The same photo from the camera and from a message, as happens.
    app.upload(&library, "IMG_0042.jpg", b"the very same bytes")
        .await;
    app.upload(&library, "forwarded-photo.jpg", b"the very same bytes")
        .await;
    app.upload(&library, "different.jpg", b"other bytes entirely")
        .await;

    app.scan(&library).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());

    let groups = listed.json();
    let groups = groups.as_array().expect("groups");
    assert_eq!(groups.len(), 1, "{}", listed.text());

    let names: Vec<String> = groups[0]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(names.contains(&"IMG_0042.jpg".to_owned()), "{names:?}");
    assert!(
        names.contains(&"forwarded-photo.jpg".to_owned()),
        "{names:?}"
    );
    assert!(!names.contains(&"different.jpg".to_owned()), "{names:?}");
}

#[tokio::test]
async fn a_group_says_what_removing_the_extras_would_reclaim() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let bytes = vec![7u8; 1000];
    app.upload(&library, "one.bin", &bytes).await;
    app.upload(&library, "two.bin", &bytes).await;
    app.upload(&library, "three.bin", &bytes).await;

    app.scan(&library).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;

    // Three copies of a thousand bytes: keeping one frees two thousand.
    assert_eq!(listed.json()[0]["size_bytes"], 1000);
    assert_eq!(listed.json()[0]["reclaimable_bytes"], 2000);
}

#[tokio::test]
async fn files_of_the_same_size_but_different_contents_are_not_duplicates() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // Identical length, different bytes. A size-based check would call
    // these duplicates and invite someone to delete the wrong one.
    app.upload(&library, "a.bin", b"aaaaaaaaaa").await;
    app.upload(&library, "b.bin", b"bbbbbbbbbb").await;

    app.scan(&library).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;
    assert_eq!(listed.json().as_array().expect("groups").len(), 0);
}

#[tokio::test]
async fn a_trashed_copy_stops_counting_as_a_duplicate() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let first = app.upload(&library, "one.bin", b"same").await;
    app.upload(&library, "two.bin", b"same").await;
    app.scan(&library).await;

    let id = first.json()["id"].as_str().expect("id").to_owned();
    app.delete(&format!("/api/v1/items/{id}")).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;

    // One copy left is not a duplicate.
    assert_eq!(listed.json().as_array().expect("groups").len(), 0);
}

#[tokio::test]
async fn a_file_that_changed_is_hashed_again_rather_than_trusted() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.upload(&library, "one.bin", b"same").await;
    let second = app.upload(&library, "two.bin", b"same").await;
    app.scan(&library).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;
    assert_eq!(listed.json().as_array().expect("groups").len(), 1);

    // Replacing one of them makes them different files.
    let id = second.json()["id"].as_str().expect("id").to_owned();
    app.send(
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/items/{id}/content"))
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(
                "now quite different".as_bytes().to_vec(),
            ))
            .expect("valid request"),
    )
    .await;

    app.scan(&library).await;

    let after = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;
    assert_eq!(after.json().as_array().expect("groups").len(), 0);
}

#[tokio::test]
async fn duplicates_are_not_visible_outside_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "one.bin", b"same").await;
    app.upload(&library, "two.bin", b"same").await;
    app.scan(&library).await;

    app.forget_session();

    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/duplicates"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_empty_library_reports_no_duplicates_rather_than_failing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;

    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.json().as_array().expect("groups").len(), 0);
}
