//! Memories: what the engine offers, and being able to say "not that one".

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

fn keys(response: &support::TestResponse) -> Vec<String> {
    response
        .json()
        .as_array()
        .expect("memories")
        .iter()
        .map(|group| group["key"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn every_memory_carries_a_key_that_survives_a_second_request() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "photo.png", b"\x89PNG\r\n\x1a\n")
        .await;

    let first = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());

    let second = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;

    // A key that changed between requests could not be hidden.
    assert_eq!(keys(&first), keys(&second));
    assert!(!keys(&first).is_empty(), "{}", first.text());
}

#[tokio::test]
async fn a_memory_can_be_hidden_and_brought_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "photo.png", b"\x89PNG\r\n\x1a\n")
        .await;

    let before = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;
    let key = keys(&before).first().cloned().expect("a memory");

    let hidden = app
        .post_json(
            &format!("/api/v1/libraries/{library}/memories/hidden"),
            json!({ "key": key }),
        )
        .await;
    assert_eq!(hidden.status, StatusCode::OK, "{}", hidden.text());

    let after = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;
    assert!(!keys(&after).contains(&key), "{}", after.text());

    // The photographs are untouched: hiding hides a memory, not files.
    let photos = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;
    assert_eq!(photos.json().as_array().expect("photos").len(), 1);

    // And it comes straight back.
    let restored = app
        .delete(&format!(
            "/api/v1/libraries/{library}/memories/hidden/{key}"
        ))
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{}", restored.text());

    let again = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;
    assert!(keys(&again).contains(&key), "{}", again.text());
}

#[tokio::test]
async fn what_is_hidden_can_be_listed() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.post_json(
        &format!("/api/v1/libraries/{library}/memories/hidden"),
        json!({ "key": "on-this-day-12-25" }),
    )
    .await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/memories/hidden"))
        .await;

    // Otherwise hiding a memory is a decision nobody can find again.
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());
    assert_eq!(listed.json()[0], "on-this-day-12-25");
}

#[tokio::test]
async fn hiding_the_same_memory_twice_is_not_an_error() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    for _ in 0..2 {
        let hidden = app
            .post_json(
                &format!("/api/v1/libraries/{library}/memories/hidden"),
                json!({ "key": "on-this-day-01-01" }),
            )
            .await;
        assert_eq!(hidden.status, StatusCode::OK, "{}", hidden.text());
    }

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/memories/hidden"))
        .await;
    assert_eq!(listed.json().as_array().expect("keys").len(), 1);
}

#[tokio::test]
async fn a_key_that_is_not_a_memory_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    for key in ["", "   ", &"x".repeat(500)] {
        let refused = app
            .post_json(
                &format!("/api/v1/libraries/{library}/memories/hidden"),
                json!({ "key": key }),
            )
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{key:?}");
    }
}

#[tokio::test]
async fn hidden_memories_are_not_visible_outside_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.forget_session();

    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/memories/hidden"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.post_json(
            &format!("/api/v1/libraries/{library}/memories/hidden"),
            json!({ "key": "on-this-day-01-01" })
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
}
