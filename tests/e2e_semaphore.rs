//! End-to-end test against a real Semaphore UI instance.
//!
//! It is `#[ignore]`d so it never runs by default or in CI: it needs a live
//! Semaphore, a real API token and a project/template where a test task can be
//! launched. Run it by hand with:
//!
//! ```bash
//! set -a; . ~/.config/semaphore-mcp/e2e.env; set +a
//! cargo test --test e2e_semaphore -- --ignored --nocapture
//! ```
//!
//! Required environment variables (read from the environment, never stored in
//! the repository): `SEMAPHORE_URL`, `SEMAPHORE_API_TOKEN`,
//! `SEMAPHORE_PUBLIC_URL`, `SEMAPHORE_E2E_PROJECT_ID` and
//! `SEMAPHORE_E2E_TEMPLATE_ID`.
//!
//! The template used by the E2E **must be harmless and finish successfully**:
//! the test asserts the task ends in `success`, and if it ends in any other
//! final state (`error`, `stopped`, …) the test fails printing the raw log. This
//! way a malformed `run_task` body cannot pass unnoticed.
//!
//! The test never prints the token nor the `Authorization` header: only ids,
//! statuses and URLs.

use std::time::{Duration, Instant};

use semaphore_mcp::client::RunTaskRequest;
use semaphore_mcp::{Config, SemaphoreClient};

/// Interval between `get_task` polls.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Maximum time to wait for the task to reach a final state.
const TASK_TIMEOUT: Duration = Duration::from_secs(180);
/// Terminal statuses reported by Semaphore; `waiting` and `running` are not.
const FINAL_STATES: [&str; 3] = ["success", "error", "stopped"];

/// Read a required environment variable, failing with a clear message.
fn required_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => panic!("missing required environment variable for the E2E test: {name}"),
    }
}

/// Read a required numeric environment variable.
fn required_id(name: &str) -> i64 {
    let value = required_env(name);
    match value.trim().parse::<i64>() {
        Ok(id) => id,
        Err(_) => panic!("environment variable {name} must be a number"),
    }
}

/// `scheme://authority` of a URL, without pulling in an extra dependency.
fn origin(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let authority = rest.split('/').next().unwrap_or_default();
            format!("{scheme}://{authority}")
        }
        None => url.to_string(),
    }
}

#[tokio::test]
#[ignore = "requires a real Semaphore instance; run with --ignored"]
async fn full_task_lifecycle_against_real_semaphore() {
    let config = Config::from_env().expect("SEMAPHORE_URL must be set for the E2E test");
    required_env("SEMAPHORE_API_TOKEN");
    required_env("SEMAPHORE_PUBLIC_URL");
    let token = config
        .resolve_token(None)
        .expect("SEMAPHORE_API_TOKEN must be set for the E2E test");
    let project_id = required_id("SEMAPHORE_E2E_PROJECT_ID");
    let template_id = required_id("SEMAPHORE_E2E_TEMPLATE_ID");

    let client = SemaphoreClient::with_public_url(&config.semaphore_url, &config.public_url)
        .expect("client should build");

    let projects = client.list_projects(&token).await.expect("list_projects");
    println!("projects visible to the token: {}", projects.len());
    assert!(
        projects.iter().any(|project| project.id == project_id),
        "project {project_id} is not visible to the token"
    );

    let templates = client
        .list_templates(&token, project_id)
        .await
        .expect("list_templates");
    println!("templates in project {project_id}: {}", templates.len());
    assert!(
        templates.iter().any(|template| template.id == template_id),
        "template {template_id} is not visible in project {project_id}"
    );

    let request = RunTaskRequest {
        template_id,
        ..Default::default()
    };
    let created = client
        .run_task(&token, project_id, &request)
        .await
        .expect("run_task");
    let task_id = created.id;
    println!("started task {task_id} from template {template_id}");

    let deadline = Instant::now() + TASK_TIMEOUT;
    let task = loop {
        let task = client
            .get_task(&token, project_id, task_id)
            .await
            .expect("get_task");
        let status = task.status.clone().unwrap_or_else(|| "unknown".to_string());
        println!("task {task_id} status: {status}");
        if FINAL_STATES.contains(&status.as_str()) {
            break task;
        }
        assert!(
            Instant::now() < deadline,
            "task {task_id} did not reach a final state within {}s (last status: {status})",
            TASK_TIMEOUT.as_secs()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    };

    let status = task.status.clone().unwrap_or_default();
    let urls = task
        .web_urls
        .clone()
        .expect("task responses must carry web_urls");
    println!("task {task_id} finished with status: {status}");
    println!("task detail url: {}", urls.task_detail);
    println!("project tasks url: {}", urls.project_tasks);

    // Fetch the raw log *before* asserting on the final status so a failure can
    // be diagnosed from the assertion message itself.
    let raw = client
        .get_task_raw_output(&token, project_id, task_id)
        .await
        .expect("get_task_raw_output");
    println!("raw output bytes: {}", raw.len());
    // A template that prints nothing legitimately returns an empty log; if the
    // E2E template is silent, relax this assertion to `let _ = raw;`.
    assert!(
        !raw.is_empty(),
        "get_task_raw_output returned an empty log for task {task_id}"
    );

    assert_eq!(
        status, "success",
        "task {task_id} ended as '{status}'; the E2E template must be harmless and succeed. Raw log:\n{raw}"
    );

    assert_eq!(
        origin(&urls.task_detail),
        origin(&config.public_url),
        "web_urls must be built from SEMAPHORE_PUBLIC_URL"
    );
    if config.public_url != config.semaphore_url {
        assert_ne!(
            origin(&urls.task_detail),
            origin(&config.semaphore_url),
            "web_urls must not be built from SEMAPHORE_URL"
        );
    }
    assert!(
        urls.task_detail
            .contains(&format!("/project/{project_id}/history?t={task_id}")),
        "unexpected task detail URL: {}",
        urls.task_detail
    );
}
