//! Photo understanding: searching photographs for what is in them.
//!
//! Most of these stand labels in directly, because they are about what
//! the product does with a label — search it, isolate it, delete it —
//! not about recognition. The last runs the real model end to end when
//! one is configured, and skips with a note when it is not.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};
use uuid::Uuid;

async fn set_profile(app: &TestApp, library: &str, profile: &str) -> TestResponse {
    app.send(
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/libraries/{library}/ai"))
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                json!({ "profile": profile }).to_string(),
            ))
            .expect("valid request"),
    )
    .await
}

async fn item_id(app: &TestApp, library: &str, name: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM items WHERE library_id = $1 AND name = $2")
        .bind(Uuid::parse_str(library).expect("a library id"))
        .bind(name)
        .fetch_one(&app.db.pool)
        .await
        .expect("the item is catalogued")
}

/// Stands in for what the model would have written.
async fn label(app: &TestApp, library: &str, name: &str, labels: &[&str], words: &str) {
    let item = item_id(app, library, name).await;
    let confidences: Vec<f32> = labels.iter().map(|_| 0.9).collect();

    sqlx::query(
        "INSERT INTO item_objects
            (item_id, library_id, model, status, labels, confidences, search_vector, source_size)
         VALUES ($1, $2, 'test.onnx#000000000000', 'labelled', $3, $4,
                 to_tsvector('simple', $5), 1)",
    )
    .bind(item)
    .bind(Uuid::parse_str(library).expect("a library id"))
    .bind(labels)
    .bind(&confidences)
    .bind(words)
    .execute(&app.db.pool)
    .await
    .expect("stand in for what the model wrote");
}

async fn label_count(app: &TestApp, library: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM item_objects WHERE library_id = $1")
        .bind(Uuid::parse_str(library).expect("a library id"))
        .fetch_one(&app.db.pool)
        .await
        .expect("count")
}

async fn search(app: &TestApp, library: &str, term: &str) -> TestResponse {
    app.get(&format!("/api/v1/libraries/{library}/search?q={term}"))
        .await
}

fn names(response: &TestResponse) -> Vec<String> {
    response
        .json()
        .as_array()
        .expect("results")
        .iter()
        .map(|hit| hit["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// A library holding two photographs, one with a dog in it.
async fn library_with_photos(app: &TestApp) -> String {
    app.sign_up_owner().await;
    let library = app.library_id().await;

    app.put_on_disk("park/IMG_0001.jpg", b"\xFF\xD8\xFF\xE0 a picture");
    app.put_on_disk("park/IMG_0002.jpg", b"\xFF\xD8\xFF\xE0 another picture");
    app.scan(&library).await;

    label(
        app,
        &library,
        "IMG_0001.jpg",
        &["dog", "person"],
        "dog dogs puppy puppies person people persons human",
    )
    .await;

    library
}

#[tokio::test]
async fn a_server_without_a_model_says_so() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let settings = app.get(&format!("/api/v1/libraries/{library}/ai")).await;

    assert_eq!(settings.status, StatusCode::OK, "{}", settings.text());
    assert_eq!(settings.json()["objects_available"], false);
    assert_eq!(settings.json()["objects_pending"], 0);
    // Photos needs a model; without one, the most this server can honour
    // is never reported as photos.
    assert_ne!(settings.json()["supported_profile"], "photos");
}

#[tokio::test]
async fn a_photograph_is_found_by_what_is_in_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;

    let response = search(&app, &library, "dog").await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(names(&response), ["IMG_0001.jpg"]);
    // Reported as an inference, with what was recognised, so the
    // interface never presents it as something the file says.
    assert_eq!(response.json()[0]["matched"], "objects");
    assert_eq!(response.json()[0]["objects"], json!(["dog", "person"]));

    app.cleanup().await;
}

#[tokio::test]
async fn the_words_people_actually_type_find_it_too() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;

    for term in ["dogs", "puppy", "people"] {
        assert_eq!(
            names(&search(&app, &library, term).await),
            ["IMG_0001.jpg"],
            "{term}"
        );
    }
    assert!(names(&search(&app, &library, "cat").await).is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn a_name_match_outranks_something_recognised() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;
    app.put_on_disk("dog licence.txt", b"Renewal due in March.");
    app.scan(&library).await;

    let response = search(&app, &library, "dog").await;

    assert_eq!(names(&response), ["dog licence.txt", "IMG_0001.jpg"]);
    assert!(
        response.json()[0].get("objects").is_none(),
        "a name match carries no labels"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn labels_never_cross_from_one_library_to_another() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    library_with_photos(&app).await;

    // Another library on the same server, owned by the same person, with
    // nothing labelled in it.
    let other = Uuid::new_v4();
    let owner: Uuid = sqlx::query_scalar("SELECT user_id FROM library_members LIMIT 1")
        .fetch_one(&app.db.pool)
        .await
        .expect("the owner");
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Other', $2)")
        .bind(other)
        .bind(app.root_path().join("other").to_string_lossy().into_owned())
        .execute(&app.db.pool)
        .await
        .expect("a second library");
    sqlx::query("INSERT INTO library_members (library_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(other)
        .bind(owner)
        .execute(&app.db.pool)
        .await
        .expect("membership");

    let response = search(&app, &other.to_string(), "dog").await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert!(names(&response).is_empty(), "{}", response.text());

    app.cleanup().await;
}

#[tokio::test]
async fn stepping_down_from_photos_removes_every_label_and_no_photograph() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;

    set_profile(&app, &library, "photos").await;
    assert_eq!(label_count(&app, &library).await, 1);

    // Still reading text, no longer looking at what is in pictures: the
    // labels go, because that is what was turned off.
    let stepped_down = set_profile(&app, &library, "text").await;
    assert_eq!(
        stepped_down.status,
        StatusCode::OK,
        "{}",
        stepped_down.text()
    );
    assert_eq!(label_count(&app, &library).await, 0);

    assert!(names(&search(&app, &library, "dog").await).is_empty());
    assert!(app.root_path().join("park/IMG_0001.jpg").is_file());

    app.cleanup().await;
}

#[tokio::test]
async fn turning_ai_off_removes_labels_too() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;

    set_profile(&app, &library, "photos").await;
    set_profile(&app, &library, "off").await;

    assert_eq!(label_count(&app, &library).await, 0);

    app.cleanup().await;
}

#[tokio::test]
async fn asking_for_faces_keeps_the_labels() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;

    set_profile(&app, &library, "photos").await;
    set_profile(&app, &library, "people").await;

    assert_eq!(label_count(&app, &library).await, 1);

    app.cleanup().await;
}

#[tokio::test]
async fn a_deleted_photograph_takes_its_labels_with_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_photos(&app).await;
    let item = item_id(&app, &library, "IMG_0001.jpg").await;

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(item)
        .execute(&app.db.pool)
        .await
        .expect("delete the catalogue row");

    assert_eq!(label_count(&app, &library).await, 0);

    app.cleanup().await;
}

/// The real thing: a model, a photograph of a dog, a scan, a search.
///
/// Needs `HOMECLOUD_OBJECT_MODEL` and `HOMECLOUD_TEST_DOG_PHOTO`, and
/// `HOMECLOUD_ONNXRUNTIME` unless the runtime is installed where the
/// loader looks. No photograph is committed: none here has a licence
/// clear enough to ship.
#[tokio::test]
async fn a_scan_labels_a_photograph_and_search_finds_it() {
    let (Some(model), Some(photo)) = (
        std::env::var_os("HOMECLOUD_OBJECT_MODEL").map(PathBuf::from),
        std::env::var_os("HOMECLOUD_TEST_DOG_PHOTO").map(PathBuf::from),
    ) else {
        eprintln!("skipping: set HOMECLOUD_OBJECT_MODEL and HOMECLOUD_TEST_DOG_PHOTO to run this");
        return;
    };
    let runtime = std::env::var_os("HOMECLOUD_ONNXRUNTIME").map(PathBuf::from);

    let detector = tokio::task::spawn_blocking(move || {
        homecloud_ai::objects::ObjectDetector::load(runtime.as_deref(), &model)
    })
    .await
    .expect("loading does not panic")
    .expect("the configured model loads");
    let detector = Arc::new(detector);

    let Some(app) = TestApp::create_with(|settings| homecloud_api::app::AppSettings {
        objects: Some(detector),
        ..settings
    })
    .await
    else {
        return;
    };
    app.sign_up_owner().await;
    let library = app.library_id().await;

    let settings = set_profile(&app, &library, "photos").await;
    assert_eq!(settings.json()["objects_available"], true);

    app.put_on_disk(
        "camera/IMG_2041.jpg",
        &std::fs::read(photo).expect("the test photograph"),
    );
    app.scan(&library).await;

    let response = search(&app, &library, "puppy").await;
    assert_eq!(names(&response), ["IMG_2041.jpg"], "{}", response.text());
    assert_eq!(response.json()[0]["matched"], "objects");

    let after = app.get(&format!("/api/v1/libraries/{library}/ai")).await;
    assert_eq!(after.json()["objects_pending"], 0);

    app.cleanup().await;
}
