//! Curating a library: favorites and albums.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

/// Uploads a file and returns its id.
async fn upload(app: &TestApp, library: &str, name: &str) -> String {
    let uploaded = app.upload(library, name, b"bytes").await;
    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.text());

    uploaded.json()["id"].as_str().expect("id").to_owned()
}

async fn create_album(app: &TestApp, library: &str, name: &str) -> String {
    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/albums"),
            json!({ "name": name }),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());

    created.json()["id"].as_str().expect("id").to_owned()
}

async fn put(app: &TestApp, path: &str) -> support::TestResponse {
    app.send(
        axum::http::Request::builder()
            .method("PUT")
            .uri(path)
            .body(axum::body::Body::empty())
            .expect("valid request"),
    )
    .await
}

#[tokio::test]
async fn a_favorite_is_remembered_and_can_be_taken_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "beach.png").await;

    assert_eq!(
        put(&app, &format!("/api/v1/items/{item}/favorite"))
            .await
            .status,
        StatusCode::OK
    );

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/favorites"))
        .await;
    assert_eq!(listed.json()[0]["name"], "beach.png");

    assert_eq!(
        app.delete(&format!("/api/v1/items/{item}/favorite"))
            .await
            .status,
        StatusCode::OK
    );

    let after = app
        .get(&format!("/api/v1/libraries/{library}/favorites"))
        .await;
    assert_eq!(after.json().as_array().expect("a list").len(), 0);
}

#[tokio::test]
async fn starring_twice_is_the_same_as_starring_once() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "beach.png").await;

    // A client that retries must not end up with two of them.
    put(&app, &format!("/api/v1/items/{item}/favorite")).await;
    let second = put(&app, &format!("/api/v1/items/{item}/favorite")).await;

    assert_eq!(second.status, StatusCode::OK);

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/favorites"))
        .await;
    assert_eq!(listed.json().as_array().expect("a list").len(), 1);
}

#[tokio::test]
async fn a_trashed_photo_leaves_the_favorites_list() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "beach.png").await;
    put(&app, &format!("/api/v1/items/{item}/favorite")).await;

    app.delete(&format!("/api/v1/items/{item}")).await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/favorites"))
        .await;
    assert_eq!(listed.json().as_array().expect("a list").len(), 0);
}

#[tokio::test]
async fn favorites_belong_to_one_person_not_to_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "beach.png").await;
    put(&app, &format!("/api/v1/items/{item}/favorite")).await;

    // Someone else joins the same library.
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
    let joined = app
        .post_json(
            &format!("/api/v1/invitations/{token}/accept"),
            json!({ "display_name": "Grace", "password": "another long passphrase" }),
        )
        .await;
    assert_eq!(joined.status, StatusCode::OK, "{}", joined.text());

    // They can see the library, but not what someone else starred.
    let theirs = app
        .get(&format!("/api/v1/libraries/{library}/favorites"))
        .await;
    assert_eq!(theirs.status, StatusCode::OK);
    assert_eq!(theirs.json().as_array().expect("a list").len(), 0);
}

#[tokio::test]
async fn an_album_holds_pictures_in_the_order_they_were_added() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let first = upload(&app, &library, "one.png").await;
    let second = upload(&app, &library, "two.png").await;
    let third = upload(&app, &library, "three.png").await;

    let album = create_album(&app, &library, "Wales, summer 2019").await;

    let added = app
        .post_json(
            &format!("/api/v1/albums/{album}/items"),
            json!({ "items": [third, first, second] }),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.text());
    assert_eq!(added.json()["added"], 3);

    let contents = app.get(&format!("/api/v1/albums/{album}")).await;
    let names: Vec<String> = contents.json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();

    // An album is an arrangement, not a filter: the order is the point.
    assert_eq!(names, vec!["three.png", "one.png", "two.png"]);
    assert_eq!(contents.json()["album"]["name"], "Wales, summer 2019");
    assert_eq!(contents.json()["album"]["item_count"], 3);
}

#[tokio::test]
async fn adding_the_same_picture_twice_does_not_duplicate_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "one.png").await;
    let album = create_album(&app, &library, "Trip").await;

    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [item.clone()] }),
    )
    .await;
    let again = app
        .post_json(
            &format!("/api/v1/albums/{album}/items"),
            json!({ "items": [item] }),
        )
        .await;

    assert_eq!(again.json()["added"], 0);

    let contents = app.get(&format!("/api/v1/albums/{album}")).await;
    assert_eq!(contents.json()["items"].as_array().expect("items").len(), 1);
}

#[tokio::test]
async fn a_picture_can_be_taken_out_of_an_album_without_being_deleted() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "one.png").await;
    let album = create_album(&app, &library, "Trip").await;
    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [item.clone()] }),
    )
    .await;

    assert_eq!(
        app.delete(&format!("/api/v1/albums/{album}/items/{item}"))
            .await
            .status,
        StatusCode::OK
    );

    let contents = app.get(&format!("/api/v1/albums/{album}")).await;
    assert_eq!(contents.json()["items"].as_array().expect("items").len(), 0);

    // The file itself is untouched.
    assert_eq!(
        app.get(&format!("/api/v1/items/{item}")).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn deleting_an_album_keeps_every_picture_in_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "one.png").await;
    let album = create_album(&app, &library, "Trip").await;
    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [item.clone()] }),
    )
    .await;

    assert_eq!(
        app.delete(&format!("/api/v1/albums/{album}")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        app.get(&format!("/api/v1/albums/{album}")).await.status,
        StatusCode::NOT_FOUND
    );

    // An album is a way of looking at a library, so losing one must not
    // lose anything else.
    assert_eq!(
        app.get(&format!("/api/v1/items/{item}")).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn an_album_can_be_renamed_but_not_to_nothing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let album = create_album(&app, &library, "Trip").await;

    let renamed = app
        .patch_json(
            &format!("/api/v1/albums/{album}"),
            json!({ "name": "Wales" }),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.text());

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/albums"))
        .await;
    assert_eq!(listed.json()[0]["name"], "Wales");

    let empty = app
        .patch_json(&format!("/api/v1/albums/{album}"), json!({ "name": "   " }))
        .await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_album_lists_a_cover_and_a_count_without_being_opened() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let first = upload(&app, &library, "one.png").await;
    let second = upload(&app, &library, "two.png").await;
    let album = create_album(&app, &library, "Trip").await;
    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [first.clone(), second] }),
    )
    .await;

    let listed = app
        .get(&format!("/api/v1/libraries/{library}/albums"))
        .await;

    assert_eq!(listed.json()[0]["item_count"], 2);
    assert_eq!(listed.json()[0]["cover_item_id"], first);
}

#[tokio::test]
async fn an_album_cannot_be_pointed_at_another_librarys_pictures() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let album = create_album(&app, &library, "Trip").await;

    let elsewhere = uuid::Uuid::new_v4().to_string();
    let refused = app
        .post_json(
            &format!("/api/v1/albums/{album}/items"),
            json!({ "items": [elsewhere] }),
        )
        .await;

    assert_eq!(refused.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn albums_are_invisible_to_someone_outside_the_library() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let album = create_album(&app, &library, "Trip").await;

    app.forget_session();

    // Not "forbidden": whether an album exists is something only the
    // library's own members should learn.
    assert_eq!(
        app.get(&format!("/api/v1/albums/{album}")).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/albums"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_trashed_picture_disappears_from_the_albums_it_was_in() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let item = upload(&app, &library, "one.png").await;
    let album = create_album(&app, &library, "Trip").await;
    app.post_json(
        &format!("/api/v1/albums/{album}/items"),
        json!({ "items": [item.clone()] }),
    )
    .await;

    app.delete(&format!("/api/v1/items/{item}")).await;

    let contents = app.get(&format!("/api/v1/albums/{album}")).await;
    assert_eq!(contents.json()["items"].as_array().expect("items").len(), 0);

    // Restoring puts it back where it was, because the album points at
    // the item rather than holding a copy of it.
    app.post_json(&format!("/api/v1/items/{item}/restore"), json!({}))
        .await;

    let after = app.get(&format!("/api/v1/albums/{album}")).await;
    assert_eq!(after.json()["items"].as_array().expect("items").len(), 1);
}
