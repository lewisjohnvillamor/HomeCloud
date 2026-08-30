//! Public share links.
//!
//! A share is a capability: read access to one item and nothing else.
//! Most of these tests are about what a link *cannot* do.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Creates a share for an item and returns `(share id, token)`.
async fn share(app: &TestApp, item_id: &str, body: serde_json::Value) -> (String, String) {
    let response = app
        .post_json(&format!("/api/v1/items/{item_id}/shares"), body)
        .await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());

    (
        response.json()["id"].as_str().expect("id").to_owned(),
        response.json()["token"].as_str().expect("token").to_owned(),
    )
}

#[tokio::test]
async fn a_shared_file_can_be_read_without_signing_in() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "report.txt", b"quarterly numbers")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let (_, token) = share(&app, &id, json!({})).await;
    app.forget_session();

    let view = app.get(&format!("/api/v1/public/{token}")).await;
    let content = app.get(&format!("/api/v1/public/{token}/content")).await;

    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.json()["item"]["name"], "report.txt");
    assert_eq!(content.status, StatusCode::OK);
    assert_eq!(content.text(), "quarterly numbers");

    app.cleanup().await;
}

#[tokio::test]
async fn the_token_is_shown_once_and_never_stored() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (_, token) = share(&app, &id, json!({})).await;

    // Listing the share never returns the token again.
    let listed = app.get(&format!("/api/v1/items/{id}/shares")).await;
    assert!(listed.json()[0]["token"].is_null(), "{}", listed.text());

    let stored: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM shares WHERE encode(token_hash, 'escape') LIKE '%' || $1 || '%'",
    )
    .bind(&token)
    .fetch_one(&app.db.pool)
    .await
    .expect("query shares");
    assert_eq!(stored.0, 0, "the raw token was stored");

    app.cleanup().await;
}

#[tokio::test]
async fn a_revoked_link_stops_working_immediately() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (share_id, token) = share(&app, &id, json!({})).await;

    assert_eq!(
        app.get(&format!("/api/v1/public/{token}")).await.status,
        StatusCode::OK
    );

    let revoked = app.delete(&format!("/api/v1/shares/{share_id}")).await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);

    app.forget_session();
    let after = app.get(&format!("/api/v1/public/{token}")).await;
    assert_eq!(after.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn an_expired_link_stops_working() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (share_id, token) = share(&app, &id, json!({"expires_in_days": 1})).await;

    // Move the expiry into the past, as time would.
    sqlx::query("UPDATE shares SET expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&share_id).expect("uuid"))
        .execute(&app.db.pool)
        .await
        .expect("expire the share");

    app.forget_session();
    let response = app.get(&format!("/api/v1/public/{token}")).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn an_unreasonable_expiry_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    for days in [0, -5, 4000] {
        let response = app
            .post_json(
                &format!("/api/v1/items/{id}/shares"),
                json!({ "expires_in_days": days }),
            )
            .await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{days}");
    }

    app.cleanup().await;
}

#[tokio::test]
async fn a_made_up_token_reveals_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    signed_in_library(&app).await;
    app.forget_session();

    for candidate in [
        "not-a-real-token",
        "../../etc/passwd",
        &"a".repeat(5000),
        "",
    ] {
        let response = app.get(&format!("/api/v1/public/{candidate}")).await;

        assert_eq!(
            response.status,
            StatusCode::NOT_FOUND,
            "unexpected answer for `{}`",
            &candidate[..candidate.len().min(20)]
        );
    }

    app.cleanup().await;
}

#[tokio::test]
async fn a_share_grants_no_access_to_the_rest_of_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let shared = app.upload(&library, "shared.txt", b"public").await;
    let private = app.upload(&library, "private.txt", b"secret").await;
    let shared_id = shared.json()["id"].as_str().expect("id").to_owned();
    let private_id = private.json()["id"].as_str().expect("id").to_owned();

    let (_, token) = share(&app, &shared_id, json!({})).await;
    app.forget_session();

    // The capability names one item. Pointing it at another is refused,
    // and the session-scoped routes stay closed.
    let other_item = app
        .get(&format!("/api/v1/public/{token}?item={private_id}"))
        .await;
    let browse = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    let direct = app
        .get(&format!("/api/v1/items/{private_id}/content"))
        .await;

    assert_eq!(other_item.status, StatusCode::NOT_FOUND);
    assert_eq!(browse.status, StatusCode::UNAUTHORIZED);
    assert_eq!(direct.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn a_shared_folder_exposes_what_is_inside_it_and_no_more() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("trip/day1/beach.jpg", b"jpeg");
    app.put_on_disk("trip/notes.txt", b"where we went");
    app.put_on_disk("private/taxes.pdf", b"pdf");
    app.scan(&library).await;

    let folder = app.find_item(&library, "", "trip").await;
    let outside = app.find_item(&library, "", "private").await;
    let inner = app.find_item(&library, "trip", "notes.txt").await;
    let (_, token) = share(&app, folder["id"].as_str().expect("id"), json!({})).await;
    app.forget_session();

    let root = app.get(&format!("/api/v1/public/{token}")).await;
    let names: Vec<String> = root.json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, vec!["day1", "notes.txt"]);

    // Something inside the shared folder is reachable...
    let nested = app
        .get(&format!(
            "/api/v1/public/{token}?item={}",
            inner["id"].as_str().expect("id")
        ))
        .await;
    assert_eq!(nested.status, StatusCode::OK);
    // ...and the path shown is relative to the share, not the library.
    assert_eq!(nested.json()["relative_path"], "notes.txt");

    // A sibling folder is not.
    let escape = app
        .get(&format!(
            "/api/v1/public/{token}?item={}",
            outside["id"].as_str().expect("id")
        ))
        .await;
    assert_eq!(escape.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn a_share_is_read_only() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (_, token) = share(&app, &id, json!({})).await;
    app.forget_session();

    // Every mutating route still needs a session; the token buys nothing.
    let renamed = app
        .post_json(
            &format!("/api/v1/items/{id}/move"),
            json!({"path": "owned.txt"}),
        )
        .await;
    let deleted = app.delete(&format!("/api/v1/items/{id}")).await;
    let uploaded_again = app.upload(&library, "sneaky.txt", b"x").await;

    assert_eq!(renamed.status, StatusCode::UNAUTHORIZED);
    assert_eq!(deleted.status, StatusCode::UNAUTHORIZED);
    assert_eq!(uploaded_again.status, StatusCode::UNAUTHORIZED);
    // The link still reads, which is all it was ever for.
    assert_eq!(
        app.get(&format!("/api/v1/public/{token}/content"))
            .await
            .status,
        StatusCode::OK
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_trashed_item_is_not_served_through_its_link() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (_, token) = share(&app, &id, json!({})).await;

    app.delete(&format!("/api/v1/items/{id}")).await;
    app.forget_session();

    let response = app.get(&format!("/api/v1/public/{token}")).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn the_owner_can_see_and_audit_live_links() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (_, token) = share(&app, &id, json!({})).await;

    app.get(&format!("/api/v1/public/{token}")).await;
    app.get(&format!("/api/v1/public/{token}")).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/shares"))
        .await;

    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.json()[0]["item_name"], "report.txt");
    assert_eq!(listed.json()[0]["access_count"], 2);

    app.cleanup().await;
}

#[tokio::test]
async fn only_a_member_can_create_or_revoke_a_share() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let (share_id, _) = share(&app, &id, json!({})).await;

    app.forget_session();

    let created = app
        .post_json(&format!("/api/v1/items/{id}/shares"), json!({}))
        .await;
    let revoked = app.delete(&format!("/api/v1/shares/{share_id}")).await;

    assert_eq!(created.status, StatusCode::UNAUTHORIZED);
    assert_eq!(revoked.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}
