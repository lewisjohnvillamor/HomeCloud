//! Backing up a phone, and — the part that actually matters — not
//! sending the same photograph twice.
//!
//! A first backup is a long upload however it is done. What decides
//! whether anybody runs a second one is what happens when they select
//! the same ten thousand pictures again next month.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Registers a phone and returns its id and folder.
async fn phone(app: &TestApp, library: &str, name: &str) -> (String, String) {
    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/backup/devices"),
            json!({ "name": name }),
        )
        .await;

    assert_eq!(created.status, StatusCode::OK, "{:?}", created.json());
    let body = created.json();
    (
        body["id"].as_str().expect("id").to_owned(),
        body["folder"].as_str().expect("folder").to_owned(),
    )
}

/// Asks which of these the server has not got.
async fn check(app: &TestApp, library: &str, device: &str, files: serde_json::Value) -> TestCheck {
    let response = app
        .post_json(
            &format!("/api/v1/libraries/{library}/backup/devices/{device}/check"),
            json!({ "files": files }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK, "{:?}", response.json());
    let body = response.json();

    TestCheck {
        missing: body["missing"]
            .as_array()
            .expect("missing")
            .iter()
            .map(|name| name.as_str().expect("name").to_owned())
            .collect(),
        already_here: body["already_here"].as_u64().expect("already_here"),
    }
}

struct TestCheck {
    missing: Vec<String>,
    already_here: u64,
}

#[tokio::test]
async fn the_first_backup_asks_for_everything() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    assert_eq!(folder, "Phone backups/Ada's phone");

    let answer = check(
        &app,
        &library,
        &device,
        json!([
            { "name": "IMG_0001.jpg", "size_bytes": 12 },
            { "name": "IMG_0002.jpg", "size_bytes": 34 },
        ]),
    )
    .await;

    assert_eq!(answer.missing, ["IMG_0001.jpg", "IMG_0002.jpg"]);
    assert_eq!(answer.already_here, 0);

    app.db.cleanup().await;
}

#[tokio::test]
async fn the_second_backup_asks_for_nothing_it_already_has() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    app.upload(&library, &format!("{folder}/IMG_0001.jpg"), b"first photo")
        .await;

    // The same roll offered again, plus one taken since.
    let answer = check(
        &app,
        &library,
        &device,
        json!([
            { "name": "IMG_0001.jpg", "size_bytes": 11 },
            { "name": "IMG_0002.jpg", "size_bytes": 34 },
        ]),
    )
    .await;

    assert_eq!(
        answer.missing,
        ["IMG_0002.jpg"],
        "a photograph already here was offered again"
    );
    assert_eq!(answer.already_here, 1);

    app.db.cleanup().await;
}

#[tokio::test]
async fn a_photograph_you_deleted_does_not_come_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    let uploaded = app
        .upload(&library, &format!("{folder}/IMG_0001.jpg"), b"regrettable")
        .await;
    let item = uploaded.json()["id"].as_str().expect("id").to_owned();

    // Into the trash it goes.
    let trashed = app.delete(&format!("/api/v1/items/{item}")).await;
    assert!(
        trashed.status.is_success(),
        "could not trash it: {:?}",
        trashed.json()
    );

    // The phone still holds it, so the next backup offers it again. The
    // server must remember that this one was deliberately thrown away:
    // a backup that resurrects what you deleted is worse than one that
    // misses something.
    let answer = check(
        &app,
        &library,
        &device,
        json!([{ "name": "IMG_0001.jpg", "size_bytes": 11 }]),
    )
    .await;

    assert_eq!(
        answer.missing,
        Vec::<String>::new(),
        "a photograph that was deleted was asked for again"
    );

    app.db.cleanup().await;
}

#[tokio::test]
async fn a_different_photograph_of_the_same_name_is_still_sent() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    app.upload(&library, &format!("{folder}/IMG_0001.jpg"), b"one")
        .await;

    // Camera rolls reuse names after a reset. Same name, different
    // size — a different picture, and skipping it would lose it.
    let answer = check(
        &app,
        &library,
        &device,
        json!([{ "name": "IMG_0001.jpg", "size_bytes": 999 }]),
    )
    .await;

    assert_eq!(answer.missing, ["IMG_0001.jpg"]);

    app.db.cleanup().await;
}

#[tokio::test]
async fn registering_the_same_phone_twice_continues_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;

    let (first, _) = phone(&app, &library, "Ada's phone").await;
    // Typed slightly differently next month; still the same phone.
    let (second, _) = phone(&app, &library, "ada's PHONE").await;

    assert_eq!(
        first, second,
        "backing up from the same phone made a second device beside it"
    );

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/backup/devices"))
        .await;
    assert_eq!(listed.json().as_array().expect("devices").len(), 1);

    app.db.cleanup().await;
}

#[tokio::test]
async fn a_device_name_cannot_put_photographs_outside_its_folder() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;

    for hostile in ["../../etc", "a/b", "..", ".hidden"] {
        let refused = app
            .post_json(
                &format!("/api/v1/libraries/{library}/backup/devices"),
                json!({ "name": hostile }),
            )
            .await;

        assert_eq!(
            refused.status,
            StatusCode::BAD_REQUEST,
            "accepted the device name {hostile:?}"
        );
    }

    app.db.cleanup().await;
}

#[tokio::test]
async fn finishing_records_when_the_backup_ran() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    let before = app
        .get(&format!("/api/v1/libraries/{library}/backup/devices"))
        .await;
    assert!(
        before.json()[0]["last_backup_at"].is_null(),
        "a phone that has never backed up should say so, not claim a date"
    );

    app.upload(&library, &format!("{folder}/IMG_0001.jpg"), b"first photo")
        .await;

    let finished = app
        .post_json(
            &format!("/api/v1/libraries/{library}/backup/devices/{device}/finish"),
            json!({ "sent": 1 }),
        )
        .await;

    assert_eq!(finished.status, StatusCode::OK);
    let body = finished.json();
    assert!(body["last_backup_at"].is_string());
    assert_eq!(
        body["photo_count"].as_i64(),
        Some(1),
        "the count is read from the folder, so it should see the upload"
    );

    app.db.cleanup().await;
}

#[tokio::test]
async fn forgetting_a_phone_keeps_its_photographs() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, folder) = phone(&app, &library, "Ada's phone").await;

    app.upload(&library, &format!("{folder}/IMG_0001.jpg"), b"a picture")
        .await;

    let forgotten = app
        .delete(&format!(
            "/api/v1/libraries/{library}/backup/devices/{device}"
        ))
        .await;
    assert_eq!(forgotten.status, StatusCode::NO_CONTENT);

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/backup/devices"))
        .await;
    assert!(listed.json().as_array().expect("devices").is_empty());

    // Selling a phone is not a request to delete years of pictures.
    // The folder has a space and an apostrophe in it, as a real phone
    // name does, so the query has to be encoded the way the browser
    // encodes it.
    let encoded =
        percent_encoding::utf8_percent_encode(&folder, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    let still_there = app.find_item(&library, &encoded, "IMG_0001.jpg").await;
    assert_eq!(
        still_there["name"], "IMG_0001.jpg",
        "forgetting the device took its photographs with it"
    );

    app.db.cleanup().await;
}

/// Brings a second person into the library. The harness's cookie jar
/// then holds that person's session.
async fn join_as(app: &TestApp, library: &str, name: &str) -> TestResponse {
    let invitation = app
        .post_json(
            &format!("/api/v1/libraries/{library}/invitations"),
            json!({}),
        )
        .await;
    let token = invitation.json()["token"]
        .as_str()
        .expect("token")
        .to_owned();

    app.forget_session();
    app.post_json(
        &format!("/api/v1/invitations/{token}/accept"),
        json!({ "display_name": name, "password": "a perfectly good passphrase" }),
    )
    .await
}

#[tokio::test]
async fn another_members_phone_is_not_reachable() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library(&app).await;
    let (device, _) = phone(&app, &library, "Ada's phone").await;

    // Someone else in the same library, with their own session.
    join_as(&app, &library, "Bram").await;

    let refused = app
        .post_json(
            &format!("/api/v1/libraries/{library}/backup/devices/{device}/check"),
            json!({ "files": [] }),
        )
        .await;

    // Not found rather than forbidden: which phones somebody backs up
    // is not something another member should be able to discover.
    assert_eq!(refused.status, StatusCode::NOT_FOUND);

    app.db.cleanup().await;
}
