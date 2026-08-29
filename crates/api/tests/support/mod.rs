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
