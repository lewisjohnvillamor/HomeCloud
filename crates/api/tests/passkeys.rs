//! Passkey ceremonies.
//!
//! A browser and an authenticator are needed to complete a real
//! ceremony, so that path is covered by the end-to-end journeys with a
//! virtual authenticator. These tests cover what the server does on its
//! own: who may start a ceremony, what a challenge discloses, and what
//! happens to a challenge that is stale, forged, or someone else's.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

#[tokio::test]
async fn a_signed_in_person_can_start_registering_a_passkey() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    let response = app
        .post_json("/api/v1/auth/passkeys/register/options", json!({}))
        .await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let body = response.json();
    assert!(body["ceremony_id"].is_string());
    // The challenge is passed to the browser unchanged.
    assert!(
        body["options"]["publicKey"]["challenge"].is_string(),
        "{}",
        response.text()
    );
    assert_eq!(body["options"]["publicKey"]["rp"]["id"], "localhost");

    app.cleanup().await;
}

#[tokio::test]
async fn registering_a_passkey_requires_a_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let options = app
        .post_json("/api/v1/auth/passkeys/register/options", json!({}))
        .await;
    let listed = app.get("/api/v1/auth/passkeys").await;

    assert_eq!(options.status, StatusCode::UNAUTHORIZED);
    assert_eq!(listed.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn an_account_with_no_passkey_cannot_sign_in_with_one() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let response = app
        .post_json(
            "/api/v1/auth/passkeys/login/options",
            json!({"display_name": "Ada"}),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn an_unknown_account_is_refused_the_same_way_as_one_without_passkeys() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let no_passkey = app
        .post_json(
            "/api/v1/auth/passkeys/login/options",
            json!({"display_name": "Ada"}),
        )
        .await;
    let no_account = app
        .post_json(
            "/api/v1/auth/passkeys/login/options",
            json!({"display_name": "Nobody"}),
        )
        .await;

    assert_eq!(no_passkey.status, no_account.status);
    assert_eq!(no_passkey.json()["detail"], no_account.json()["detail"]);

    app.cleanup().await;
}

#[tokio::test]
async fn a_forged_ceremony_id_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    for ceremony in ["not-a-uuid", "00000000-0000-0000-0000-000000000000"] {
        let response = app
            .post_json(
                "/api/v1/auth/passkeys/register/verify",
                json!({
                    "ceremony_id": ceremony,
                    "nickname": "Laptop",
                    "credential": {
                        "id": "AAAA",
                        "rawId": "AAAA",
                        "type": "public-key",
                        "response": {
                            "attestationObject": "AAAA",
                            "clientDataJSON": "AAAA"
                        }
                    }
                }),
            )
            .await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{ceremony}");
    }

    app.cleanup().await;
}

#[tokio::test]
async fn one_persons_challenge_cannot_be_completed_by_another() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    // Ada starts a registration.
    let started = app
        .post_json("/api/v1/auth/passkeys/register/options", json!({}))
        .await;
    let ceremony = started.json()["ceremony_id"]
        .as_str()
        .expect("id")
        .to_owned();

    // A second account tries to finish it.
    let intruder: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (display_name, password_hash) VALUES ('Mallory', 'x') RETURNING id",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("create user");
    let session = homecloud_auth::session::create(
        &app.db.pool,
        homecloud_domain::identity::UserId::from_uuid(intruder),
    )
    .await
    .expect("session");

    let response = app
        .send(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/passkeys/register/verify")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .header(
                    axum::http::header::COOKIE,
                    format!("homecloud_session={}", session.expose()),
                )
                .body(axum::body::Body::from(
                    json!({
                        "ceremony_id": ceremony,
                        "credential": {
                            "id": "AAAA",
                            "rawId": "AAAA",
                            "type": "public-key",
                            "response": {
                                "attestationObject": "AAAA",
                                "clientDataJSON": "AAAA"
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn a_person_only_sees_and_removes_their_own_passkeys() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let owner: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE display_name = 'Ada'")
        .fetch_one(&app.db.pool)
        .await
        .expect("owner");

    // A credential belonging to somebody else.
    let other: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (display_name, password_hash) VALUES ('Grace', 'x') RETURNING id",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("create user");
    let credential: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO credentials (user_id, credential_id, passkey, nickname)
         VALUES ($1, $2, '{}'::jsonb, 'Their laptop') RETURNING id",
    )
    .bind(other)
    .bind(b"another-credential".to_vec())
    .fetch_one(&app.db.pool)
    .await
    .expect("create credential");

    let listed = app.get("/api/v1/auth/passkeys").await;
    let removed = app
        .delete(&format!("/api/v1/auth/passkeys/{credential}"))
        .await;

    assert_eq!(listed.status, StatusCode::OK);
    assert!(listed.json().as_array().expect("passkeys").is_empty());
    assert_eq!(removed.status, StatusCode::NOT_FOUND);
    // Still there.
    let survived: (i64,) = sqlx::query_as("SELECT count(*) FROM credentials WHERE id = $1")
        .bind(credential)
        .fetch_one(&app.db.pool)
        .await
        .expect("count");
    assert_eq!(survived.0, 1);
    assert_ne!(owner, other);

    app.cleanup().await;
}

#[tokio::test]
async fn passkeys_report_themselves_unavailable_when_no_public_origin_is_set() {
    let Some(db) = support::TestDatabase::create().await else {
        return;
    };
    let root = tempfile::TempDir::new().expect("temporary root");
    let state = homecloud_api::app::AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(root.path().to_path_buf()),
    );

    assert!(
        !homecloud_api::passkeys::is_available(&state),
        "passkeys must be off without a configured origin"
    );

    db.cleanup().await;
}
