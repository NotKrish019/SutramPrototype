//! Binary entrypoint for SUTRAM Rust Backend.

use std::error::Error;
use sutram_backend_rust::{create_router_async, observability::init_tracing, AppConfig};
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = AppConfig::from_env()?;
    init_tracing(&config);

    let addr = config.socket_addr();
    let app = create_router_async(config).await;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("SUTRAM Rust backend server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("SUTRAM Rust backend server stopped gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C interrupt signal; initiating graceful shutdown...");
        },
        _ = terminate => {
            info!("Received termination signal; initiating graceful shutdown...");
        },
    }
}
