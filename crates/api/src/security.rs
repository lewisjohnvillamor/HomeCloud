//! HTTP security baseline.
//!
//! The API serves JSON to its own web app and nothing else. The defaults
//! here say exactly that: no framing, no sniffing, no cross-origin
//! reads, no referrers, and no unbounded request bodies.

use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;

/// Largest accepted body for metadata routes. File transfer will get its
/// own routes with their own, much larger, bounded limits.
pub const MAX_METADATA_BODY_BYTES: usize = 64 * 1024;

/// Response headers applied to every API response.
///
/// The API returns JSON, never HTML, so the content policy forbids
/// loading anything at all: if a response is ever rendered as a document,
/// nothing in it can execute.
fn security_headers() -> [(HeaderName, HeaderValue); 7] {
    [
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ),
        (
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; sandbox"),
        ),
        (
            HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-origin"),
        ),
        (
            HeaderName::from_static("cross-origin-opener-policy"),
            HeaderValue::from_static("same-origin"),
        ),
        (
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), interest-cohort=()",
            ),
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
    ]
}

/// Adds the baseline headers and refuses cross-origin state changes.
///
/// No CORS headers are emitted anywhere, so a browser will not expose an
/// API response to another origin. This middleware closes the remaining
/// gap: requests that a browser sends cross-origin without a preflight
/// (simple form posts) are rejected before they reach a handler.
pub async fn security_middleware(request: Request, next: Next) -> Response {
    if let Err(error) = check_origin(&request) {
        return error.into_response();
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in security_headers() {
        headers.insert(name, value);
    }

    response
}

/// A state-changing request carrying an `Origin` that is not this server
/// is rejected. Requests without an `Origin` (non-browser clients) and
/// safe methods are unaffected.
fn check_origin(request: &Request) -> Result<(), ApiError> {
    let is_safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if is_safe {
        return Ok(());
    }

    let Some(origin) = request.headers().get(header::ORIGIN) else {
        return Ok(());
    };

    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|value| value.split("://").nth(1));

    match (origin_host, host) {
        (Some(origin_host), Some(host)) if origin_host == host => Ok(()),
        _ => {
            tracing::warn!("rejected a cross-origin state-changing request");
            Err(ApiError::forbidden(
                "Cross-origin requests are not accepted by this server.",
            ))
        }
    }
}
