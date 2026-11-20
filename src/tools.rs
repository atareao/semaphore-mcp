//! MCP server state and tool registry.
//!
//! The server exposes the v1 read/execute tools over the streamable HTTP
//! transport. Every tool forwards the caller's `Authorization: Bearer` token to
//! Semaphore unless `SEMAPHORE_API_TOKEN` is configured, in which case the fixed
//! token always wins.

use std::sync::Arc;

use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData as McpError, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::{SemaphoreClient, SemaphoreError};
use crate::config::Config;

/// Tools registered in v1.
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

/// Input for tools that only need a project ID.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ProjectIdParams {
    /// Semaphore project ID.
    pub project_id: i64,
}

/// Input for tools that need a project and a template.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TemplateIdParams {
    /// Semaphore project ID.
    pub project_id: i64,
    /// Semaphore template ID.
    pub template_id: i64,
}

/// Input for tools that need a project and a task.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TaskIdParams {
    /// Semaphore project ID.
    pub project_id: i64,
    /// Semaphore task ID.
    pub task_id: i64,
}

/// Input for `run_task`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RunTaskParams {
    /// Semaphore project ID.
    pub project_id: i64,
    /// Semaphore template ID to run.
    pub template_id: i64,
}

/// Shared state for the MCP server.
#[derive(Clone)]
pub struct SemaphoreServer {
    config: Arc<Config>,
    client: SemaphoreClient,
}

#[tool_router]
impl SemaphoreServer {
    /// Build the server from a validated configuration.
    pub fn new(config: Config) -> Result<Self, SemaphoreError> {
        let client = SemaphoreClient::with_public_url(&config.semaphore_url, &config.public_url)?;
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

    /// Names of the tools registered in v1.
    pub fn tool_names(&self) -> &'static [&'static str] {
        &TOOL_NAMES
    }

    /// Resolve the Semaphore token for an incoming MCP request.
    ///
    /// A configured `SEMAPHORE_API_TOKEN` always wins; otherwise the
    /// `Authorization: Bearer <token>` header is forwarded to Semaphore.
    fn token_from_parts(&self, parts: &http::request::Parts) -> Result<String, McpError> {
        let authorization = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());

        self.config.resolve_token(authorization).ok_or_else(|| {
            McpError::invalid_params(
                "missing Semaphore token: send an Authorization: Bearer <token> header or set SEMAPHORE_API_TOKEN",
                None,
            )
        })
    }

    /// Health check.
    #[tool(description = "Health check. Returns pong when the MCP server is alive.")]
    async fn ping(&self) -> String {
        "pong".to_string()
    }

    /// List the projects visible to the token.
    #[tool(description = "List the Semaphore projects visible to the provided token.")]
    async fn list_projects(
        &self,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self.client.list_projects(&token).await {
            Ok(projects) => json_result(projects),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// List the task templates of a project.
    #[tool(description = "List the task templates of a Semaphore project.")]
    async fn list_templates(
        &self,
        Parameters(params): Parameters<ProjectIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self.client.list_templates(&token, params.project_id).await {
            Ok(templates) => json_result(templates),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// Get a single task template.
    #[tool(description = "Get a single task template from a Semaphore project.")]
    async fn get_template(
        &self,
        Parameters(params): Parameters<TemplateIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .get_template(&token, params.project_id, params.template_id)
            .await
        {
            Ok(template) => json_result(template),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// Run a task template.
    #[tool(description = "Run a Semaphore task template and return the created task.")]
    async fn run_task(
        &self,
        Parameters(params): Parameters<RunTaskParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        let request = crate::client::RunTaskRequest {
            template_id: params.template_id,
            ..Default::default()
        };
        match self
            .client
            .run_task(&token, params.project_id, &request)
            .await
        {
            Ok(task) => json_result(task),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// Get a task by ID.
    #[tool(description = "Get a Semaphore task by ID, including its public web URLs.")]
    async fn get_task(
        &self,
        Parameters(params): Parameters<TaskIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .get_task(&token, params.project_id, params.task_id)
            .await
        {
            Ok(task) => json_result(task),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// List the tasks of a project.
    #[tool(description = "List the tasks of a Semaphore project, including their public web URLs.")]
    async fn list_tasks(
        &self,
        Parameters(params): Parameters<ProjectIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self.client.list_tasks(&token, params.project_id).await {
            Ok(tasks) => json_result(tasks),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// Stop a running task.
    #[tool(description = "Stop a running Semaphore task.")]
    async fn stop_task(
        &self,
        Parameters(params): Parameters<TaskIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .stop_task(&token, params.project_id, params.task_id)
            .await
        {
            Ok(()) => json_result(serde_json::json!({
                "project_id": params.project_id,
                "task_id": params.task_id,
                "stopped": true,
            })),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// Get the raw output of a task.
    #[tool(description = "Get the raw output of a Semaphore task as plain text.")]
    async fn get_task_raw_output(
        &self,
        Parameters(params): Parameters<TaskIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .get_task_raw_output(&token, params.project_id, params.task_id)
            .await
        {
            Ok(raw_output) => json_result(serde_json::json!({
                "project_id": params.project_id,
                "task_id": params.task_id,
                "raw_output": raw_output,
            })),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// List the inventory of a project.
    #[tool(description = "List the inventory of a Semaphore project.")]
    async fn list_inventory(
        &self,
        Parameters(params): Parameters<ProjectIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self.client.list_inventory(&token, params.project_id).await {
            Ok(inventory) => json_result(inventory),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// List the repositories of a project.
    #[tool(description = "List the repositories of a Semaphore project.")]
    async fn list_repositories(
        &self,
        Parameters(params): Parameters<ProjectIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .list_repositories(&token, params.project_id)
            .await
        {
            Ok(repositories) => json_result(repositories),
            Err(error) => Ok(tool_error(error)),
        }
    }

    /// List the access keys of a project.
    #[tool(description = "List the access keys of a Semaphore project.")]
    async fn list_access_keys(
        &self,
        Parameters(params): Parameters<ProjectIdParams>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> Result<CallToolResult, McpError> {
        let token = self.token_from_parts(&parts)?;
        match self
            .client
            .list_access_keys(&token, params.project_id)
            .await
        {
            Ok(keys) => json_result(keys),
            Err(error) => Ok(tool_error(error)),
        }
    }
}

#[tool_handler(
    instructions = "Semaphore UI MCP server. List projects, templates, tasks, inventory, repositories and access keys; run and stop tasks. Send the Semaphore API token as Authorization: Bearer <token> unless SEMAPHORE_API_TOKEN is configured."
)]
impl rmcp::ServerHandler for SemaphoreServer {}

fn json_result<T: Serialize>(value: T) -> Result<CallToolResult, McpError> {
    let value = serde_json::to_value(value).map_err(|error| {
        McpError::internal_error(format!("failed to serialize tool response: {error}"), None)
    })?;
    Ok(CallToolResult::structured(value))
}

fn tool_error(error: SemaphoreError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn server_for(base_url: &str) -> SemaphoreServer {
        let config = Config {
            semaphore_url: base_url.to_string(),
            public_url: "https://semaphore.example.com".to_string(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_token: None,
        };
        SemaphoreServer::new(config).expect("server should build")
    }

    fn parts_with_bearer(token: &str) -> Extension<http::request::Parts> {
        let request = http::Request::builder()
            .header(http::header::AUTHORIZATION, format!("Bearer {token}"))
            .body(())
            .expect("request should build");
        Extension(request.into_parts().0)
    }

    fn parts_without_auth() -> Extension<http::request::Parts> {
        let request = http::Request::builder()
            .body(())
            .expect("request should build");
        Extension(request.into_parts().0)
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let server = server_for("http://127.0.0.1:1");
        assert_eq!(server.ping().await, "pong");
    }

    #[tokio::test]
    async fn list_projects_forwards_bearer_token() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "infra"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_projects(parts_with_bearer("request-token"))
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content[0]["name"], "infra");
    }

    #[tokio::test]
    async fn configured_token_wins_over_bearer_header() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .and(header("authorization", "Bearer fixed-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "infra"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = Config {
            semaphore_url: mock_server.uri(),
            public_url: "https://semaphore.example.com".to_string(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_token: Some("fixed-token".to_string()),
        };
        let server = SemaphoreServer::new(config).expect("server should build");
        let result = server
            .list_projects(parts_with_bearer("request-token"))
            .await
            .expect("tool should succeed");

        assert!(result.structured_content.is_some());
    }

    #[tokio::test]
    async fn list_templates_returns_templates() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/templates"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 3, "name": "deploy"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_templates(
                Parameters(ProjectIdParams { project_id: 7 }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content[0]["name"], "deploy");
    }

    #[tokio::test]
    async fn get_template_returns_template() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/templates/3"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": 3, "name": "deploy"})),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .get_template(
                Parameters(TemplateIdParams {
                    project_id: 7,
                    template_id: 3,
                }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content["name"], "deploy");
    }

    #[tokio::test]
    async fn run_task_posts_template_id() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/project/7/tasks"))
            .and(header("authorization", "Bearer request-token"))
            .and(body_json(json!({"template_id": 42})))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({"id": 99, "status": "waiting", "template_id": 42})),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .run_task(
                Parameters(RunTaskParams {
                    project_id: 7,
                    template_id: 42,
                }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content["id"], 99);
        assert_eq!(
            content["web_urls"]["task_detail"],
            "https://semaphore.example.com/project/7/history?t=99"
        );
    }

    #[tokio::test]
    async fn get_task_returns_web_urls() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"id": 99, "status": "running", "template_id": 42})),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .get_task(
                Parameters(TaskIdParams {
                    project_id: 7,
                    task_id: 99,
                }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(
            content["web_urls"]["task_detail"],
            "https://semaphore.example.com/project/7/history?t=99"
        );
    }

    #[tokio::test]
    async fn list_tasks_returns_web_urls() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([{"id": 99, "status": "running", "template_id": 42}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_tasks(
                Parameters(ProjectIdParams { project_id: 7 }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(
            content[0]["web_urls"]["project_tasks"],
            "https://semaphore.example.com/project/7/history"
        );
    }

    #[tokio::test]
    async fn stop_task_posts_stop_endpoint() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/project/7/tasks/99/stop"))
            .and(header("authorization", "Bearer request-token"))
            .and(header("content-type", "application/json"))
            .and(body_json(json!({"force": false})))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .stop_task(
                Parameters(TaskIdParams {
                    project_id: 7,
                    task_id: 99,
                }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content["stopped"], true);
    }

    #[tokio::test]
    async fn get_task_raw_output_returns_text() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99/raw_output"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("line 1\nline 2\n"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .get_task_raw_output(
                Parameters(TaskIdParams {
                    project_id: 7,
                    task_id: 99,
                }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content["raw_output"], "line 1\nline 2\n");
    }

    #[tokio::test]
    async fn list_inventory_returns_inventory() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/inventory"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "prod"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_inventory(
                Parameters(ProjectIdParams { project_id: 7 }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content[0]["name"], "prod");
    }

    #[tokio::test]
    async fn list_repositories_returns_repositories() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/repositories"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 2, "name": "infra"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_repositories(
                Parameters(ProjectIdParams { project_id: 7 }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content[0]["name"], "infra");
    }

    #[tokio::test]
    async fn list_access_keys_returns_keys() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/keys"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 3, "name": "deploy-key"}])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_access_keys(
                Parameters(ProjectIdParams { project_id: 7 }),
                parts_with_bearer("request-token"),
            )
            .await
            .expect("tool should succeed");

        let content = result.structured_content.expect("structured content");
        assert_eq!(content[0]["name"], "deploy-key");
    }

    #[tokio::test]
    async fn missing_token_is_rejected() {
        let server = server_for("http://127.0.0.1:1");
        let error = server
            .list_projects(parts_without_auth())
            .await
            .expect_err("missing token should fail");

        assert!(
            error.message.contains("missing Semaphore token"),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn upstream_error_is_returned_as_tool_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .and(header("authorization", "Bearer request-token"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let server = server_for(&mock_server.uri());
        let result = server
            .list_projects(parts_with_bearer("request-token"))
            .await
            .expect("tool should return a tool error");

        assert_eq!(result.is_error, Some(true));
        let text = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| text.text.as_str())
            .unwrap_or_default();
        assert!(text.contains("401"), "unexpected error text: {text}");
    }

    #[test]
    fn tool_router_exposes_all_v1_tools() {
        let router = SemaphoreServer::tool_router();
        let mut names: Vec<String> = router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();

        let mut expected: Vec<String> = TOOL_NAMES.iter().map(|name| name.to_string()).collect();
        expected.sort();

        assert_eq!(names, expected);
    }
}
