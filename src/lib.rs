//! Semaphore MCP server library.

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

pub mod client;
pub mod config;
pub mod tools;
pub mod urls;

pub use client::{SemaphoreClient, SemaphoreError};
pub use config::{AllowedHosts, Config, ConfigError};
pub use tools::SemaphoreServer;

/// Build the axum router serving the MCP streamable HTTP transport at `/mcp`.
///
/// The `Host` header policy comes from [`AllowedHosts`]:
///
/// - [`AllowedHosts::Default`] keeps rmcp's safe default (loopback only);
/// - [`AllowedHosts::Any`] disables the check (any `Host` is accepted);
/// - [`AllowedHosts::List`] accepts exactly the listed hosts.
///
/// The returned [`CancellationToken`] is the parent token: cancel it to stop the
/// running sessions, as `main` does on shutdown.
pub fn build_app(
    server: SemaphoreServer,
    allowed_hosts: &AllowedHosts,
) -> (axum::Router, CancellationToken) {
    let cancellation_token = CancellationToken::new();
    let mut http_config = StreamableHttpServerConfig::default()
        .with_cancellation_token(cancellation_token.child_token());

    match allowed_hosts {
        AllowedHosts::Default => {
            tracing::info!("allowed hosts: loopback (default)");
        }
        AllowedHosts::Any => {
            tracing::info!("allowed hosts: any");
            http_config = http_config.disable_allowed_hosts();
        }
        AllowedHosts::List(hosts) => {
            tracing::info!(allowed_hosts = ?hosts, "allowed hosts: explicit list");
            http_config = http_config.with_allowed_hosts(hosts.iter().cloned());
        }
    }

    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        http_config,
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    (router, cancellation_token)
}
