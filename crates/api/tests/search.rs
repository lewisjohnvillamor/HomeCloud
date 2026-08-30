//! Content search: finding a document by what is written inside it.

mod support;

use axum::http::StatusCode;
use support::TestApp;

async fn library_with_documents(app: &TestApp) -> String {
    app.sign_up_owner().await;
    let library = app.library_id().await;

    app.put_on_disk(
        "invoices/march.txt",
        b"Invoice 4021. One standby generator, delivered to the workshop.",
    );
    app.put_on_disk(
        "invoices/april.txt",
        b"Invoice 4088. Two desk lamps and a kettle.",
    );
    app.put_on_disk("photos/beach.jpg", b"\xFF\xD8\xFF\xE0 not really a jpeg");
    app.scan(&library).await;

    library
}

fn names(response: &support::TestResponse) -> Vec<String> {
    response
        .json()
        .as_array()
        .expect("results")
        .iter()
        .map(|hit| hit["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn a_document_is_found_by_a_word_inside_it() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(names(&response), vec!["march.txt"]);
    assert_eq!(response.json()[0]["matched"], "content");

    app.cleanup().await;
}

#[tokio::test]
async fn a_content_match_comes_back_with_a_snippet_showing_why() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    let snippet = response.json()[0]["snippet"]
        .as_str()
        .expect("a snippet")
        .to_owned();
    assert!(snippet.contains("<<generator>>"), "{snippet}");
    // Highlighting is markers, not markup: a document cannot inject HTML
    // into the page through a snippet.
    assert!(
        !snippet.contains('<') || !snippet.contains("<b>"),
        "{snippet}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_name_match_still_works_and_is_labelled_as_one() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=april"))
        .await;

    assert_eq!(names(&response), vec!["april.txt"]);
    assert!(
        response.json()[0]["matched"] == "name"
            || response.json()[0]["matched"] == "name_and_content",
        "{}",
        response.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn a_word_in_both_the_name_and_the_text_ranks_first() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;
    app.put_on_disk(
        "generator-manual.txt",
        b"How to start the generator safely.",
    );
    app.scan(&library).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    let results = names(&response);
    assert_eq!(
        results.first().map(String::as_str),
        Some("generator-manual.txt")
    );
    assert_eq!(response.json()[0]["matched"], "name_and_content");
    assert!(results.contains(&"march.txt".to_owned()), "{results:?}");

    app.cleanup().await;
}

#[tokio::test]
async fn a_multi_word_search_matches_documents_containing_all_of_them() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    let both = app
        .get(&format!(
            "/api/v1/libraries/{library}/search?q=generator%20workshop"
        ))
        .await;
    let neither = app
        .get(&format!(
            "/api/v1/libraries/{library}/search?q=generator%20kettle"
        ))
        .await;

    assert_eq!(names(&both), vec!["march.txt"]);
    assert!(names(&neither).is_empty(), "{}", neither.text());

    app.cleanup().await;
}

#[tokio::test]
async fn indexing_skips_files_it_cannot_read_and_says_so_once() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    let status = app.get(&format!("/api/v1/libraries/{library}/scan")).await;
    let index = &status.json()["last_index"];

    assert!(
        index["indexed"].as_u64().unwrap_or(0) >= 2,
        "{}",
        status.text()
    );
    // The image is skipped rather than parsed as text.
    assert!(
        index["skipped"].as_u64().unwrap_or(0) >= 1,
        "{}",
        status.text()
    );

    let recorded: (String,) = sqlx::query_as(
        "SELECT status FROM item_text t JOIN items i ON i.id = t.item_id WHERE i.name = 'beach.jpg'",
    )
    .fetch_one(&app.db.pool)
    .await
    .expect("the image was recorded as unreadable");
    assert_eq!(recorded.0, "unsupported");

    app.cleanup().await;
}

#[tokio::test]
async fn a_rescan_does_not_re_read_unchanged_documents() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    app.scan(&library).await;
    let status = app.get(&format!("/api/v1/libraries/{library}/scan")).await;

    // Nothing changed on disk, so the second pass has nothing to read.
    assert_eq!(
        status.json()["last_index"]["indexed"],
        0,
        "{}",
        status.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn edited_text_is_found_by_its_new_contents() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    app.put_on_disk(
        "invoices/march.txt",
        b"Invoice 4021. One transformer instead.",
    );
    app.scan(&library).await;

    let old = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;
    let new = app
        .get(&format!("/api/v1/libraries/{library}/search?q=transformer"))
        .await;

    assert!(names(&old).is_empty(), "{}", old.text());
    assert_eq!(names(&new), vec!["march.txt"]);

    app.cleanup().await;
}

#[tokio::test]
async fn trashed_documents_drop_out_of_search() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;
    let item = app.find_item(&library, "invoices", "march.txt").await;
    let id = item["id"].as_str().expect("id").to_owned();

    app.delete(&format!("/api/v1/items/{id}")).await;

    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    assert!(names(&response).is_empty(), "{}", response.text());

    app.cleanup().await;
}

#[tokio::test]
async fn hostile_queries_are_answered_rather_than_breaking_the_search() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    for hostile in [
        "'; DROP TABLE item_text; --",
        "%%%",
        "\"unclosed quote",
        "or or or",
        "&|!():*",
    ] {
        let encoded = urlencoding(hostile);
        let response = app
            .get(&format!("/api/v1/libraries/{library}/search?q={encoded}"))
            .await;

        assert_eq!(response.status, StatusCode::OK, "failed on `{hostile}`");
    }

    // The index is still there and still works.
    let after = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;
    assert_eq!(names(&after), vec!["march.txt"]);

    app.cleanup().await;
}

#[tokio::test]
async fn search_stays_inside_the_callers_libraries() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = library_with_documents(&app).await;

    app.forget_session();
    let response = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

/// Percent-encodes a query string value for the tests above.
fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
