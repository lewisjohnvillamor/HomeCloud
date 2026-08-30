//! HTTP security baseline.
//!
//! The API serves JSON to its own web app and nothing else. The defaults
//! here say exactly that: no framing, no sniffing, no cross-origin
//! reads, no referrers, and no unbounded request bodies.

use axum::extract::{Request, State};
use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
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
pub async fn security_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Err(error) = check_origin(&request, state.origin_policy()) {
        return error.into_response();
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in security_headers() {
        // `Cache-Control` is the one baseline header a handler may
        // override: thumbnails are private but worth caching, and
        // `no-store` would make a photo grid re-fetch everything.
        if name == header::CACHE_CONTROL && headers.contains_key(&name) {
            continue;
        }

        headers.insert(name, value);
    }

    response
}

/// Which origins may make state-changing requests.
#[derive(Debug, Clone)]
pub struct OriginPolicy {
    /// Explicitly configured origins, for deployments behind a proxy
    /// that rewrites `Host`.
    pub trusted: Vec<String>,
    /// In development, loopback origins are accepted whatever port they
    /// are on, because the web dev server proxies from a different port
    /// than the API listens on. Production accepts no such exception.
    pub allow_loopback: bool,
}

impl OriginPolicy {
    fn permits(&self, origin: &str, host: Option<&str>) -> bool {
        let origin = origin.trim_end_matches('/');

        if self.trusted.iter().any(|trusted| trusted == origin) {
            return true;
        }

        let Some(origin_host) = origin.split("://").nth(1) else {
            return false;
        };

        if host.is_some_and(|host| host == origin_host) {
            return true;
        }

        self.allow_loopback && is_loopback(origin_host)
    }
}

/// Whether a host component names this machine.
fn is_loopback(host: &str) -> bool {
    let name = host.split(':').next().unwrap_or(host);

    matches!(name, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// A state-changing request carrying an `Origin` that is not this server
/// is rejected. Requests without an `Origin` (non-browser clients) and
/// safe methods are unaffected.
fn check_origin(request: &Request, policy: &OriginPolicy) -> Result<(), ApiError> {
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

    match origin.to_str() {
        Ok(origin) if policy.permits(origin, host) => Ok(()),
        _ => {
            tracing::warn!("rejected a cross-origin state-changing request");
            Err(ApiError::forbidden(
                "Cross-origin requests are not accepted by this server.",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(allow_loopback: bool, trusted: &[&str]) -> OriginPolicy {
        OriginPolicy {
            trusted: trusted.iter().map(|value| (*value).to_owned()).collect(),
            allow_loopback,
        }
    }

    #[test]
    fn the_servers_own_origin_is_permitted() {
        assert!(policy(false, &[]).permits("https://home.example", Some("home.example")));
    }

    #[test]
    fn another_origin_is_refused() {
        assert!(!policy(false, &[]).permits("https://evil.example", Some("home.example")));
    }

    #[test]
    fn a_configured_origin_is_permitted_behind_a_rewriting_proxy() {
        let policy = policy(false, &["https://home.example"]);

        assert!(policy.permits("https://home.example", Some("127.0.0.1:8080")));
        assert!(policy.permits("https://home.example/", Some("127.0.0.1:8080")));
        assert!(!policy.permits("https://evil.example", Some("127.0.0.1:8080")));
    }

    #[test]
    fn loopback_is_accepted_only_where_the_deployment_allows_it() {
        let development = policy(true, &[]);
        let production = policy(false, &[]);

        assert!(development.permits("http://127.0.0.1:3000", Some("127.0.0.1:8080")));
        assert!(development.permits("http://localhost:3000", Some("127.0.0.1:8080")));
        assert!(!development.permits("https://evil.example", Some("127.0.0.1:8080")));
        assert!(!production.permits("http://127.0.0.1:3000", Some("127.0.0.1:8080")));
    }
}
