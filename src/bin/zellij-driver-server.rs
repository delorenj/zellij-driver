use anyhow::{Context, Result};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, signal};
use tracing_subscriber::EnvFilter;
use zellij_driver::bridge::{router, ZellijCliExecutor};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let token = load_token().await?;
    anyhow::ensure!(
        token.len() >= 32,
        "ZELLIJ_DRIVER_TOKEN must be at least 32 bytes"
    );

    let bind: SocketAddr = env::var("ZELLIJ_DRIVER_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8084".to_string())
        .parse()
        .context("ZELLIJ_DRIVER_BIND must be a socket address")?;
    let session = env::var("ZELLIJ_DRIVER_SESSION").unwrap_or_else(|_| "Workspace".to_string());
    let zellij_binary =
        env::var("ZELLIJ_BIN").unwrap_or_else(|_| "/home/delorenj/.local/bin/zellij".to_string());

    let executor = Arc::new(ZellijCliExecutor::new(zellij_binary, session.clone()));
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind Zellij bridge to {bind}"))?;

    tracing::info!(%bind, %session, "Zellij driver bridge listening");
    axum::serve(listener, router(executor, token))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Zellij driver HTTP server stopped unexpectedly")?;
    Ok(())
}

async fn load_token() -> Result<String> {
    if let Ok(token) = env::var("ZELLIJ_DRIVER_TOKEN") {
        return Ok(token);
    }

    let token_file = env::var("ZELLIJ_DRIVER_TOKEN_FILE")
        .context("ZELLIJ_DRIVER_TOKEN or ZELLIJ_DRIVER_TOKEN_FILE is required")?;
    let token = tokio::fs::read_to_string(&token_file)
        .await
        .with_context(|| format!("failed to read bridge credential from {token_file}"))?;
    Ok(token.trim().to_string())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
