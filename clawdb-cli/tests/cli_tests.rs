/// Integration tests for the clawdb CLI binary using assert_cmd + wiremock.
///
/// These tests mock the HTTP server and run the actual binary via assert_cmd.
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn clawdb() -> Command {
    Command::cargo_bin("clawdb-cli").expect("binary not found")
}

// ─── Basic CLI meta-tests ──────────────────────────────────────────────────

#[test]
fn test_version_flag() {
    clawdb()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_help_flag() {
    clawdb().arg("--help").assert().success();
}

#[test]
fn test_completion_bash() {
    let out = clawdb()
        .args(["completion", "bash"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(!out.is_empty(), "completion output should not be empty");
}

// ─── init command ─────────────────────────────────────────────────────────

#[test]
fn test_init() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("clawdb-test");

    clawdb()
        .args(["init", "--data-dir", dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(
        dir.join("config.toml").exists(),
        "config.toml should have been created"
    );
}

// ─── config command ────────────────────────────────────────────────────────

#[test]
fn test_config_get_set() {
    let tmp = TempDir::new().unwrap();

    clawdb()
        .env("CLAW_DATA_DIR", tmp.path())
        .args(["config", "set", "base_url", "http://x.y:9999"])
        .assert()
        .success();

    clawdb()
        .env("CLAW_DATA_DIR", tmp.path())
        .args(["config", "get", "base_url"])
        .assert()
        .success()
        .stdout(predicate::str::contains("http://x.y:9999"));
}

// ─── remember ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_remember_ok() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/memories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "mem_abcd1234"
        })))
        .mount(&server)
        .await;

    clawdb()
        .args([
            "--base-url",
            &server.uri(),
            "--token",
            "test-token",
            "remember",
            "hello world",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("mem_abcd1234"));
}

#[tokio::test]
async fn test_remember_401() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/memories"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "Unauthorized",
            "detail": "no token provided"
        })))
        .mount(&server)
        .await;

    let out = clawdb()
        .args(["--base-url", &server.uri(), "remember", "hello"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = String::from_utf8_lossy(&out);
    assert!(
        stderr.contains("session create") || stderr.contains("Unauthorized"),
        "stderr should contain auth hint: {stderr}"
    );
}

// ─── search ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_table() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/memories/search"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "m1", "content": "alpha memory", "score": 0.9, "tags": ["tag1"], "memory_type": "message" },
                { "id": "m2", "content": "beta memory",  "score": 0.8, "tags": [],       "memory_type": "message" },
                { "id": "m3", "content": "gamma memory", "score": 0.7, "tags": ["tag2"], "memory_type": "context" },
            ])),
        )
        .mount(&server)
        .await;

    let out = clawdb()
        .args([
            "--base-url",
            &server.uri(),
            "--token",
            "tok",
            "--output",
            "table",
            "search",
            "test query",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&out);
    assert!(
        stdout.contains("alpha memory"),
        "should contain first hit: {stdout}"
    );
}

#[tokio::test]
async fn test_search_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/memories/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "m1", "content": "alpha", "score": 0.9, "tags": [], "memory_type": "message" },
        ])))
        .mount(&server)
        .await;

    let out = clawdb()
        .args([
            "--base-url",
            &server.uri(),
            "--token",
            "tok",
            "--output",
            "json",
            "search",
            "alpha",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Vec<serde_json::Value> =
        serde_json::from_slice(&out).expect("output should be valid JSON array");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["id"], "m1");
}

// ─── status ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_status_ok() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ok": true,
            "components": { "core": true, "vector": true, "branch": true }
        })))
        .mount(&server)
        .await;

    clawdb()
        .args(["--base-url", &server.uri(), "--token", "tok", "status"])
        .assert()
        .success();
}

#[tokio::test]
async fn test_status_degraded() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ok": false,
            "components": { "core": true, "vector": false, "branch": true }
        })))
        .mount(&server)
        .await;

    let out = clawdb()
        .args(["--base-url", &server.uri(), "--token", "tok", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&out);
    assert!(
        stdout.contains("\"ok\": false") || stdout.contains("\"vector\": false"),
        "should include degraded component in output: {stdout}"
    );
}
