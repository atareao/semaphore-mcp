//! Typed REST client for the Semaphore UI API.
//!
//! The client never stores a token: every call receives the token to forward
//! (token passthrough) and sends it as `Authorization: Bearer <token>`.

use std::time::Duration;

use reqwest::Response;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::urls::{WebUrls, task_web_urls};

/// Timeout applied to every REST call.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout applied to establishing the TCP connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Semaphore project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: i64,
    pub name: String,
}

/// Semaphore task template.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Template {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub playbook: Option<String>,
}

/// Semaphore task.
///
/// `web_urls` is not returned by the Semaphore API: the client fills it in with
/// public, user-facing links built from the configured public URL.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    pub id: i64,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub template_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_urls: Option<WebUrls>,
}

/// Semaphore inventory.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Inventory {
    pub id: i64,
    pub name: String,
}

/// Semaphore repository.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Repository {
    pub id: i64,
    pub name: String,
}

/// Semaphore access key.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccessKey {
    pub id: i64,
    pub name: String,
}

/// Body sent to `POST /api/project/{project_id}/tasks`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunTaskRequest {
    pub template_id: i64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub debug: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Errors returned by the Semaphore client.
#[derive(Debug, thiserror::Error)]
pub enum SemaphoreError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Semaphore returned {status} for {url}: {body}")]
    Status {
        status: u16,
        url: String,
        body: String,
    },
    #[error("failed to decode Semaphore response: {0}")]
    Decode(#[from] serde_json::Error),
}

/// REST client for Semaphore UI.
///
/// `base_url` is used for REST calls; `public_url` is only used to build the
/// user-facing `web_urls` attached to task responses.
#[derive(Debug, Clone)]
pub struct SemaphoreClient {
    base_url: String,
    public_url: String,
    http: reqwest::Client,
}

impl SemaphoreClient {
    /// Create a client pointing at the internal Semaphore URL.
    ///
    /// The public URL used for `web_urls` defaults to `base_url`.
    pub fn new(base_url: impl Into<String>) -> Result<Self, SemaphoreError> {
        let base_url = base_url.into();
        Self::with_public_url(base_url.clone(), base_url)
    }

    /// Create a client with distinct internal and public URLs.
    ///
    /// `base_url` is used for REST calls; `public_url` (typically
    /// `SEMAPHORE_PUBLIC_URL`) is used to build the `web_urls` returned with
    /// tasks.
    pub fn with_public_url(
        base_url: impl Into<String>,
        public_url: impl Into<String>,
    ) -> Result<Self, SemaphoreError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let public_url = public_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .user_agent(concat!("semaphore-mcp/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()?;
        Ok(Self {
            base_url,
            public_url,
            http,
        })
    }

    /// Internal base URL used for REST calls.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Public base URL used to build `web_urls`.
    pub fn public_url(&self) -> &str {
        &self.public_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// Attach the public `web_urls` for a task, if not already present.
    fn attach_web_urls(&self, project_id: i64, task: &mut Task) {
        task.web_urls = Some(task_web_urls(
            &self.public_url,
            &project_id.to_string(),
            &task.id.to_string(),
        ));
    }

    /// Read a response body, mapping non-2xx statuses to a typed error.
    async fn read_body(response: Response, url: &str) -> Result<String, SemaphoreError> {
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(SemaphoreError::Status {
                status: status.as_u16(),
                url: url.to_string(),
                body,
            });
        }
        Ok(body)
    }

    async fn get_text(&self, token: &str, path: &str) -> Result<String, SemaphoreError> {
        let url = self.url(path);
        let response = self.http.get(&url).bearer_auth(token).send().await?;
        Self::read_body(response, &url).await
    }

    async fn get<T: DeserializeOwned>(&self, token: &str, path: &str) -> Result<T, SemaphoreError> {
        let body = self.get_text(token, path).await?;
        Ok(serde_json::from_str(&body)?)
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        token: &str,
        path: &str,
        body: &B,
    ) -> Result<T, SemaphoreError> {
        let url = self.url(path);
        let response = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(body)
            .send()
            .await?;
        let body = Self::read_body(response, &url).await?;
        Ok(serde_json::from_str(&body)?)
    }

    async fn post_empty(&self, token: &str, path: &str) -> Result<(), SemaphoreError> {
        let url = self.url(path);
        let response = self.http.post(&url).bearer_auth(token).send().await?;
        Self::read_body(response, &url).await?;
        Ok(())
    }

    /// `GET /api/projects`
    pub async fn list_projects(&self, token: &str) -> Result<Vec<Project>, SemaphoreError> {
        self.get(token, "api/projects").await
    }

    /// `GET /api/project/{project_id}/templates`
    pub async fn list_templates(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Template>, SemaphoreError> {
        self.get(token, &format!("api/project/{project_id}/templates"))
            .await
    }

    /// `GET /api/project/{project_id}/templates/{template_id}`
    pub async fn get_template(
        &self,
        token: &str,
        project_id: i64,
        template_id: i64,
    ) -> Result<Template, SemaphoreError> {
        self.get(
            token,
            &format!("api/project/{project_id}/templates/{template_id}"),
        )
        .await
    }

    /// `GET /api/project/{project_id}/tasks`
    pub async fn list_tasks(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Task>, SemaphoreError> {
        let mut tasks: Vec<Task> = self
            .get(token, &format!("api/project/{project_id}/tasks"))
            .await?;
        for task in &mut tasks {
            self.attach_web_urls(project_id, task);
        }
        Ok(tasks)
    }

    /// `GET /api/project/{project_id}/tasks/{task_id}`
    pub async fn get_task(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<Task, SemaphoreError> {
        let mut task: Task = self
            .get(token, &format!("api/project/{project_id}/tasks/{task_id}"))
            .await?;
        self.attach_web_urls(project_id, &mut task);
        Ok(task)
    }

    /// `GET /api/project/{project_id}/tasks/{task_id}/raw_output`
    ///
    /// Returns the raw task log as plain text, exactly as Semaphore stores it
    /// (ANSI escapes included). No parsing or heuristics are applied.
    pub async fn get_task_raw_output(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<String, SemaphoreError> {
        self.get_text(
            token,
            &format!("api/project/{project_id}/tasks/{task_id}/raw_output"),
        )
        .await
    }

    /// `POST /api/project/{project_id}/tasks/{task_id}/stop`
    pub async fn stop_task(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<(), SemaphoreError> {
        self.post_empty(
            token,
            &format!("api/project/{project_id}/tasks/{task_id}/stop"),
        )
        .await
    }

    /// `POST /api/project/{project_id}/tasks`
    pub async fn run_task(
        &self,
        token: &str,
        project_id: i64,
        request: &RunTaskRequest,
    ) -> Result<Task, SemaphoreError> {
        let mut task: Task = self
            .post(token, &format!("api/project/{project_id}/tasks"), request)
            .await?;
        self.attach_web_urls(project_id, &mut task);
        Ok(task)
    }

    /// `GET /api/project/{project_id}/inventory`
    pub async fn list_inventory(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Inventory>, SemaphoreError> {
        self.get(token, &format!("api/project/{project_id}/inventory"))
            .await
    }

    /// `GET /api/project/{project_id}/repositories`
    pub async fn list_repositories(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Repository>, SemaphoreError> {
        self.get(token, &format!("api/project/{project_id}/repositories"))
            .await
    }

    /// `GET /api/project/{project_id}/keys`
    pub async fn list_access_keys(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<AccessKey>, SemaphoreError> {
        self.get(token, &format!("api/project/{project_id}/keys"))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn list_projects_sends_bearer_and_hits_api_projects() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "infra"}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let projects = client
            .list_projects("test-token")
            .await
            .expect("list_projects should succeed");

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "infra");
    }

    #[tokio::test]
    async fn run_task_posts_template_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/project/7/tasks"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"template_id": 42})))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": 99, "status": "waiting"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let request = RunTaskRequest {
            template_id: 42,
            ..Default::default()
        };

        let task = client.run_task("test-token", 7, &request).await;

        let task = task.expect("run_task should POST to /api/project/7/tasks with template_id");

        assert_eq!(task.id, 99);
        let urls = task.web_urls.expect("run_task should attach web_urls");
        assert_eq!(
            urls.task_detail,
            format!("{}/project/7/history?t=99", server.uri())
        );
    }

    /// Assert that an error is a typed [`SemaphoreError::Status`] with the
    /// expected status code and a non-empty body.
    fn assert_status_error(error: SemaphoreError, expected: u16) {
        match error {
            SemaphoreError::Status { status, body, .. } => {
                assert_eq!(status, expected);
                assert!(!body.is_empty(), "status errors should carry the body");
            }
            other => panic!("expected a Status error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_templates_gets_project_templates() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/templates"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([{"id": 1, "name": "deploy", "playbook": "site.yml"}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let templates = client
            .list_templates("test-token", 7)
            .await
            .expect("list_templates should succeed");

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "deploy");
        assert_eq!(templates[0].playbook.as_deref(), Some("site.yml"));
    }

    #[tokio::test]
    async fn get_template_gets_single_template() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/templates/3"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": 3, "name": "deploy"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let template = client
            .get_template("test-token", 7, 3)
            .await
            .expect("get_template should succeed");

        assert_eq!(template.id, 3);
        assert_eq!(template.name, "deploy");
    }

    #[tokio::test]
    async fn list_tasks_gets_project_tasks() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([{"id": 99, "status": "success", "template_id": 3}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let tasks = client
            .list_tasks("test-token", 7)
            .await
            .expect("list_tasks should succeed");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status.as_deref(), Some("success"));
        assert!(tasks[0].web_urls.is_some());
    }

    #[tokio::test]
    async fn get_task_gets_single_task_and_builds_web_urls_from_public_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"id": 99, "status": "running", "template_id": 3})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client =
            SemaphoreClient::with_public_url(server.uri(), "https://semaphore.example.com")
                .expect("client");
        let task = client
            .get_task("test-token", 7, 99)
            .await
            .expect("get_task should succeed");

        assert_eq!(task.id, 99);
        let urls = task.web_urls.expect("get_task should attach web_urls");
        assert_eq!(
            urls.task_detail,
            "https://semaphore.example.com/project/7/history?t=99"
        );
        assert_eq!(
            urls.project_tasks,
            "https://semaphore.example.com/project/7/history"
        );
    }

    #[tokio::test]
    async fn get_task_raw_output_returns_text_untouched() {
        let server = MockServer::start().await;
        let raw = "PLAY [all] ***\n\x1b[0;32mok\x1b[0m: [host]\n";
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99/raw_output"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(raw))
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let output = client
            .get_task_raw_output("test-token", 7, 99)
            .await
            .expect("get_task_raw_output should succeed");

        assert_eq!(output, raw);
    }

    #[tokio::test]
    async fn stop_task_posts_to_stop_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/project/7/tasks/99/stop"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");

        client
            .stop_task("test-token", 7, 99)
            .await
            .expect("stop_task should succeed");
    }

    #[tokio::test]
    async fn list_inventory_gets_project_inventory() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/inventory"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "prod"}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let inventory = client
            .list_inventory("test-token", 7)
            .await
            .expect("list_inventory should succeed");

        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].name, "prod");
    }

    #[tokio::test]
    async fn list_repositories_gets_project_repositories() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/repositories"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "infra"}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let repositories = client
            .list_repositories("test-token", 7)
            .await
            .expect("list_repositories should succeed");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].name, "infra");
    }

    #[tokio::test]
    async fn list_access_keys_gets_project_keys() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/keys"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 1, "name": "ssh-prod"}])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let keys = client
            .list_access_keys("test-token", 7)
            .await
            .expect("list_access_keys should succeed");

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "ssh-prod");
    }

    #[tokio::test]
    async fn unauthorized_response_is_typed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let error = client
            .list_projects("bad-token")
            .await
            .expect_err("401 should fail");

        assert_status_error(error, 401);
    }

    #[tokio::test]
    async fn not_found_response_is_typed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99"))
            .respond_with(ResponseTemplate::new(404).set_body_string("task not found"))
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let error = client
            .get_task("test-token", 7, 99)
            .await
            .expect_err("404 should fail");

        assert_status_error(error, 404);
    }

    #[tokio::test]
    async fn server_error_response_is_typed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/project/7/tasks/99/stop"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let error = client
            .stop_task("test-token", 7, 99)
            .await
            .expect_err("500 should fail");

        assert_status_error(error, 500);
    }

    #[tokio::test]
    async fn invalid_json_is_reported_as_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>nope</html>"))
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let error = client
            .list_projects("test-token")
            .await
            .expect_err("invalid JSON should fail");

        assert!(matches!(error, SemaphoreError::Decode(_)));
    }

    #[tokio::test]
    async fn raw_output_http_error_is_typed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/project/7/tasks/99/raw_output"))
            .respond_with(ResponseTemplate::new(404).set_body_string("no output yet"))
            .mount(&server)
            .await;

        let client = SemaphoreClient::new(server.uri()).expect("client");
        let error = client
            .get_task_raw_output("test-token", 7, 99)
            .await
            .expect_err("404 should fail");

        assert_status_error(error, 404);
    }
}
