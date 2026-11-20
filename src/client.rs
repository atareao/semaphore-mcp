//! Typed REST client for the Semaphore UI API.
//!
//! The client never stores a token: every call receives the token to forward
//! (token passthrough) and sends it as `Authorization: Bearer <token>`.

use reqwest::Response;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    pub id: i64,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub template_id: Option<i64>,
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
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
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
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

/// REST client for Semaphore UI.
#[derive(Debug, Clone)]
pub struct SemaphoreClient {
    base_url: String,
    http: reqwest::Client,
}

impl SemaphoreClient {
    /// Create a client pointing at the internal Semaphore URL.
    pub fn new(base_url: impl Into<String>) -> Result<Self, SemaphoreError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .user_agent(concat!("semaphore-mcp/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { base_url, http })
    }

    /// Internal base URL used for REST calls.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    async fn get<T: DeserializeOwned>(&self, token: &str, path: &str) -> Result<T, SemaphoreError> {
        let url = self.url(path);
        let response = self.http.get(&url).bearer_auth(token).send().await?;
        Self::decode(response, &url).await
    }

    async fn decode<T: DeserializeOwned>(
        response: Response,
        url: &str,
    ) -> Result<T, SemaphoreError> {
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(SemaphoreError::Status {
                status: status.as_u16(),
                url: url.to_string(),
                body,
            });
        }
        Ok(serde_json::from_str(&body)?)
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
        let _ = (token, project_id);
        Err(SemaphoreError::NotImplemented("list_templates"))
    }

    /// `GET /api/project/{project_id}/templates/{template_id}`
    pub async fn get_template(
        &self,
        token: &str,
        project_id: i64,
        template_id: i64,
    ) -> Result<Template, SemaphoreError> {
        let _ = (token, project_id, template_id);
        Err(SemaphoreError::NotImplemented("get_template"))
    }

    /// `GET /api/project/{project_id}/tasks`
    pub async fn list_tasks(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Task>, SemaphoreError> {
        let _ = (token, project_id);
        Err(SemaphoreError::NotImplemented("list_tasks"))
    }

    /// `GET /api/project/{project_id}/tasks/{task_id}`
    pub async fn get_task(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<Task, SemaphoreError> {
        let _ = (token, project_id, task_id);
        Err(SemaphoreError::NotImplemented("get_task"))
    }

    /// `GET /api/project/{project_id}/tasks/{task_id}/raw_output`
    pub async fn get_task_raw_output(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<String, SemaphoreError> {
        let _ = (token, project_id, task_id);
        Err(SemaphoreError::NotImplemented("get_task_raw_output"))
    }

    /// `POST /api/project/{project_id}/tasks/{task_id}/stop`
    pub async fn stop_task(
        &self,
        token: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<(), SemaphoreError> {
        let _ = (token, project_id, task_id);
        Err(SemaphoreError::NotImplemented("stop_task"))
    }

    /// `POST /api/project/{project_id}/tasks`
    pub async fn run_task(
        &self,
        token: &str,
        project_id: i64,
        request: &RunTaskRequest,
    ) -> Result<Task, SemaphoreError> {
        let _ = (token, project_id, request);
        Err(SemaphoreError::NotImplemented("run_task"))
    }

    /// `GET /api/project/{project_id}/inventory`
    pub async fn list_inventory(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Inventory>, SemaphoreError> {
        let _ = (token, project_id);
        Err(SemaphoreError::NotImplemented("list_inventory"))
    }

    /// `GET /api/project/{project_id}/repositories`
    pub async fn list_repositories(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<Repository>, SemaphoreError> {
        let _ = (token, project_id);
        Err(SemaphoreError::NotImplemented("list_repositories"))
    }

    /// `GET /api/project/{project_id}/keys`
    pub async fn list_access_keys(
        &self,
        token: &str,
        project_id: i64,
    ) -> Result<Vec<AccessKey>, SemaphoreError> {
        let _ = (token, project_id);
        Err(SemaphoreError::NotImplemented("list_access_keys"))
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

        assert!(
            task.is_ok(),
            "run_task should POST to /api/project/7/tasks with template_id, got {task:?}"
        );
    }
}
