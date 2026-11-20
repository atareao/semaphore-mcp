//! MCP server state and tool registry.
//!
//! The registry is intentionally empty in the initial skeleton. The v1 scope
//! lists the 12 tools below; they will be registered incrementally.

use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestMethod, CallToolRequestParams, CallToolResponse, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};

use crate::client::{SemaphoreClient, SemaphoreError};
use crate::config::Config;

/// Tools planned for v1.
pub const TOOL_NAMES: [&str; 12] = [
    "ping",
    "list_projects",
    "list_templates",
    "get_template",
    "run_task",
    "get_task",
    "list_tasks",
    "stop_task",
    "get_task_raw_output",
    "list_inventory",
    "list_repositories",
    "list_access_keys",
];

/// Shared state for the MCP server.
#[derive(Clone)]
pub struct SemaphoreServer {
    config: Arc<Config>,
    client: SemaphoreClient,
}

impl SemaphoreServer {
    /// Build the server from a validated configuration.
    pub fn new(config: Config) -> Result<Self, SemaphoreError> {
        let client = SemaphoreClient::new(&config.semaphore_url)?;
        Ok(Self {
            config: Arc::new(config),
            client,
        })
    }

    /// Runtime configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Semaphore REST client.
    pub fn client(&self) -> &SemaphoreClient {
        &self.client
    }

    /// Names of the tools planned for v1.
    pub fn tool_names(&self) -> &'static [&'static str] {
        &TOOL_NAMES
    }
}

impl ServerHandler for SemaphoreServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Semaphore UI MCP server. Tool registry is empty in this initial skeleton.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::default())
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        Err(McpError::method_not_found::<CallToolRequestMethod>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_tools_are_listed() {
        assert_eq!(TOOL_NAMES.len(), 12);
        assert!(TOOL_NAMES.contains(&"ping"));
        assert!(TOOL_NAMES.contains(&"run_task"));
    }
}
