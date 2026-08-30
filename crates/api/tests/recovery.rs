//! Account recovery and password-protected share links.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

/// The code handed back at setup, shown once.
async fn owner_recovery_code(app: &TestApp) -> String {
    let response = app.sign_up_owner().await;

    response.json()["recovery_code"]
        .as_str()
        .unwrap_or_else(|| panic!("setup did not return a recovery code: {}", response.text()))
        .to_owned()
}

#[tokio::test]
async fn setup_hands_back_a_recovery_code_without_being_asked() {
    let Some(app) = TestApp::create().await else {
        return;
    };

    let code = owner_recovery_code(&app).await;

    // Five readable groups, and nothing that looks like anything else.
    assert_eq!(code.split('-').count(), 5, "{code}");
    let status = app.get("/api/v1/auth/recovery").await;
    assert_eq!(status.json()["has_code"], true);
    // The code itself is never returned again.
    assert!(status.json()["code"].is_null(), "{}", status.text());

    app.cleanup().await;
}

#[tokio::test]
async fn the_code_is_stored_hashed_not_in_the_clear() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let code = owner_recovery_code(&app).await;

    let stored: (Option<String>,) =
        sqlx::query_as("SELECT recovery_code_hash FROM users WHERE display_name = 'Ada'")
            .fetch_one(&app.db.pool)
            .await
            .expect("query");

    let stored = stored.0.expect("a stored hash");
    assert!(!stored.contains(&code), "the code was stored in the clear");
    assert!(stored.starts_with("$argon2id$"), "{stored}");

    app.cleanup().await;
}

#[tokio::test]
async fn a_forgotten_password_can_be_recovered_with_the_code() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let code = owner_recovery_code(&app).await;
    app.forget_session();

    let recovered = app
        .post_json(
            "/api/v1/auth/recover",
            json!({
                "display_name": "Ada",
                "recovery_code": code,
                "new_password": "a completely different passphrase",
            }),
        )
        .await;

    assert_eq!(recovered.status, StatusCode::OK, "{}", recovered.text());
    assert_eq!(recovered.json()["authenticated"], true);
    // Signed in immediately, and handed a fresh code in the same breath.
    assert!(recovered.json()["recovery_code"].is_string());
    assert_eq!(
        app.get("/api/v1/session").await.json()["authenticated"],
        true
    );

    // The new password works and the old one does not.
    app.forget_session();
    let with_new = app
        .post_json(
            "/api/v1/auth/login",
            json!({"display_name": "Ada", "password": "a completely different passphrase"}),
        )
        .await;
    assert_eq!(with_new.status, StatusCode::OK);

    app.forget_session();
    let with_old = app
        .post_json(
            "/api/v1/auth/login",
            json!({"display_name": "Ada", "password": "correct horse battery staple"}),
        )
        .await;
    assert_eq!(with_old.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn recovery_ends_every_existing_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let code = owner_recovery_code(&app).await;
    let old_session = app.session_cookie().expect("a session");

    app.forget_session();
    app.post_json(
        "/api/v1/auth/recover",
        json!({
            "display_name": "Ada",
            "recovery_code": code,
            "new_password": "a completely different passphrase",
        }),
    )
    .await;

    // Recovery is what someone does after a compromise, so whoever held
    // the old session loses it.
    let with_old_session = app
        .send(
            axum::http::Request::builder()
                .uri("/api/v1/libraries")
                .header(axum::http::header::COOKIE, old_session)
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await;

    assert_eq!(with_old_session.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn a_code_cannot_be_used_twice() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let code = owner_recovery_code(&app).await;
    app.forget_session();

    let first = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Ada", "recovery_code": code.clone(), "new_password": "first new passphrase here"}),
        )
        .await;
    app.forget_session();
    let second = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Ada", "recovery_code": code, "new_password": "second new passphrase here"}),
        )
        .await;

    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(second.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn a_wrong_code_is_refused_and_reveals_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    owner_recovery_code(&app).await;
    app.forget_session();

    let wrong_code = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Ada", "recovery_code": "ABCDE-FGHJK-MNPQR-STUVW-XYZ23", "new_password": "a new passphrase here"}),
        )
        .await;
    let unknown_account = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Nobody", "recovery_code": "ABCDE-FGHJK-MNPQR-STUVW-XYZ23", "new_password": "a new passphrase here"}),
        )
        .await;

    assert_eq!(wrong_code.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_account.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        wrong_code.json()["detail"],
        unknown_account.json()["detail"]
    );

    app.cleanup().await;
}

#[tokio::test]
async fn recovery_shares_the_sign_in_throttle() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    owner_recovery_code(&app).await;
    app.forget_session();

    let mut throttled = false;
    for _ in 0..12 {
        let response = app
            .post_json(
                "/api/v1/auth/recover",
                json!({"display_name": "Ada", "recovery_code": "AAAAA-AAAAA-AAAAA-AAAAA-AAAAA", "new_password": "a new passphrase here"}),
            )
            .await;

        if response.status == StatusCode::TOO_MANY_REQUESTS {
            throttled = true;
            break;
        }
    }

    assert!(throttled, "recovery codes could be guessed without limit");

    app.cleanup().await;
}

#[tokio::test]
async fn regenerating_replaces_the_old_code() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let first = owner_recovery_code(&app).await;

    let regenerated = app.post_json("/api/v1/auth/recovery", json!({})).await;
    let second = regenerated.json()["code"]
        .as_str()
        .expect("a code")
        .to_owned();
    assert_ne!(first, second);

    app.forget_session();
    let with_old = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Ada", "recovery_code": first, "new_password": "a new passphrase here"}),
        )
        .await;

    assert_eq!(with_old.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn a_weak_new_password_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let code = owner_recovery_code(&app).await;
    app.forget_session();

    let response = app
        .post_json(
            "/api/v1/auth/recover",
            json!({"display_name": "Ada", "recovery_code": code, "new_password": "short"}),
        )
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

// --- Password-protected share links ---

#[tokio::test]
async fn a_protected_link_discloses_nothing_until_it_is_unlocked() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let uploaded = app
        .upload(&library, "private-report.txt", b"contents")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let share = app
        .post_json(
            &format!("/api/v1/items/{id}/shares"),
            json!({"password": "sunflower77"}),
        )
        .await;
    assert_eq!(share.json()["protected"], true);
    let token = share.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();
    let locked = app.get(&format!("/api/v1/public/{token}")).await;

    assert_eq!(locked.status, StatusCode::UNAUTHORIZED);
    assert_eq!(locked.json()["code"], "password_required");
    // Not even the file's name leaks before the password is proved.
    assert!(
        !locked.text().contains("private-report"),
        "{}",
        locked.text()
    );
    // Nor does the content endpoint hand the bytes over.
    let content = app.get(&format!("/api/v1/public/{token}/content")).await;
    assert_eq!(content.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn the_right_password_unlocks_the_link_for_a_while() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let uploaded = app
        .upload(&library, "report.txt", b"quarterly numbers")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let share = app
        .post_json(
            &format!("/api/v1/items/{id}/shares"),
            json!({"password": "sunflower77"}),
        )
        .await;
    let token = share.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();
    let unlocked = app
        .post_json(
            &format!("/api/v1/public/{token}/unlock"),
            json!({"password": "sunflower77"}),
        )
        .await;

    assert_eq!(unlocked.status, StatusCode::OK, "{}", unlocked.text());
    let key = unlocked.json()["key"].as_str().expect("a key").to_owned();
    // The key is not the password.
    assert_ne!(key, "sunflower77");

    let view = app.get(&format!("/api/v1/public/{token}?key={key}")).await;
    let content = app
        .get(&format!("/api/v1/public/{token}/content?key={key}"))
        .await;

    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.json()["item"]["name"], "report.txt");
    assert_eq!(content.text(), "quarterly numbers");

    app.cleanup().await;
}

#[tokio::test]
async fn a_wrong_share_password_is_refused_and_throttled() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let share = app
        .post_json(
            &format!("/api/v1/items/{id}/shares"),
            json!({"password": "sunflower77"}),
        )
        .await;
    let token = share.json()["token"].as_str().expect("token").to_owned();

    app.forget_session();

    let mut throttled = false;
    for _ in 0..12 {
        let response = app
            .post_json(
                &format!("/api/v1/public/{token}/unlock"),
                json!({"password": "guessing"}),
            )
            .await;

        if response.status == StatusCode::TOO_MANY_REQUESTS {
            throttled = true;
            break;
        }
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }

    assert!(throttled, "a link password could be guessed without limit");

    app.cleanup().await;
}

#[tokio::test]
async fn an_unlock_key_does_not_open_a_different_link() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let first = app.upload(&library, "one.txt", b"one").await;
    let second = app.upload(&library, "two.txt", b"two").await;

    async fn make_share(app: &TestApp, id: &str) -> support::TestResponse {
        app.post_json(
            &format!("/api/v1/items/{id}/shares"),
            json!({"password": "sunflower77"}),
        )
        .await
    }
    let first_share = make_share(&app, first.json()["id"].as_str().expect("id")).await;
    let second_share = make_share(&app, second.json()["id"].as_str().expect("id")).await;
    let first_token = first_share.json()["token"]
        .as_str()
        .expect("token")
        .to_owned();
    let second_token = second_share.json()["token"]
        .as_str()
        .expect("token")
        .to_owned();

    app.forget_session();
    let unlocked = app
        .post_json(
            &format!("/api/v1/public/{first_token}/unlock"),
            json!({"password": "sunflower77"}),
        )
        .await;
    let key = unlocked.json()["key"].as_str().expect("a key").to_owned();

    let crossed = app
        .get(&format!("/api/v1/public/{second_token}?key={key}"))
        .await;

    assert_eq!(crossed.status, StatusCode::UNAUTHORIZED);
    assert_eq!(crossed.json()["code"], "password_required");

    app.cleanup().await;
}

#[tokio::test]
async fn an_unprotected_link_still_opens_without_a_password() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let uploaded = app.upload(&library, "public.txt", b"open").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let share = app
        .post_json(&format!("/api/v1/items/{id}/shares"), json!({}))
        .await;
    let token = share.json()["token"].as_str().expect("token").to_owned();

    assert_eq!(share.json()["protected"], false);

    app.forget_session();
    let view = app.get(&format!("/api/v1/public/{token}")).await;

    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.json()["item"]["name"], "public.txt");
    // And unlocking one that has no password is not a way in either.
    let unlock = app
        .post_json(
            &format!("/api/v1/public/{token}/unlock"),
            json!({"password": "x"}),
        )
        .await;
    assert_eq!(unlock.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn a_short_link_password_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app
        .post_json(
            &format!("/api/v1/items/{id}/shares"),
            json!({"password": "short"}),
        )
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}
