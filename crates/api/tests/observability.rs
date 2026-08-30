//! Request correlation behaviour.

mod support;

use axum::body::Body;
use axum::routing::get;
use axum::Router;
use homecloud_api::observability::{request_id_middleware, REQUEST_ID_HEADER};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// A router with only the middleware under test, plus one route that
/// panics, so the correlation behaviour is observed in isolation.
fn instrumented() -> Router {
    Router::new()
        .route("/ok", get(|| async { "ok" }))
        .route("/panic", get(panicking_handler))
        .fallback(|| async { homecloud_api::error::ApiError::not_found() })
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(
            homecloud_api::observability::panic_response,
        ))
        .layer(axum::middleware::from_fn(request_id_middleware))
}

/// Declared return type keeps the panic from making the handler's type
/// depend on never-type fallback.
async fn panicking_handler() -> &'static str {
    panic!("handler panicked with secret detail")
}

async fn send(path: &str, request_id: Option<&str>) -> (StatusCode, String, Vec<u8>) {
    let mut builder = Request::builder().uri(path);
    if let Some(id) = request_id {
        builder = builder.header(REQUEST_ID_HEADER, id);
    }

    let response = instrumented()
        .oneshot(builder.body(Body::empty()).expect("valid request"))
        .await
        .expect("router responds");

    let status = response.status();
    let echoed = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .expect("every response carries a request id")
        .to_str()
        .expect("request id is printable ASCII")
        .to_owned();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes()
        .to_vec();

    (status, echoed, body)
}

#[tokio::test]
async fn a_request_id_is_generated_when_none_is_supplied() {
    let (status, request_id, _) = send("/ok", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(!request_id.is_empty());
}

#[tokio::test]
async fn generated_request_ids_are_unique_per_request() {
    let (_, first, _) = send("/ok", None).await;
    let (_, second, _) = send("/ok", None).await;

    assert_ne!(first, second);
}

#[tokio::test]
async fn a_well_formed_inbound_request_id_is_reused() {
    let (_, request_id, _) = send("/ok", Some("trace-0123456789")).await;

    assert_eq!(request_id, "trace-0123456789");
}

#[tokio::test]
async fn malformed_inbound_request_ids_are_replaced_not_echoed() {
    let hostile = [
        "id with spaces",
        "injected\tvalue",
        "<script>alert(1)</script>",
        &"a".repeat(65),
    ];

    for candidate in hostile {
        let (status, request_id, _) = send("/ok", Some(candidate)).await;

        assert_eq!(status, StatusCode::OK);
        assert_ne!(request_id, candidate, "untrusted id was echoed verbatim");
    }
}

#[tokio::test]
async fn error_bodies_carry_the_request_id() {
    let (status, header_id, body) = send("/missing", Some("corr-1234")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(&body).expect("problem body is JSON");
    assert_eq!(body["request_id"], "corr-1234");
    assert_eq!(header_id, "corr-1234");
}

#[tokio::test]
async fn a_panicking_handler_returns_a_problem_without_leaking_details() {
    let (status, request_id, body) = send("/panic", None).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!request_id.is_empty());

    let rendered = String::from_utf8(body).expect("body is UTF-8");
    assert!(
        !rendered.contains("secret detail"),
        "panic leaked: {rendered}"
    );
    let body: Value = serde_json::from_str(&rendered).expect("problem body is JSON");
    assert_eq!(body["code"], "internal");
}
