//! HTTP security baseline.

mod support;

use axum::body::Body;
use homecloud_api::app::{router, AppState};
use homecloud_api::security::MAX_METADATA_BODY_BYTES;
use http::{header, Request, StatusCode};
use tower::ServiceExt;

use support::TestDatabase;

#[tokio::test]
async fn every_response_carries_the_baseline_headers() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let response = router(AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(std::path::PathBuf::from(".")),
    ))
    .oneshot(
        Request::builder()
            .uri("/health/live")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    let headers = response.headers();
    for (name, expected) in [
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cache-control", "no-store"),
    ] {
        assert_eq!(
            headers.get(name).and_then(|value| value.to_str().ok()),
            Some(expected),
            "missing or wrong `{name}`"
        );
    }

    let csp = headers
        .get("content-security-policy")
        .and_then(|value| value.to_str().ok())
        .expect("a content security policy is set");
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");

    db.cleanup().await;
}

#[tokio::test]
async fn no_cors_headers_are_exposed() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let response = router(AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(std::path::PathBuf::from(".")),
    ))
    .oneshot(
        Request::builder()
            .uri("/api/v1/bootstrap")
            .header(header::ORIGIN, "https://evil.example")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    // Without `Access-Control-Allow-Origin` a browser will not hand the
    // response body to another origin.
    assert!(response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    db.cleanup().await;
}

#[tokio::test]
async fn cross_origin_state_changing_requests_are_refused() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let response = router(AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(std::path::PathBuf::from(".")),
    ))
    .oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/bootstrap")
            .header(header::HOST, "homecloud.local")
            .header(header::ORIGIN, "https://evil.example")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    db.cleanup().await;
}

#[tokio::test]
async fn same_origin_state_changing_requests_reach_routing() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let response = router(AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(std::path::PathBuf::from(".")),
    ))
    .oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/bootstrap")
            .header(header::HOST, "homecloud.local")
            .header(header::ORIGIN, "https://homecloud.local")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    // No POST route exists yet; what matters is that the origin check
    // let the request through to routing instead of rejecting it.
    assert_ne!(response.status(), StatusCode::FORBIDDEN);

    db.cleanup().await;
}

#[tokio::test]
async fn oversized_request_bodies_are_rejected() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let oversized = vec![b'a'; MAX_METADATA_BODY_BYTES + 1];
    let response = router(AppState::new(
        db.pool.clone(),
        homecloud_api::app::AppSettings::development(std::path::PathBuf::from(".")),
    ))
    .oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/bootstrap")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, oversized.len())
            .body(Body::from(oversized))
            .expect("valid request"),
    )
    .await
    .expect("router responds");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    db.cleanup().await;
}
