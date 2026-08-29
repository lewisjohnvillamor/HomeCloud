//! Database harness behaviour: migrations must apply to a clean database,
//! be re-runnable, and leave the deployment row in a state the server can
//! rely on.

mod support;

use support::TestDatabase;

#[tokio::test]
async fn migrations_apply_to_a_clean_database() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let installed: (bool,) = sqlx::query_as("SELECT only_row FROM deployment")
        .fetch_one(&db.pool)
        .await
        .expect("deployment row exists after migration");

    assert!(installed.0);

    db.cleanup().await;
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    // Restarting a deployment must not reinitialise anything.
    homecloud_api::db::run_migrations(&db.pool)
        .await
        .expect("re-running migrations is a no-op");

    let deployments: (i64,) = sqlx::query_as("SELECT count(*) FROM deployment")
        .fetch_one(&db.pool)
        .await
        .expect("count deployment rows");

    assert_eq!(deployments.0, 1);

    db.cleanup().await;
}

#[tokio::test]
async fn a_second_deployment_row_is_rejected_by_the_database() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let result = sqlx::query("INSERT INTO deployment (only_row) VALUES (TRUE)")
        .execute(&db.pool)
        .await;

    assert!(result.is_err(), "the singleton constraint must be enforced");

    db.cleanup().await;
}

#[tokio::test]
async fn health_check_succeeds_against_a_live_database() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    homecloud_api::db::check_health(&db.pool)
        .await
        .expect("a live database is healthy");

    db.cleanup().await;
}

#[tokio::test]
async fn health_check_fails_when_the_database_is_unreachable() {
    let config =
        homecloud_api::config::ServerConfig::from_source(&std::collections::HashMap::from([(
            homecloud_api::config::vars::DATABASE_URL.to_owned(),
            // Port 1 is reserved and never serves PostgreSQL.
            "postgres://homecloud@127.0.0.1:1/homecloud".to_owned(),
        )]))
        .expect("valid config");

    let pool = homecloud_api::db::connect(&config.database)
        .await
        .expect("lazy pool creation does not connect");

    assert!(homecloud_api::db::check_health(&pool).await.is_err());
}
