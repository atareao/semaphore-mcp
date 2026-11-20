//! Helpers to build public, user-facing Semaphore URLs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Join a base URL with a path, avoiding duplicated slashes.
///
/// Absolute paths (already starting with `http://` or `https://`) are returned
/// unchanged. Trailing slashes on the base and leading slashes on the path are
/// collapsed into a single separator.
pub fn public_url_for(base_public_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }

    let base = base_public_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');

    if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    }
}

/// Public URLs returned alongside task results.
///
/// They are not part of the Semaphore API payload: the client builds them from
/// the configured public URL and the task coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WebUrls {
    /// Direct link to the task detail view.
    pub task_detail: String,
    /// Link to the project task history.
    pub project_tasks: String,
}

/// Link to the task history of a project.
pub fn project_tasks_url(base_public_url: &str, project_id: &str) -> String {
    public_url_for(base_public_url, &format!("project/{project_id}/history"))
}

/// Build the public URLs for a task.
pub fn task_web_urls(base_public_url: &str, project_id: &str, task_id: &str) -> WebUrls {
    WebUrls {
        task_detail: public_url_for(
            base_public_url,
            &format!("project/{project_id}/history?t={task_id}"),
        ),
        project_tasks: project_tasks_url(base_public_url, project_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_base_and_path_with_a_single_slash() {
        assert_eq!(
            public_url_for("https://semaphore.example.com/", "/project/1/history"),
            "https://semaphore.example.com/project/1/history"
        );
        assert_eq!(
            public_url_for("https://semaphore.example.com", "project/1/history"),
            "https://semaphore.example.com/project/1/history"
        );
    }

    #[test]
    fn keeps_absolute_paths_untouched() {
        assert_eq!(
            public_url_for(
                "https://semaphore.example.com",
                "https://other.example.com/project/1"
            ),
            "https://other.example.com/project/1"
        );
    }

    #[test]
    fn builds_task_web_urls() {
        let urls = task_web_urls("https://semaphore.example.com/", "7", "42");

        assert_eq!(
            urls.task_detail,
            "https://semaphore.example.com/project/7/history?t=42"
        );
        assert_eq!(
            urls.project_tasks,
            "https://semaphore.example.com/project/7/history"
        );
    }
}
