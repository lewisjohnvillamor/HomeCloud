//! Request correlation and structured request logging.
//!
//! Every response carries a request id that a user can quote and an
//! operator can grep for. Logged fields are limited to routing metadata:
//! query strings, headers, and bodies can contain filenames, tokens, and
//! other user data, so they are not logged here.

use std::time::Instant;

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use crate::error::ApiError;

/// Header used to accept and return the correlation id.
pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Longest inbound id accepted. An unbounded value would end up in every
/// log line for the request.
const MAX_REQUEST_ID_LEN: usize = 64;

/// Correlation id for a single request, available to handlers through
/// request extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestId(String);

impl RequestId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Inbound ids come from the network and are echoed into logs and
/// responses, so they are accepted only in a conservative shape:
/// bounded length, ASCII alphanumerics, `-` and `_`. Anything else is
/// replaced with a generated id rather than rejected, because a proxy's
/// habits should not turn into user-visible errors.
fn accept_inbound(raw: &HeaderValue) -> Option<RequestId> {
    let value = raw.to_str().ok()?;

    let acceptable = !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');

    acceptable.then(|| RequestId(value.to_owned()))
}

/// Assigns a request id, records a structured span for the request, and
/// attaches the id to the response (including error bodies).
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(accept_inbound)
        .unwrap_or_else(RequestId::generate);

    let method = request.method().clone();
    // Path only: query strings can carry user data.
    let path = request.uri().path().to_owned();

    request.extensions_mut().insert(request_id.clone());

    let span = tracing::info_span!(
        "http_request",
        request_id = request_id.as_str(),
        method = %method,
        path = %path,
    );
    let _entered = span.enter();

    let started = Instant::now();
    let mut response = next.run(request).await;
    let duration_ms = started.elapsed().as_millis() as u64;

    // Errors are re-rendered here so the client receives the same id that
    // appears in the logs.
    if let Some(error) = response.extensions().get::<ApiError>().cloned() {
        response = error.to_response(Some(request_id.as_str()));
    }

    let status = response.status();
    if status.is_server_error() {
        tracing::error!(status = status.as_u16(), duration_ms, "request failed");
    } else {
        tracing::info!(status = status.as_u16(), duration_ms, "request completed");
    }

    if let Ok(header) = HeaderValue::from_str(request_id.as_str()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, header);
    }

    response
}

/// Turns a panic in a handler into the standard problem response. The
/// panic payload never reaches the client; the correlation layer above
/// re-renders this response with the request id.
pub fn panic_response(_panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    use axum::response::IntoResponse;

    ApiError::internal().into_response()
}
