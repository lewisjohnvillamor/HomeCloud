//! Multi-user: invitations, membership, and the powers a member does
//! not have.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Creates an invitation and returns its token.
async fn invite(app: &TestApp, library: &str) -> String {
    let response = app
        .post_json(
            &format!("/api/v1/libraries/{library}/invitations"),
            json!({}),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());

    response.json()["token"].as_str().expect("token").to_owned()
}

/// Accepts an invitation as a brand-new person. The harness's cookie jar
/// then holds that person's session.
async fn accept_as(app: &TestApp, token: &str, name: &str) -> TestResponse {
    app.forget_session();

    app.post_json(
        &format!("/api/v1/invitations/{token}/accept"),
        json!({ "display_name": name, "password": "a perfectly good passphrase" }),
    )
    .await
}

#[tokio::test]
async fn an_invited_person_creates_an_account_and_sees_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "shared-notes.txt", b"family stuff")
        .await;
    let token = invite(&app, &library).await;

    let accepted = accept_as(&app, &token, "Grace").await;

    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.text());
    assert_eq!(accepted.json()["authenticated"], true);
    assert_eq!(accepted.json()["display_name"], "Grace");

    // They are signed in and can see the library's contents.
    let libraries = app.get("/api/v1/libraries").await;
    assert_eq!(libraries.json()[0]["id"], library);
    assert_eq!(libraries.json()[0]["role"], "member");

    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    assert_eq!(listing.json()["items"][0]["name"], "shared-notes.txt");

    app.cleanup().await;
}

#[tokio::test]
async fn an_invitation_says_what_it_is_for_and_nothing_more() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "private.txt", b"secret").await;
    let token = invite(&app, &library).await;
    app.forget_session();

    let preview = app
        .get(&format!("/api/v1/invitations/{token}/preview"))
        .await;

    assert_eq!(preview.status, StatusCode::OK);
    assert_eq!(preview.json()["library_name"], "Home");
    assert_eq!(preview.json()["invited_by"], "Ada");
    // No file names, no ids, no member list.
    let rendered = preview.text();
    assert!(!rendered.contains("private.txt"), "{rendered}");
    assert!(!rendered.contains(&library), "{rendered}");

    app.cleanup().await;
}

#[tokio::test]
async fn an_invitation_can_only_be_used_once() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;

    let first = accept_as(&app, &token, "Grace").await;
    let second = accept_as(&app, &token, "Mallory").await;

    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(second.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn a_revoked_invitation_cannot_be_accepted() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;

    let pending = app
        .get(&format!("/api/v1/libraries/{library}/invitations"))
        .await;
    let id = pending.json()[0]["id"].as_str().expect("id").to_owned();
    let revoked = app.delete(&format!("/api/v1/invitations/{id}")).await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);

    let accepted = accept_as(&app, &token, "Grace").await;

    assert_eq!(accepted.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn an_expired_invitation_cannot_be_accepted() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;

    sqlx::query("UPDATE invitations SET expires_at = now() - interval '1 minute'")
        .execute(&app.db.pool)
        .await
        .expect("expire the invitation");

    let preview = app
        .get(&format!("/api/v1/invitations/{token}/preview"))
        .await;
    let accepted = accept_as(&app, &token, "Grace").await;

    assert_eq!(preview.status, StatusCode::NOT_FOUND);
    assert_eq!(accepted.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn an_unreasonable_invitation_expiry_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    for days in [0, -1, 900] {
        let response = app
            .post_json(
                &format!("/api/v1/libraries/{library}/invitations"),
                json!({ "expires_in_days": days }),
            )
            .await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{days}");
    }

    app.cleanup().await;
}

#[tokio::test]
async fn a_made_up_invitation_token_reveals_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    signed_in_library(&app).await;
    app.forget_session();

    for candidate in ["nope", "../../etc/passwd", &"a".repeat(4000)] {
        let response = app
            .get(&format!("/api/v1/invitations/{candidate}/preview"))
            .await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
    }

    app.cleanup().await;
}

// --- What a member may not do ---

#[tokio::test]
async fn a_member_cannot_invite_anyone_else() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;
    accept_as(&app, &token, "Grace").await;

    // Now acting as Grace, a member.
    let invited = app
        .post_json(
            &format!("/api/v1/libraries/{library}/invitations"),
            json!({}),
        )
        .await;
    let listed = app
        .get(&format!("/api/v1/libraries/{library}/invitations"))
        .await;

    assert_eq!(invited.status, StatusCode::FORBIDDEN);
    assert_eq!(listed.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

#[tokio::test]
async fn a_member_cannot_remove_anyone_including_the_owner() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;
    accept_as(&app, &token, "Grace").await;

    let members = app
        .get(&format!("/api/v1/libraries/{library}/members"))
        .await;
    let owner_id = members
        .json()
        .as_array()
        .expect("members")
        .iter()
        .find(|member| member["role"] == "owner")
        .and_then(|member| member["user_id"].as_str())
        .expect("an owner")
        .to_owned();

    let removed = app
        .delete(&format!("/api/v1/libraries/{library}/members/{owner_id}"))
        .await;

    assert_eq!(removed.status, StatusCode::FORBIDDEN);
    // The owner is still there.
    let after = app
        .get(&format!("/api/v1/libraries/{library}/members"))
        .await;
    assert_eq!(after.json().as_array().expect("members").len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn the_owner_cannot_be_removed_even_by_themselves() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let members = app
        .get(&format!("/api/v1/libraries/{library}/members"))
        .await;
    let owner_id = members.json()[0]["user_id"]
        .as_str()
        .expect("id")
        .to_owned();

    let removed = app
        .delete(&format!("/api/v1/libraries/{library}/members/{owner_id}"))
        .await;

    assert_eq!(removed.status, StatusCode::NOT_FOUND);
    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/members"))
            .await
            .json()
            .as_array()
            .expect("members")
            .len(),
        1
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_removed_member_loses_access_immediately() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;
    accept_as(&app, &token, "Grace").await;

    // Grace can read the library right now.
    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/browse"))
            .await
            .status,
        StatusCode::OK
    );
    let grace_id = app.get("/api/v1/session").await.json()["user_id"]
        .as_str()
        .expect("id")
        .to_owned();
    let grace_cookie = app.session_cookie().expect("Grace has a session");

    // The owner signs back in and removes her.
    app.forget_session();
    app.post_json(
        "/api/v1/auth/login",
        json!({"display_name": "Ada", "password": "correct horse battery staple"}),
    )
    .await;
    let removed = app
        .delete(&format!("/api/v1/libraries/{library}/members/{grace_id}"))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);

    // Her old session is gone, not merely powerless.
    let with_old_session = app
        .send(
            axum::http::Request::builder()
                .uri(format!("/api/v1/libraries/{library}/browse"))
                .header(axum::http::header::COOKIE, grace_cookie)
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await;

    assert_eq!(with_old_session.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn members_can_see_who_else_is_in_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;
    accept_as(&app, &token, "Grace").await;

    let members = app
        .get(&format!("/api/v1/libraries/{library}/members"))
        .await;

    assert_eq!(members.status, StatusCode::OK);
    let names: Vec<String> = members
        .json()
        .as_array()
        .expect("members")
        .iter()
        .map(|member| {
            member["display_name"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(names, vec!["Ada", "Grace"]);
    assert_eq!(members.json()[0]["role"], "owner");
    assert_eq!(members.json()[1]["is_you"], true);

    app.cleanup().await;
}

#[tokio::test]
async fn a_member_can_work_with_the_libraries_files() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;
    accept_as(&app, &token, "Grace").await;

    // Membership is read *and* write: a shared library that only one
    // person can add to is not a shared library.
    let uploaded = app.upload(&library, "graces-notes.txt", b"hello").await;
    let folder = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({"path": "Graces folder"}),
        )
        .await;

    assert_eq!(uploaded.status, StatusCode::OK);
    assert_eq!(folder.status, StatusCode::OK);

    app.cleanup().await;
}

#[tokio::test]
async fn someone_outside_the_library_sees_nothing_of_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // An account with no membership at all, created through its own
    // invitation to a different library.
    let outsider: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (display_name, password_hash) VALUES ('Mallory', 'x') RETURNING id",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("create user");
    let session = homecloud_auth::session::create(
        &app.db.pool,
        homecloud_domain::identity::UserId::from_uuid(outsider),
    )
    .await
    .expect("session");

    app.forget_session();

    async fn as_outsider(app: &TestApp, token: &str, path: String) -> TestResponse {
        app.send(
            axum::http::Request::builder()
                .uri(path)
                .header(
                    axum::http::header::COOKIE,
                    format!("homecloud_session={token}"),
                )
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await
    }

    let token = session.expose();
    let members = as_outsider(&app, token, format!("/api/v1/libraries/{library}/members")).await;
    let invitations = as_outsider(
        &app,
        token,
        format!("/api/v1/libraries/{library}/invitations"),
    )
    .await;

    // Not "forbidden": an outsider must not learn the library exists.
    assert_eq!(members.status, StatusCode::NOT_FOUND);
    assert_eq!(invitations.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn accepting_while_signed_in_adds_the_membership_to_that_account() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let token = invite(&app, &library).await;

    // The owner accepts their own invitation: already a member, so this
    // must not create a second account or a duplicate membership.
    let accepted = app
        .post_json(&format!("/api/v1/invitations/{token}/accept"), json!({}))
        .await;

    assert_eq!(accepted.status, StatusCode::OK);
    assert_eq!(accepted.json()["display_name"], "Ada");
    let members = app
        .get(&format!("/api/v1/libraries/{library}/members"))
        .await;
    assert_eq!(members.json().as_array().expect("members").len(), 1);
    assert_eq!(members.json()[0]["role"], "owner");

    app.cleanup().await;
}
