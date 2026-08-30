#![forbid(unsafe_code)]

use std::process::ExitCode;

use homecloud_api::app::{router, AppState};
use homecloud_api::config::ServerConfig;
use homecloud_api::{db, SERVICE_NAME};
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,homecloud_api=debug".into()),
        )
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // `{error:#}` prints the source chain without a panic backtrace.
            tracing::error!(
                error = format!("{error:#}"),
                "{SERVICE_NAME} failed to start"
            );
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let config = ServerConfig::from_env()?;

    let pool = db::connect(&config.database).await?;
    db::run_migrations(&pool).await?;

    // Create the library root if it does not exist yet: the path is
    // explicit configuration, and a first run should not fail because an
    // empty directory is missing. Existing directories are left alone.
    tokio::fs::create_dir_all(&config.storage_root).await?;
    let storage_root = tokio::fs::canonicalize(&config.storage_root).await?;
    tracing::info!(root = %storage_root.display(), "library root ready");

    spawn_session_purge(pool.clone());

    let listener = TcpListener::bind(config.listen_addr).await?;
    let bound = listener.local_addr()?;

    tracing::info!(
        service = SERVICE_NAME,
        address = %bound,
        environment = ?config.environment,
        "listening"
    );

    let state = AppState::new(
        pool,
        homecloud_api::app::AppSettings {
            storage_root,
            production: config.environment.is_production(),
            trusted_origins: config.trusted_origins.clone(),
            public_origin: config.public_origin.clone(),
        },
    );

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Periodically removes expired session rows.
///
/// Correctness does not depend on this — every lookup checks expiry in
/// the database — so a failure is logged and the loop continues.
fn spawn_session_purge(pool: sqlx::PgPool) {
    const INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(INTERVAL);
        // The first tick fires immediately; skip it so startup does no
        // database work it does not need to.
        ticker.tick().await;

        loop {
            ticker.tick().await;

            match homecloud_auth::session::purge_expired(&pool).await {
                Ok(0) => {}
                Ok(removed) => tracing::info!(removed, "purged expired sessions"),
                Err(error) => tracing::warn!(error = %error, "session purge failed"),
            }

            // Expired share links are marked revoked in the same sweep,
            // so an owner's list of live links stays truthful.
            match homecloud_api::shares::purge_expired(&pool).await {
                Ok(0) => {}
                Ok(expired) => tracing::info!(expired, "closed expired share links"),
                Err(error) => tracing::warn!(error = %error, "share purge failed"),
            }
        }
    });
}

/// Stops accepting new connections on Ctrl-C or SIGTERM so in-flight
/// requests can finish instead of being cut off mid-response.
async fn shutdown_signal() {
    let interrupt = async {
        signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => tracing::warn!(error = %error, "cannot listen for SIGTERM"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }

    tracing::info!("shutdown signal received");
}
