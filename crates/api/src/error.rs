//! The API's single error surface.
//!
//! Every failure a client can observe is rendered as the same stable JSON
//! shape, modelled on RFC 9457 "problem details". Internal detail —
//! driver errors, paths, SQL, stack context — is logged, never returned.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Stable machine-readable error codes. Clients may branch on these; they
/// are part of the API contract and must not be renamed casually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthenticated,
    PasswordRequired,
    Forbidden,
    Conflict,
    TooManyRequests,
    NotFound,
    PayloadTooLarge,
    DependencyUnavailable,
    Internal,
}

impl ErrorCode {
    fn status(self) -> StatusCode {
        match self {
            ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthenticated => StatusCode::UNAUTHORIZED,
            ErrorCode::PasswordRequired => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// An error as the client sees it.
#[derive(Debug, Clone)]
pub struct ApiError {
    code: ErrorCode,
    /// Safe, human-readable summary. Never interpolates untrusted input
    /// or internal identifiers beyond a fixed vocabulary.
    detail: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn dependency_unavailable(dependency: &'static str) -> Self {
        Self::new(
            ErrorCode::DependencyUnavailable,
            format!("The {dependency} is not available. Retry shortly."),
        )
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, detail)
    }

    /// No usable session. The web app treats this as "show the sign-in
    /// screen", never as an unexpected failure.
    pub fn unauthenticated() -> Self {
        Self::new(ErrorCode::Unauthenticated, "Sign in to continue.")
    }

    /// Credentials were supplied and rejected.
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthenticated, detail)
    }

    /// A protected share link that has not been unlocked yet. Distinct
    /// from "sign in", because the visitor has no account to sign in to.
    pub fn password_required() -> Self {
        Self::new(
            ErrorCode::PasswordRequired,
            "This link is password protected.",
        )
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, detail)
    }

    pub fn too_many_requests(retry_after: std::time::Duration) -> Self {
        let seconds = retry_after.as_secs().max(1);

        Self::new(
            ErrorCode::TooManyRequests,
            format!("Too many attempts. Try again in {seconds} seconds."),
        )
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, detail)
    }

    pub fn not_found() -> Self {
        Self::new(
            ErrorCode::NotFound,
            "The requested resource does not exist.",
        )
    }

    pub fn internal() -> Self {
        Self::new(
            ErrorCode::Internal,
            "The server could not complete the request.",
        )
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn status(&self) -> StatusCode {
        self.code.status()
    }

    /// Renders the problem document. The request id is attached by
    /// middleware, which is the only layer that knows it.
    pub fn to_response(&self, request_id: Option<&str>) -> Response {
        let body = ProblemBody {
            code: self.code,
            detail: self.detail.clone(),
            request_id: request_id.map(str::to_owned),
        };

        let mut response = (self.status(), Json(body)).into_response();
        // RFC 9457 media type, so a client can tell a problem document
        // from a successful JSON payload without inspecting the body.
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

/// Wire format. `request_id` lets a user quote one identifier that a
/// server operator can find in the logs.
#[derive(Debug, Serialize)]
struct ProblemBody {
    code: ErrorCode,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = self.to_response(None);
        // Middleware re-renders the body once the request id is known.
        response.extensions_mut().insert(self);
        response
    }
}
