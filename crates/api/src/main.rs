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

    let listener = TcpListener::bind(config.listen_addr).await?;
    let bound = listener.local_addr()?;

    tracing::info!(
        service = SERVICE_NAME,
        address = %bound,
        environment = ?config.environment,
        "listening"
    );

    let state = AppState::new(pool, storage_root, config.environment.is_production());

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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
