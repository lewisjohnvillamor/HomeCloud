//! Authentication: setup, sign-in, sessions, and the failures that
//! matter more than the happy path.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

#[tokio::test]
async fn setup_creates_the_owner_and_signs_them_in() {
    let Some(app) = TestApp::create().await else {
        return;
    };

    let response = app.sign_up_owner().await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["authenticated"], true);
    assert_eq!(response.json()["display_name"], "Ada");

    let cookie = response
        .headers
        .get(axum::http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("a session cookie is set");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");

    app.cleanup().await;
}

#[tokio::test]
async fn the_session_endpoint_reports_the_signed_in_user() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    let response = app.get("/api/v1/session").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["authenticated"], true);
    assert_eq!(response.json()["display_name"], "Ada");

    app.cleanup().await;
}

#[tokio::test]
async fn an_anonymous_caller_is_reported_as_such_rather_than_as_an_error() {
    let Some(app) = TestApp::create().await else {
        return;
    };

    let response = app.get("/api/v1/session").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["authenticated"], false);

    app.cleanup().await;
}

#[tokio::test]
async fn setup_cannot_be_used_twice_to_take_over_a_deployment() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let response = app
        .post_json(
            "/api/v1/setup",
            json!({
                "display_name": "Mallory",
                "password": "another perfectly fine passphrase",
                "library_name": "Takeover",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json()["code"], "conflict");

    app.cleanup().await;
}

#[tokio::test]
async fn setup_enforces_the_password_policy() {
    let Some(app) = TestApp::create().await else {
        return;
    };

    let response = app
        .post_json(
            "/api/v1/setup",
            json!({
                "display_name": "Ada",
                "password": "short",
                "library_name": "Home",
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(
        response.text().contains("12 characters"),
        "{}",
        response.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn sign_in_succeeds_with_the_right_password() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let response = app
        .post_json(
            "/api/v1/auth/login",
            json!({"display_name": "Ada", "password": "correct horse battery staple"}),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["authenticated"], true);
    assert_eq!(
        app.get("/api/v1/session").await.json()["authenticated"],
        true
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_wrong_password_is_refused_without_revealing_whether_the_account_exists() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let wrong_password = app
        .post_json(
            "/api/v1/auth/login",
            json!({"display_name": "Ada", "password": "not the right passphrase"}),
        )
        .await;
    let unknown_account = app
        .post_json(
            "/api/v1/auth/login",
            json!({"display_name": "Grace", "password": "not the right passphrase"}),
        )
        .await;

    assert_eq!(wrong_password.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_account.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        wrong_password.json()["detail"],
        unknown_account.json()["detail"]
    );

    app.cleanup().await;
}

#[tokio::test]
async fn repeated_failures_are_throttled() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let mut throttled = false;
    for _ in 0..12 {
        let response = app
            .post_json(
                "/api/v1/auth/login",
                json!({"display_name": "Ada", "password": "wrong wrong wrong wrong"}),
            )
            .await;

        if response.status == StatusCode::TOO_MANY_REQUESTS {
            throttled = true;
            break;
        }
    }

    assert!(throttled, "password guessing was never throttled");

    app.cleanup().await;
}

#[tokio::test]
async fn signing_out_invalidates_the_session_immediately() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    let response = app.post_json("/api/v1/auth/logout", json!({})).await;

    assert_eq!(response.status, StatusCode::OK);
    let cleared = response
        .headers
        .get(axum::http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("the cookie is cleared");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    assert_eq!(
        app.get("/api/v1/session").await.json()["authenticated"],
        false
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_forged_session_cookie_does_not_authenticate() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    app.forget_session();

    let response = app
        .send(
            axum::http::Request::builder()
                .uri("/api/v1/session")
                .header(
                    axum::http::header::COOKIE,
                    "homecloud_session=nGOtaRealToken00000000000000000000000000000",
                )
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await;

    assert_eq!(response.json()["authenticated"], false);

    app.cleanup().await;
}

#[tokio::test]
async fn the_session_token_is_not_stored_in_the_database() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let response = app.sign_up_owner().await;
    let cookie = response
        .headers
        .get(axum::http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("cookie");
    let token = cookie
        .split(';')
        .next()
        .and_then(|pair| pair.split_once('='))
        .map(|(_, value)| value.to_owned())
        .expect("token");

    let matches: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM sessions WHERE encode(token_hash, 'escape') LIKE '%' || $1 || '%'",
    )
    .bind(&token)
    .fetch_one(&app.db.pool)
    .await
    .expect("query sessions");

    assert_eq!(matches.0, 0, "the raw token was stored");

    app.cleanup().await;
}

#[tokio::test]
async fn two_accounts_cannot_share_a_name() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;

    // Same name in different case: the database, not application code,
    // is the authority on this.
    let duplicate =
        sqlx::query("INSERT INTO users (display_name, password_hash) VALUES ('ada', 'x')")
            .execute(&app.db.pool)
            .await;

    assert!(duplicate.is_err(), "a duplicate account name was accepted");

    app.cleanup().await;
}

#[tokio::test]
async fn setup_starts_the_first_scan_so_existing_files_appear() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    // A folder that already has files in it, as a person pointing
    // HomeCloud at their existing library would have.
    app.put_on_disk("existing.txt", b"already here");

    app.sign_up_owner().await;
    let library = app.library_id().await;

    // The scan runs in the background; wait for it to settle.
    for _ in 0..100 {
        let listing = app
            .get(&format!("/api/v1/libraries/{library}/browse"))
            .await;
        if listing.json()["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            app.cleanup().await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    panic!("the initial scan never indexed the existing file");
}
