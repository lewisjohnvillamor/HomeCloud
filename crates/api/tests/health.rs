//! HTTP-level behaviour of the health endpoints.

mod support;

use axum::body::Body;
use homecloud_api::app::{router, AppState};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use support::TestDatabase;

async fn get(app: axum::Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router responds");

    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("responses are JSON");

    (status, body)
}

#[tokio::test]
async fn liveness_succeeds_without_a_database() {
    // Deliberately points at a port nothing listens on: liveness must not
    // depend on the database being reachable.
    let config =
        homecloud_api::config::ServerConfig::from_source(&std::collections::HashMap::from([(
            homecloud_api::config::vars::DATABASE_URL.to_owned(),
            "postgres://homecloud@127.0.0.1:1/homecloud".to_owned(),
        )]))
        .expect("valid config");
    let pool = homecloud_api::db::connect(&config.database)
        .await
        .expect("lazy pool");

    let (status, body) = get(
        router(AppState::new(pool, std::path::PathBuf::from("."), false)),
        "/health/live",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn readiness_fails_when_the_database_is_unreachable() {
    let config =
        homecloud_api::config::ServerConfig::from_source(&std::collections::HashMap::from([(
            homecloud_api::config::vars::DATABASE_URL.to_owned(),
            "postgres://homecloud:hunter2@127.0.0.1:1/homecloud".to_owned(),
        )]))
        .expect("valid config");
    let pool = homecloud_api::db::connect(&config.database)
        .await
        .expect("lazy pool");

    let (status, body) = get(
        router(AppState::new(pool, std::path::PathBuf::from("."), false)),
        "/health/ready",
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "dependency_unavailable");

    // The failure must not disclose credentials, hosts, ports, or driver text.
    let rendered = body.to_string();
    for leak in ["hunter2", "127.0.0.1", "postgres", "Connection refused"] {
        assert!(
            !rendered.contains(leak),
            "response leaked `{leak}`: {rendered}"
        );
    }
}

#[tokio::test]
async fn readiness_succeeds_against_a_live_database() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let (status, body) = get(
        router(AppState::new(
            db.pool.clone(),
            std::path::PathBuf::from("."),
            false,
        )),
        "/health/ready",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");

    db.cleanup().await;
}

#[tokio::test]
async fn unknown_routes_return_the_problem_shape() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let (status, body) = get(
        router(AppState::new(
            db.pool.clone(),
            std::path::PathBuf::from("."),
            false,
        )),
        "/does-not-exist",
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
    assert!(body["detail"].is_string());

    db.cleanup().await;
}

#[tokio::test]
async fn problems_use_the_problem_json_media_type() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let response = router(AppState::new(
        db.pool.clone(),
        std::path::PathBuf::from("."),
        false,
    ))
    .oneshot(
        Request::builder()
            .uri("/does-not-exist")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    assert_eq!(
        response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );

    db.cleanup().await;
}

#[tokio::test]
async fn bootstrap_status_reports_that_an_owner_is_needed() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let (status, body) = get(
        router(AppState::new(
            db.pool.clone(),
            std::path::PathBuf::from("."),
            false,
        )),
        "/api/v1/bootstrap",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["needs_owner"], true);

    db.cleanup().await;
}

#[tokio::test]
async fn bootstrap_status_is_unavailable_without_a_database() {
    let config =
        homecloud_api::config::ServerConfig::from_source(&std::collections::HashMap::from([(
            homecloud_api::config::vars::DATABASE_URL.to_owned(),
            "postgres://homecloud@127.0.0.1:1/homecloud".to_owned(),
        )]))
        .expect("valid config");
    let pool = homecloud_api::db::connect(&config.database)
        .await
        .expect("lazy pool");

    let (status, body) = get(
        router(AppState::new(pool, std::path::PathBuf::from("."), false)),
        "/api/v1/bootstrap",
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "dependency_unavailable");
}
