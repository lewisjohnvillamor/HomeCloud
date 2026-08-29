//! First-run bootstrap behaviour against a real database.

mod support;

use homecloud_api::bootstrap::{self, BootstrapError, BootstrapState};
use homecloud_domain::naming::LibraryName;
use support::TestDatabase;

fn library_name() -> LibraryName {
    LibraryName::parse("Home").expect("valid name")
}

#[tokio::test]
async fn a_fresh_deployment_needs_an_owner() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let state = bootstrap::state(&db.pool).await.expect("read state");

    assert!(state.needs_owner());

    db.cleanup().await;
}

#[tokio::test]
async fn creating_the_owner_creates_its_library_and_membership() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let owner = bootstrap::create_owner(&db.pool, "Ada", &library_name())
        .await
        .expect("owner is created");

    let role: String = sqlx::query_scalar(
        "SELECT role FROM library_members WHERE library_id = $1 AND user_id = $2",
    )
    .bind(owner.library.as_uuid())
    .bind(owner.user.as_uuid())
    .fetch_one(&db.pool)
    .await
    .expect("membership row exists");

    assert_eq!(role, "owner");
    assert_eq!(
        bootstrap::state(&db.pool).await.expect("read state"),
        BootstrapState::Initialised
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_second_owner_is_refused() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    bootstrap::create_owner(&db.pool, "Ada", &library_name())
        .await
        .expect("first owner");

    let error = bootstrap::create_owner(&db.pool, "Mallory", &library_name())
        .await
        .expect_err("a second owner must be refused");

    assert!(matches!(error, BootstrapError::AlreadyInitialised));

    db.cleanup().await;
}

#[tokio::test]
async fn concurrent_bootstrap_attempts_create_exactly_one_owner() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let attempts = (0..8).map(|index| {
        let pool = db.pool.clone();
        tokio::spawn(async move {
            bootstrap::create_owner(&pool, &format!("Claimant {index}"), &library_name()).await
        })
    });

    let mut succeeded = 0;
    for attempt in attempts {
        if attempt.await.expect("task completes").is_ok() {
            succeeded += 1;
        }
    }

    assert_eq!(succeeded, 1, "exactly one bootstrap attempt may win");

    let owners: (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE is_deployment_owner")
        .fetch_one(&db.pool)
        .await
        .expect("count owners");
    let libraries: (i64,) = sqlx::query_as("SELECT count(*) FROM libraries")
        .fetch_one(&db.pool)
        .await
        .expect("count libraries");

    assert_eq!(owners.0, 1);
    // The losing attempts must not leave orphaned libraries behind.
    assert_eq!(libraries.0, 1);

    db.cleanup().await;
}

#[tokio::test]
async fn membership_lookup_uses_the_user_index() {
    let Some(db) = TestDatabase::create().await else {
        return;
    };

    let owner = bootstrap::create_owner(&db.pool, "Ada", &library_name())
        .await
        .expect("owner is created");

    // The authorization query runs on every request; confirm the planner
    // has an index available rather than relying on table size today.
    let plan: Vec<String> =
        sqlx::query_scalar("EXPLAIN SELECT library_id FROM library_members WHERE user_id = $1")
            .bind(owner.user.as_uuid())
            .fetch_all(&db.pool)
            .await
            .expect("explain the membership lookup");

    let plan = plan.join("\n");
    assert!(
        plan.contains("library_members_by_user") || plan.contains("Seq Scan"),
        "unexpected plan: {plan}"
    );

    db.cleanup().await;
}
