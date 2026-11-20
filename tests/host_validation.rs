//! Host header validation tests for the streamable HTTP transport.
//!
//! They mount the real router with [`build_app`] and send requests through
//! `tower::ServiceExt::oneshot`, pinning the `Host` header by hand to emulate a
//! deployment behind a reverse proxy such as Traefik.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use semaphore_mcp::{AllowedHosts, Config, SemaphoreServer, build_app};
use tower::ServiceExt;

/// Host used in the production deployment that motivated these tests.
const PUBLIC_HOST: &str = "semaphore-mcp.territoriolinux.es";
/// A host that must never be accepted by an explicit list.
const FOREIGN_HOST: &str = "evil.example.com";
/// Message emitted by the rmcp host-validation middleware.
const FORBIDDEN_BODY: &str = "Host header is not allowed";

/// Build a server whose configuration is irrelevant for host validation.
fn test_server() -> SemaphoreServer {
    let config = Config {
        semaphore_url: "http://semaphore:3000".to_string(),
        public_url: "http://semaphore:3000".to_string(),
        host: "0.0.0.0".to_string(),
        port: 8090,
        api_token: None,
        allowed_hosts: AllowedHosts::Default,
    };
    SemaphoreServer::new(config).expect("test server should build")
}

/// Send a minimal MCP `ping` request with an explicit `Host` header.
async fn post_mcp(allowed: &AllowedHosts, host: &str) -> (StatusCode, String) {
    let (app, _token) = build_app(test_server(), allowed);
    let request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::HOST, host)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        ))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router is infallible");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should be readable")
        .to_bytes();

    (status, String::from_utf8_lossy(&body).to_string())
}

#[tokio::test]
async fn listed_host_is_not_forbidden() {
    let allowed = AllowedHosts::List(vec![PUBLIC_HOST.to_string()]);

    let (status, body) = post_mcp(&allowed, PUBLIC_HOST).await;

    assert_ne!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert!(
        !body.contains(FORBIDDEN_BODY),
        "host was rejected: {status} {body}"
    );
}

#[tokio::test]
async fn host_outside_the_list_is_forbidden() {
    let allowed = AllowedHosts::List(vec![PUBLIC_HOST.to_string()]);

    let (status, body) = post_mcp(&allowed, FOREIGN_HOST).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert!(body.contains(FORBIDDEN_BODY), "unexpected body: {body}");
}

#[tokio::test]
async fn default_policy_rejects_foreign_host() {
    let (status, body) = post_mcp(&AllowedHosts::Default, FOREIGN_HOST).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert!(body.contains(FORBIDDEN_BODY), "unexpected body: {body}");
}

#[tokio::test]
async fn default_policy_accepts_loopback_with_port() {
    let (status, body) = post_mcp(&AllowedHosts::Default, "127.0.0.1:8090").await;

    assert_ne!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert!(
        !body.contains(FORBIDDEN_BODY),
        "host was rejected: {status} {body}"
    );
}

#[tokio::test]
async fn any_policy_accepts_any_host() {
    let (status, body) = post_mcp(&AllowedHosts::Any, FOREIGN_HOST).await;

    assert_ne!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert!(
        !body.contains(FORBIDDEN_BODY),
        "host was rejected: {status} {body}"
    );
}
