//! The private AI switch: off by default, owner-only, and honest about
//! what the machine can do.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{TestApp, TestResponse};

async fn signed_in_library(app: &TestApp) -> String {
    app.sign_up_owner().await;
    app.library_id().await
}

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

#[tokio::test]
async fn ai_is_off_until_someone_turns_it_on() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let settings = app.get(&format!("/api/v1/libraries/{library}/ai")).await;

    assert_eq!(settings.status, StatusCode::OK, "{}", settings.text());
    assert_eq!(settings.json()["profile"], "off");
    assert_eq!(settings.json()["pending_items"], 0);
}

#[tokio::test]
async fn the_switch_reports_what_the_machine_can_do_not_what_was_asked_for() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let settings = app.get(&format!("/api/v1/libraries/{library}/ai")).await;

    // Whatever this machine has, the two answers are separate fields.
    // A deployment without the tool must be able to say so rather than
    // accept a setting and quietly do nothing.
    assert!(settings.json()["ocr_available"].is_boolean());
    assert!(settings.json()["supported_profile"].is_string());
}

#[tokio::test]
async fn a_profile_can_be_chosen_and_taken_back() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    let on = set_profile(&app, &library, "text").await;
    assert_eq!(on.status, StatusCode::OK, "{}", on.text());
    assert_eq!(on.json()["profile"], "text");

    let read_back = app.get(&format!("/api/v1/libraries/{library}/ai")).await;
    assert_eq!(read_back.json()["profile"], "text");

    let off = set_profile(&app, &library, "off").await;
    assert_eq!(off.json()["profile"], "off");
}

#[tokio::test]
async fn a_nonsense_profile_is_refused() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    for candidate in ["", "on", "maximum", "OFF"] {
        let refused = set_profile(&app, &library, candidate).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{candidate:?}");
    }
}

#[tokio::test]
async fn only_the_owner_can_turn_it_on() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

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
        json!({ "display_name": "Grace", "password": "another long passphrase" }),
    )
    .await;

    // A member can see the setting — it describes the library they are
    // in — but committing the machine to the work is the owner's call.
    let seen = app.get(&format!("/api/v1/libraries/{library}/ai")).await;
    assert_eq!(seen.status, StatusCode::OK, "{}", seen.text());

    let refused = set_profile(&app, &library, "text").await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.text());
}

#[tokio::test]
async fn a_stranger_sees_nothing_at_all() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;
    app.forget_session();

    assert_eq!(
        app.get(&format!("/api/v1/libraries/{library}/ai"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        set_profile(&app, &library, "text").await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn turning_it_off_removes_what_it_wrote_and_nothing_else() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    // A real document, whose text is read straight out of the file.
    app.upload(&library, "notes.txt", b"a standby generator invoice")
        .await;
    app.scan(&library).await;

    // Something AI wrote, stood in for directly: this test is about
    // deletion, not about recognition.
    let item: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM items WHERE library_id = $1 AND name = 'notes.txt'")
            .bind(uuid::Uuid::parse_str(&library).expect("a library id"))
            .fetch_one(&app.db.pool)
            .await
            .expect("the uploaded file");

    sqlx::query(
        "INSERT INTO item_text (item_id, library_id, content, status, source, source_size)
         VALUES ($1, $2, 'words from a photograph', 'indexed', 'ocr', 10)
         ON CONFLICT (item_id) DO UPDATE SET content = 'words from a photograph', source = 'ocr'",
    )
    .bind(item)
    .bind(uuid::Uuid::parse_str(&library).expect("a library id"))
    .execute(&app.db.pool)
    .await
    .expect("stand in for what AI wrote");

    set_profile(&app, &library, "text").await;
    set_profile(&app, &library, "off").await;

    // Everything AI wrote is derived: dropping it costs a rescan and
    // nothing else, and leaving it after someone said no would be the
    // wrong answer to the only question they asked.
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM item_text WHERE library_id = $1 AND source = 'ocr'",
    )
    .bind(uuid::Uuid::parse_str(&library).expect("a library id"))
    .fetch_one(&app.db.pool)
    .await
    .expect("count");

    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn search_keeps_working_with_ai_off() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    let library = signed_in_library(&app).await;

    app.upload(&library, "4021.txt", b"Invoice for one standby generator")
        .await;
    app.scan(&library).await;

    // The exit gate for this whole phase: with nothing enabled, the
    // deterministic half of the product is untouched.
    let found = app
        .get(&format!("/api/v1/libraries/{library}/search?q=generator"))
        .await;

    assert_eq!(found.status, StatusCode::OK, "{}", found.text());
    assert_eq!(found.json()[0]["name"], "4021.txt");
}

/// Renders a scan of some text, the way the provider tests do.
async fn scan_image(path: &std::path::Path, text: &str) -> bool {
    const FONTS: [&str; 3] = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
    ];

    let Some(font) = FONTS
        .iter()
        .find(|candidate| std::path::Path::new(candidate).is_file())
    else {
        return false;
    };

    tokio::process::Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error"])
        .args(["-f", "lavfi", "-i", "color=white:s=900x200"])
        .arg("-vf")
        .arg(format!(
            "drawtext=fontfile={font}:text='{text}':fontcolor=black:fontsize=72:x=40:y=60"
        ))
        .args(["-frames:v", "1"])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[tokio::test]
async fn with_ai_on_a_scan_reads_the_words_in_a_picture() {
    let Some(app) = TestApp::create().await else {
        return;
    };
    if !homecloud_ai::ocr::is_available().await {
        eprintln!("skipping: tesseract is not installed");
        return;
    }

    let library = signed_in_library(&app).await;

    // A photograph of a receipt: nothing in its name says what it is,
    // which is the whole reason to read the picture.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let source = directory.path().join("IMG_4821.png");
    if !scan_image(&source, "ARBORETUM 77341").await {
        eprintln!("skipping: no ffmpeg or font to render the fixture with");
        return;
    }
    let bytes = std::fs::read(&source).expect("the rendered image");

    app.upload(&library, "IMG_4821.png", &bytes).await;

    // With AI off, a scan does not read it, and the word is not findable.
    app.scan(&library).await;
    let before = app
        .get(&format!("/api/v1/libraries/{library}/search?q=arboretum"))
        .await;
    assert_eq!(
        before.json().as_array().expect("results").len(),
        0,
        "text was read with the feature off: {}",
        before.text()
    );

    // Turned on, the next scan reads it and search finds the picture by
    // a word that appears nowhere in its name.
    set_profile(&app, &library, "text").await;
    app.scan(&library).await;

    let after = app
        .get(&format!("/api/v1/libraries/{library}/search?q=arboretum"))
        .await;
    assert_eq!(after.status, StatusCode::OK, "{}", after.text());
    assert_eq!(after.json()[0]["name"], "IMG_4821.png", "{}", after.text());

    // And turning it off again takes the words back out of the index.
    set_profile(&app, &library, "off").await;
    let removed = app
        .get(&format!("/api/v1/libraries/{library}/search?q=arboretum"))
        .await;
    assert_eq!(removed.json().as_array().expect("results").len(), 0);
}
