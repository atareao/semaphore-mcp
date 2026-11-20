//! Binary entry point for the Semaphore MCP server.

use semaphore_mcp::{Config, SemaphoreServer, build_app};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    let address = format!("{}:{}", config.host, config.port);

    let server = SemaphoreServer::new(config.clone())?;
    let (router, cancellation_token) = build_app(server, &config.allowed_hosts);
    let listener = tokio::net::TcpListener::bind(&address).await?;

    tracing::info!("semaphore-mcp listening on http://{address}/mcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation_token.cancel();
        })
        .await?;

    Ok(())
}
