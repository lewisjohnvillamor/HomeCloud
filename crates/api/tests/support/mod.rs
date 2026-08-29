//! Shared harness for tests that need a real PostgreSQL database.
//!
//! Each test gets its own database created from the configured server so
//! tests never observe each other's writes. When no database is
//! configured the harness reports it and the test skips, so a
//! contributor without PostgreSQL still gets a useful `cargo test` run
//! while CI (which always sets the variable) enforces the real thing.

#![allow(dead_code)]

use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

/// Connection string of a PostgreSQL server that tests may create and
/// drop databases on.
pub const DATABASE_URL_VAR: &str = "DATABASE_URL";

/// An isolated database that is dropped when the test finishes.
pub struct TestDatabase {
    pub pool: PgPool,
    admin_url: String,
    name: String,
}

impl TestDatabase {
    /// Creates a fresh database and applies all migrations to it.
    /// Returns `None` when no PostgreSQL server is configured.
    pub async fn create() -> Option<Self> {
        let admin_url = match std::env::var(DATABASE_URL_VAR) {
            Ok(url) if !url.trim().is_empty() => url,
            _ => {
                eprintln!(
                    "skipping database test: set {DATABASE_URL_VAR} to a PostgreSQL server to run it"
                );
                return None;
            }
        };

        let name = format!("homecloud_test_{}", Uuid::new_v4().simple());

        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("connect to the configured PostgreSQL server");
        admin
            .execute(format!(r#"CREATE DATABASE "{name}""#).as_str())
            .await
            .expect("create isolated test database");
        admin.close().await.ok();

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url_for(&admin_url, &name))
            .await
            .expect("connect to isolated test database");

        homecloud_api::db::run_migrations(&pool)
            .await
            .expect("migrations apply to a clean database");

        Some(Self {
            pool,
            admin_url,
            name,
        })
    }

    pub fn url(&self) -> String {
        database_url_for(&self.admin_url, &self.name)
    }

    /// Drops the database. Called explicitly because `Drop` cannot await.
    pub async fn cleanup(self) {
        self.pool.close().await;

        if let Ok(mut admin) = PgConnection::connect(&self.admin_url).await {
            let _ = admin
                .execute(
                    format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name).as_str(),
                )
                .await;
            let _ = admin.close().await;
        }
    }
}

/// Rewrites the database name in a PostgreSQL connection string.
fn database_url_for(admin_url: &str, database: &str) -> String {
    let (base, query) = match admin_url.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (admin_url, None),
    };

    let trimmed = base.trim_end_matches('/');
    let without_database = match trimmed.rfind('/') {
        Some(index) if index > "postgres://".len() => &trimmed[..index],
        _ => trimmed,
    };

    match query {
        Some(query) => format!("{without_database}/{database}?{query}"),
        None => format!("{without_database}/{database}"),
    }
}

/// A running application backed by an isolated database and a temporary
/// library root, plus a client that remembers its session cookie.
pub struct TestApp {
    pub db: TestDatabase,
    pub root: tempfile::TempDir,
    // One state for the whole test, as in production: per-request state
    // would silently reset in-process counters such as login throttling.
    state: homecloud_api::app::AppState,
    cookie: std::sync::Mutex<Option<String>>,
}

/// One HTTP response, decoded far enough for assertions.
pub struct TestResponse {
    pub status: axum::http::StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    /// The body as JSON. Panics if it is not JSON, which in a test is
    /// the correct outcome.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!(
                "expected a JSON body, got `{}`: {error}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

impl TestApp {
    pub async fn create() -> Option<Self> {
        let db = TestDatabase::create().await?;
        let root = tempfile::TempDir::new().expect("temporary library root");
        let state =
            homecloud_api::app::AppState::new(db.pool.clone(), root.path().to_path_buf(), false);

        Some(Self {
            db,
            root,
            state,
            cookie: std::sync::Mutex::new(None),
        })
    }

    pub fn root_path(&self) -> &std::path::Path {
        self.root.path()
    }

    pub fn state(&self) -> homecloud_api::app::AppState {
        self.state.clone()
    }

    pub fn router(&self) -> axum::Router {
        homecloud_api::app::router(self.state())
    }

    /// Sends a request through the whole middleware stack, carrying the
    /// session cookie from any earlier sign-in.
    pub async fn send(&self, request: axum::http::Request<axum::body::Body>) -> TestResponse {
        use tower::ServiceExt;

        let mut request = request;
        if let Some(cookie) = self.cookie.lock().expect("cookie lock").clone() {
            request.headers_mut().insert(
                axum::http::header::COOKIE,
                axum::http::HeaderValue::from_str(&cookie).expect("valid cookie"),
            );
        }

        let response = self
            .router()
            .oneshot(request)
            .await
            .expect("router responds");

        let status = response.status();
        let headers = response.headers().clone();

        if let Some(set_cookie) = headers
            .get(axum::http::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
        {
            let pair = set_cookie.split(';').next().unwrap_or_default().to_owned();
            let cleared = pair.ends_with('=');
            *self.cookie.lock().expect("cookie lock") = (!cleared).then_some(pair);
        }

        let body = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024)
            .await
            .expect("read body")
            .to_vec();

        TestResponse {
            status,
            headers,
            body,
        }
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        self.send(
            axum::http::Request::builder()
                .uri(path)
                .body(axum::body::Body::empty())
                .expect("valid request"),
        )
        .await
    }

    pub async fn post_json(&self, path: &str, body: serde_json::Value) -> TestResponse {
        self.send(
            axum::http::Request::builder()
                .method("POST")
                .uri(path)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .expect("valid request"),
        )
        .await
    }

    /// Creates the owner account and signs in, the precondition for
    /// every authenticated test.
    pub async fn sign_up_owner(&self) -> TestResponse {
        self.post_json(
            "/api/v1/setup",
            serde_json::json!({
                "display_name": "Ada",
                "password": "correct horse battery staple",
                "library_name": "Home",
            }),
        )
        .await
    }

    /// Drops the remembered cookie without telling the server, which is
    /// what an unauthenticated caller looks like.
    pub fn forget_session(&self) {
        *self.cookie.lock().expect("cookie lock") = None;
    }

    pub async fn cleanup(self) {
        self.db.cleanup().await;
    }
}
