//! Catalog, transfers, and file operations end to end through the API.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestApp;

/// Signs up the owner and returns the library id.
async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

#[tokio::test]
async fn a_scan_indexes_what_is_on_disk() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.put_on_disk("notes.txt", b"hello");
    app.put_on_disk("photos/2026/beach.jpg", b"jpeg-bytes");
    app.scan(&library).await;

    let root = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    let names: Vec<String> = root.json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();

    // Folders first, then files, each by name.
    assert_eq!(names, vec!["photos", "notes.txt"]);

    let nested = app
        .get(&format!(
            "/api/v1/libraries/{library}/browse?path=photos/2026"
        ))
        .await;
    assert_eq!(nested.json()["items"][0]["name"], "beach.jpg");
    assert_eq!(nested.json()["items"][0]["content_type"], "image/jpeg");
    assert_eq!(nested.json()["items"][0]["is_image"], true);
    assert_eq!(nested.json()["breadcrumb"][0]["name"], "photos");
    assert_eq!(nested.json()["breadcrumb"][1]["path"], "photos/2026");

    app.cleanup().await;
}

#[tokio::test]
async fn rescanning_keeps_item_ids_stable() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("notes.txt", b"hello");
    app.scan(&library).await;

    let before = app.find_item(&library, "", "notes.txt").await;
    app.scan(&library).await;
    let after = app.find_item(&library, "", "notes.txt").await;

    assert_eq!(before["id"], after["id"]);

    app.cleanup().await;
}

#[tokio::test]
async fn a_file_removed_outside_the_app_stops_being_listed_but_is_not_forgotten() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("notes.txt", b"hello");
    app.scan(&library).await;
    let item = app.find_item(&library, "", "notes.txt").await;

    std::fs::remove_file(app.root_path().join("notes.txt")).expect("remove file");
    app.scan(&library).await;

    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    assert_eq!(listing.json()["items"].as_array().expect("items").len(), 0);

    // The row survives, so anything attached to the item survives a
    // disconnected drive.
    let still_known: (i64,) = sqlx::query_as("SELECT count(*) FROM items WHERE id = $1")
        .bind(uuid::Uuid::parse_str(item["id"].as_str().expect("id")).expect("uuid"))
        .fetch_one(&app.db.pool)
        .await
        .expect("query");
    assert_eq!(still_known.0, 1);

    app.cleanup().await;
}

#[tokio::test]
async fn the_scan_ignores_homeclouds_own_directories() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.make_dir_on_disk(".homecloud-trash");
    app.put_on_disk(".homecloud-trash/old.txt", b"trashed");
    app.scan(&library).await;

    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;

    assert_eq!(listing.json()["items"].as_array().expect("items").len(), 0);

    app.cleanup().await;
}

#[tokio::test]
async fn an_uploaded_file_can_be_downloaded_again() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let uploaded = app
        .upload(&library, "report.txt", b"quarterly numbers")
        .await;
    assert_eq!(uploaded.status, StatusCode::OK);
    assert_eq!(uploaded.json()["name"], "report.txt");

    let id = uploaded.json()["id"].as_str().expect("id").to_owned();
    let downloaded = app.get(&format!("/api/v1/items/{id}/content")).await;

    assert_eq!(downloaded.status, StatusCode::OK);
    assert_eq!(downloaded.text(), "quarterly numbers");
    assert_eq!(
        downloaded
            .headers
            .get(axum::http::header::ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok()),
        Some("bytes")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn uploading_the_same_name_twice_keeps_both_files() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.upload(&library, "report.txt", b"first").await;
    let second = app.upload(&library, "report.txt", b"second").await;

    assert_eq!(second.json()["name"], "report (2).txt");

    let id = second.json()["id"].as_str().expect("id").to_owned();
    assert_eq!(
        app.get(&format!("/api/v1/items/{id}/content")).await.text(),
        "second"
    );
    // The original is untouched.
    let first = app.find_item(&library, "", "report.txt").await;
    let first_id = first["id"].as_str().expect("id");
    assert_eq!(
        app.get(&format!("/api/v1/items/{first_id}/content"))
            .await
            .text(),
        "first"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_download_serves_a_byte_range() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "numbers.txt", b"0123456789").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app
        .send(
            axum::http::Request::builder()
                .uri(format!("/api/v1/items/{id}/content"))
                .header(axum::http::header::RANGE, "bytes=2-5")
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await;

    assert_eq!(response.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.text(), "2345");
    assert_eq!(
        response
            .headers
            .get(axum::http::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok()),
        Some("bytes 2-5/10")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn an_unsatisfiable_range_reports_the_real_size() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "numbers.txt", b"0123456789").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app
        .send(
            axum::http::Request::builder()
                .uri(format!("/api/v1/items/{id}/content"))
                .header(axum::http::header::RANGE, "bytes=99-200")
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await;

    assert_eq!(response.status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        response
            .headers
            .get(axum::http::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok()),
        Some("bytes */10")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_dangerous_content_type_is_never_served_back_as_itself() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "page.html", b"<script>alert(1)</script>")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app.get(&format!("/api/v1/items/{id}/content")).await;

    assert_eq!(
        response
            .headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/octet-stream")
    );
    let disposition = response
        .headers
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .expect("a disposition is set");
    assert!(disposition.starts_with("attachment"), "{disposition}");

    app.cleanup().await;
}

#[tokio::test]
async fn folders_can_be_created_and_files_moved_into_them() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "report.txt", b"contents").await;

    let folder = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({"path": "Documents"}),
        )
        .await;
    assert_eq!(folder.status, StatusCode::OK);
    assert_eq!(folder.json()["kind"], "folder");

    let file = app.find_item(&library, "", "report.txt").await;
    let id = file["id"].as_str().expect("id").to_owned();
    let moved = app
        .post_json(
            &format!("/api/v1/items/{id}/move"),
            json!({"path": "Documents/report.txt"}),
        )
        .await;

    assert_eq!(moved.status, StatusCode::OK);
    assert_eq!(moved.json()["path"], "Documents/report.txt");
    // Identity survives the move.
    assert_eq!(moved.json()["id"], id);
    assert!(app.root_path().join("Documents/report.txt").exists());

    app.cleanup().await;
}

#[tokio::test]
async fn moving_a_folder_rewrites_the_paths_of_everything_inside_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("trip/day1/beach.jpg", b"jpeg");
    app.scan(&library).await;

    let folder = app.find_item(&library, "", "trip").await;
    let id = folder["id"].as_str().expect("id").to_owned();
    app.post_json(
        &format!("/api/v1/items/{id}/move"),
        json!({"path": "Holiday"}),
    )
    .await;

    let inner = app
        .get(&format!(
            "/api/v1/libraries/{library}/browse?path=Holiday/day1"
        ))
        .await;

    assert_eq!(inner.json()["items"][0]["name"], "beach.jpg");
    assert_eq!(inner.json()["items"][0]["path"], "Holiday/day1/beach.jpg");

    app.cleanup().await;
}

#[tokio::test]
async fn deleting_moves_a_file_to_the_trash_and_it_can_come_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "report.txt", b"contents").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let trashed = app.delete(&format!("/api/v1/items/{id}")).await;
    assert_eq!(trashed.status, StatusCode::OK);
    assert_eq!(trashed.json()["trashed"], true);

    // Gone from the listing, present in the trash, still on disk.
    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    assert_eq!(listing.json()["items"].as_array().expect("items").len(), 0);
    let trash = app.get(&format!("/api/v1/libraries/{library}/trash")).await;
    assert_eq!(trash.json()[0]["id"], id);
    assert!(!app.root_path().join("report.txt").exists());
    let trash_dir = app.root_path().join(".homecloud-trash");
    assert_eq!(
        std::fs::read_dir(&trash_dir).expect("trash dir").count(),
        1,
        "the file should still exist in the trash directory"
    );

    let restored = app
        .post_json(&format!("/api/v1/items/{id}/restore"), json!({}))
        .await;
    assert_eq!(restored.status, StatusCode::OK);
    assert_eq!(restored.json()["trashed"], false);
    assert!(app.root_path().join("report.txt").exists());

    app.cleanup().await;
}

#[tokio::test]
async fn search_finds_files_by_name() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("invoices/march-generator.pdf", b"pdf");
    app.put_on_disk("invoices/april-lamp.pdf", b"pdf");
    app.scan(&library).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    let names: Vec<String> = response
        .json()
        .as_array()
        .expect("results")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, vec!["march-generator.pdf"]);

    app.cleanup().await;
}

#[tokio::test]
async fn search_handles_hostile_input_without_failing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("notes.txt", b"hello");
    app.scan(&library).await;

    for hostile in ["'; DROP TABLE items; --", "%", "&|!()", "   "] {
        let encoded = hostile
            .replace(' ', "%20")
            .replace('&', "%26")
            .replace('#', "%23");
        let response = app
            .get(&format!("/api/v1/libraries/{library}/search?q={encoded}"))
            .await;

        assert_eq!(response.status, StatusCode::OK, "failed on `{hostile}`");
    }

    // The table is still there.
    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    assert_eq!(listing.json()["items"].as_array().expect("items").len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn photos_lists_only_images() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("beach.jpg", b"jpeg");
    app.put_on_disk("notes.txt", b"hello");
    app.scan(&library).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;

    let names: Vec<String> = response
        .json()
        .as_array()
        .expect("results")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, vec!["beach.jpg"]);

    app.cleanup().await;
}

// --- Authorization ---

#[tokio::test]
async fn every_library_route_requires_a_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "report.txt", b"secret").await;
    let item = app.find_item(&library, "", "report.txt").await;
    let id = item["id"].as_str().expect("id").to_owned();

    app.forget_session();

    for path in [
        "/api/v1/libraries".to_owned(),
        format!("/api/v1/libraries/{library}/browse"),
        format!("/api/v1/libraries/{library}/photos"),
        format!("/api/v1/libraries/{library}/search?q=x"),
        format!("/api/v1/libraries/{library}/trash"),
        format!("/api/v1/libraries/{library}/scan"),
        format!("/api/v1/items/{id}"),
        format!("/api/v1/items/{id}/children"),
        format!("/api/v1/items/{id}/content"),
    ] {
        let response = app.get(&path).await;

        assert_eq!(
            response.status,
            StatusCode::UNAUTHORIZED,
            "`{path}` answered an anonymous caller"
        );
    }

    app.cleanup().await;
}

#[tokio::test]
async fn a_member_of_another_library_cannot_reach_this_one() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "private.txt", b"secret").await;
    let item = app.find_item(&library, "", "private.txt").await;
    let item_id = item["id"].as_str().expect("id").to_owned();

    // A second account with its own library, created the way an admin
    // flow eventually will.
    let intruder: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (display_name, password_hash) VALUES ('Mallory', 'x') RETURNING id",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("create user");
    let other_library: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO libraries (name, root_path) VALUES ('Other', '/tmp') RETURNING id",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("create library");
    sqlx::query("INSERT INTO library_members (library_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(other_library)
        .bind(intruder)
        .execute(&app.db.pool)
        .await
        .expect("add membership");
    let token = homecloud_auth::session::create(
        &app.db.pool,
        homecloud_domain::identity::UserId::from_uuid(intruder),
    )
    .await
    .expect("session");

    app.forget_session();

    async fn as_intruder(app: &TestApp, token: &str, path: String) -> support::TestResponse {
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

    let token = token.expose();
    let browse = as_intruder(&app, token, format!("/api/v1/libraries/{library}/browse")).await;
    let read_item = as_intruder(&app, token, format!("/api/v1/items/{item_id}")).await;
    let read_content = as_intruder(&app, token, format!("/api/v1/items/{item_id}/content")).await;
    let own_libraries = as_intruder(&app, token, "/api/v1/libraries".to_owned()).await;

    // Not "forbidden": whether the library exists is itself private.
    assert_eq!(browse.status, StatusCode::NOT_FOUND);
    assert_eq!(read_item.status, StatusCode::NOT_FOUND);
    assert_eq!(read_content.status, StatusCode::NOT_FOUND);
    assert_eq!(
        own_libraries.json().as_array().expect("libraries").len(),
        1,
        "the intruder should only see their own library"
    );
    assert_eq!(own_libraries.json()[0]["name"], "Other");

    app.cleanup().await;
}

#[tokio::test]
async fn homeclouds_own_directories_cannot_be_written_to() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let upload = app
        .upload(&library, ".homecloud-trash/sneaky.txt", b"x")
        .await;
    let folder = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({"path": ".homecloud-incoming/sneaky"}),
        )
        .await;

    assert_eq!(upload.status, StatusCode::BAD_REQUEST);
    assert_eq!(folder.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn traversal_in_an_upload_path_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let response = app.upload(&library, "../escaped.txt", b"x").await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(!app
        .root_path()
        .parent()
        .expect("parent")
        .join("escaped.txt")
        .exists());

    app.cleanup().await;
}

#[tokio::test]
async fn a_deep_folder_path_records_its_ancestors() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let created = app
        .post_json(
            &format!("/api/v1/libraries/{library}/folders"),
            json!({"path": "Documents/2026/Taxes"}),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK);

    // Each level is browsable, which only works if every ancestor was
    // catalogued with the right parent.
    for (folder, expected) in [
        ("", "Documents"),
        ("Documents", "2026"),
        ("Documents/2026", "Taxes"),
    ] {
        let item = app.find_item(&library, folder, expected).await;
        assert_eq!(item["kind"], "folder");
    }

    app.cleanup().await;
}

#[tokio::test]
async fn a_trashed_folder_takes_its_contents_with_it_and_brings_them_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.put_on_disk("trip/day1/beach.jpg", b"jpeg");
    app.scan(&library).await;

    let folder = app.find_item(&library, "", "trip").await;
    let id = folder["id"].as_str().expect("id").to_owned();

    app.delete(&format!("/api/v1/items/{id}")).await;

    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    assert_eq!(listing.json()["items"].as_array().expect("items").len(), 0);
    assert!(!app.root_path().join("trip").exists());
    // The nested file is trashed too, not left as a live orphan.
    let nested_live: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM items WHERE relative_path = 'trip/day1/beach.jpg' AND trashed_at IS NULL",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("query");
    assert_eq!(nested_live.0, 0);

    app.post_json(&format!("/api/v1/items/{id}/restore"), json!({}))
        .await;

    assert!(app.root_path().join("trip/day1/beach.jpg").exists());
    let restored = app
        .get(&format!(
            "/api/v1/libraries/{library}/browse?path=trip/day1"
        ))
        .await;
    assert_eq!(restored.json()["items"][0]["name"], "beach.jpg");

    app.cleanup().await;
}

#[tokio::test]
async fn simultaneous_uploads_of_one_name_both_survive() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // Sequential at the API, but they race for the same destination the
    // way two browser tabs would.
    let first = app.upload(&library, "report.txt", b"first").await;
    let second = app.upload(&library, "report.txt", b"second").await;
    let third = app.upload(&library, "report.txt", b"third").await;

    for response in [&first, &second, &third] {
        assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    }

    let names: Vec<String> = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await
        .json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();

    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names.contains(&"report.txt".to_owned()), "{names:?}");

    app.cleanup().await;
}

#[tokio::test]
async fn a_file_uploaded_during_a_scan_is_not_marked_missing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // Start a scan and upload into the library while it runs. The scan
    // cannot have seen the new file, but it must not conclude it vanished.
    app.post_json(&format!("/api/v1/libraries/{library}/scan"), json!({}))
        .await;
    let uploaded = app.upload(&library, "during-scan.txt", b"contents").await;
    assert_eq!(uploaded.status, StatusCode::OK);

    app.scan(&library).await;

    let item = app.find_item(&library, "", "during-scan.txt").await;
    assert_eq!(item["name"], "during-scan.txt");

    app.cleanup().await;
}

// --- Thumbnails ---

/// A small but genuinely valid PNG, so the decoder has real work to do.
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    use std::io::Cursor;

    let mut buffer = image::RgbImage::new(width, height);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]);
    }

    let mut output = Vec::new();
    image::DynamicImage::ImageRgb8(buffer)
        .write_to(&mut Cursor::new(&mut output), image::ImageFormat::Png)
        .expect("encode test image");

    output
}

#[tokio::test]
async fn a_photo_has_a_thumbnail_that_is_smaller_than_the_original() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let original = png_bytes(1200, 900);
    let uploaded = app.upload(&library, "beach.png", &original).await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response
            .headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/jpeg")
    );
    assert!(
        response.body.len() < original.len(),
        "thumbnail {} bytes vs original {} bytes",
        response.body.len(),
        original.len()
    );

    let decoded = image::load_from_memory(&response.body).expect("a readable image");
    assert_eq!(decoded.width(), 320);

    app.cleanup().await;
}

#[tokio::test]
async fn each_thumbnail_size_is_served() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "beach.png", &png_bytes(2000, 2000))
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    for (size, edge) in [("small", 320), ("medium", 640), ("large", 1280)] {
        let response = app
            .get(&format!("/api/v1/items/{id}/thumbnail?size={size}"))
            .await;

        assert_eq!(response.status, StatusCode::OK, "{size}");
        let decoded = image::load_from_memory(&response.body).expect("a readable image");
        assert_eq!(decoded.width(), edge, "{size}");
    }

    app.cleanup().await;
}

#[tokio::test]
async fn thumbnails_are_cached_and_kept_out_of_the_library_listing() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "beach.png", &png_bytes(800, 600))
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let first = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;
    let second = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(
        first.body, second.body,
        "the cached copy should be identical"
    );
    assert!(
        app.root_path().join(".homecloud-derivatives").exists(),
        "the derivative cache should live inside the library root"
    );

    // Cacheable by the browser, but never in a shared cache.
    let cache_control = second
        .headers
        .get(axum::http::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .expect("a cache policy is set");
    assert!(cache_control.contains("private"), "{cache_control}");
    assert!(!cache_control.contains("no-store"), "{cache_control}");

    // A scan must not index the cache as if it were the user's content.
    app.scan(&library).await;
    let listing = app
        .get(&format!("/api/v1/libraries/{library}/browse"))
        .await;
    let names: Vec<String> = listing.json()["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, vec!["beach.png"]);

    app.cleanup().await;
}

#[tokio::test]
async fn a_file_that_is_not_a_picture_has_no_thumbnail() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app.upload(&library, "notes.txt", b"hello").await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn a_file_that_only_claims_to_be_a_picture_is_refused_cleanly() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    // A shell script named `.png`: the catalog believes the extension,
    // the decoder does not.
    let uploaded = app
        .upload(&library, "trojan.png", b"#!/bin/sh\nrm -rf /\n")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["code"], "bad_request");

    app.cleanup().await;
}

#[tokio::test]
async fn a_thumbnail_needs_a_session_and_membership() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "beach.png", &png_bytes(400, 300))
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    app.forget_session();
    let response = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn an_unknown_thumbnail_size_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "beach.png", &png_bytes(400, 300))
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app
        .get(&format!("/api/v1/items/{id}/thumbnail?size=enormous"))
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

// --- Memories ---

#[tokio::test]
async fn memories_collect_photos_from_this_day_in_earlier_years() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.upload(&library, "old.png", &png_bytes(200, 150)).await;
    app.upload(&library, "recent.png", &png_bytes(200, 150))
        .await;

    // Backdate one photo to this day two years ago, the way a camera
    // timestamp would.
    sqlx::query("UPDATE items SET modified_at = now() - interval '2 years' WHERE name = 'old.png'")
        .execute(&app.db.pool)
        .await
        .expect("backdate the photo");

    let response = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let groups = response.json();
    let on_this_day = groups
        .as_array()
        .expect("groups")
        .iter()
        .find(|group| group["title"] == "On this day")
        .unwrap_or_else(|| panic!("no `On this day` group: {}", response.text()));

    let names: Vec<String> = on_this_day["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        names,
        vec!["old.png"],
        "today's own photos are not memories"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn memories_are_empty_rather_than_invented_when_there_is_nothing_to_show() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.json().as_array().expect("groups").is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn memories_need_a_session() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.forget_session();

    let response = app
        .get(&format!("/api/v1/libraries/{library}/memories"))
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

// --- Video posters ---

/// Renders a short video with FFmpeg, or `None` when it is not installed.
fn make_video(directory: &std::path::Path, name: &str) -> Option<Vec<u8>> {
    let path = directory.join(name);

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=10:duration=1",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .status()
        .ok()?;

    if !status.success() {
        eprintln!("skipping video test: ffmpeg is not usable here");
        return None;
    }

    std::fs::read(&path).ok()
}

#[tokio::test]
async fn a_video_gets_a_poster_frame_for_a_thumbnail() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let temp = tempfile::TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "clip.mp4") else {
        app.cleanup().await;
        return;
    };

    let uploaded = app.upload(&library, "holiday.mp4", &video).await;
    assert_eq!(uploaded.status, StatusCode::OK, "{}", uploaded.text());
    assert_eq!(uploaded.json()["is_video"], true);
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let poster = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    assert_eq!(poster.status, StatusCode::OK, "{}", poster.text());
    assert_eq!(
        poster
            .headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/jpeg")
    );
    let decoded = image::load_from_memory(&poster.body).expect("a readable poster");
    assert_eq!(decoded.width(), 320);

    // Cached like any other derivative, so the second request costs
    // nothing.
    let again = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;
    assert_eq!(again.body, poster.body);

    app.cleanup().await;
}

#[tokio::test]
async fn videos_appear_in_the_photo_timeline() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let temp = tempfile::TempDir::new().expect("temp dir");
    let Some(video) = make_video(temp.path(), "clip.mp4") else {
        app.cleanup().await;
        return;
    };

    app.upload(&library, "holiday.mp4", &video).await;
    app.upload(&library, "beach.png", &png_bytes(200, 150))
        .await;
    app.upload(&library, "notes.txt", b"not media").await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/photos"))
        .await;

    let mut names: Vec<String> = response
        .json()
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["beach.png", "holiday.mp4"]);

    app.cleanup().await;
}

#[tokio::test]
async fn a_file_that_only_claims_to_be_a_video_is_refused_cleanly() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    let uploaded = app
        .upload(&library, "trojan.mp4", b"#!/bin/sh\nrm -rf /\n")
        .await;
    let id = uploaded.json()["id"].as_str().expect("id").to_owned();

    let response = app.get(&format!("/api/v1/items/{id}/thumbnail")).await;

    // A clear client error, and no host detail in the message.
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(!response.text().contains("/tmp"), "{}", response.text());
    assert!(!response.text().contains("ffmpeg"), "{}", response.text());

    app.cleanup().await;
}
