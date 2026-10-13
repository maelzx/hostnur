//! Phase 11.5 dispatch checks: control operations backed by the journal.

use hostnur_agent::contract::{Catalog, OperationRequest, Uuid, parse_request_str};
use hostnur_agent::dispatch::AgentHandler;
use hostnur_agent::journal::Journal;
use hostnur_agent::transport::RequestHandler;
use serde_json::{Map, Value, json};

const REQUEST_ID: &str = "6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7";
const IDEMPOTENCY_KEY: &str = "cf36a770-2abf-42e6-83cf-a0f042bcfc3c";

fn request(operation: &str, parameters: Map<String, Value>) -> OperationRequest {
    OperationRequest {
        protocol_version: 1,
        request_id: Uuid::parse(REQUEST_ID).unwrap(),
        idempotency_key: Uuid::parse(IDEMPOTENCY_KEY).unwrap(),
        operation: operation.to_string(),
        resource: None,
        parameters,
        timeout_seconds: None,
    }
}

fn inspect_request(execution_id: &str) -> OperationRequest {
    let mut parameters = Map::new();
    parameters.insert("execution_id".into(), json!(execution_id));
    request("agent.inspect", parameters)
}

fn cancel_request(execution_id: &str) -> OperationRequest {
    let mut parameters = Map::new();
    parameters.insert("execution_id".into(), json!(execution_id));
    request("agent.cancel", parameters)
}

fn accepted_execution(journal: &Journal) -> String {
    let text = format!(
        r#"{{
            "protocol_version": 1,
            "request_id": "{REQUEST_ID}",
            "idempotency_key": "{IDEMPOTENCY_KEY}",
            "operation": "server.snapshot",
            "parameters": {{}}
        }}"#
    );
    let parsed = parse_request_str(&text, &Catalog::embedded()).unwrap();
    journal
        .accept(&parsed)
        .unwrap()
        .execution_id
        .as_str()
        .to_string()
}

#[test]
fn capabilities_reports_real_mode_and_supported_operations() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let body = handler
        .handle(&request("agent.capabilities", Map::new()))
        .unwrap();
    assert_eq!(body["mode"], "real");
    assert_eq!(body["available"], true);
    let operations = body["operations"].as_array().unwrap();
    assert!(operations.iter().any(|op| op == "agent.inspect"));
    assert!(operations.iter().any(|op| op == "server.snapshot"));
}

#[test]
fn server_snapshot_is_read_only_real_mode() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let body = handler
        .handle(&request("server.snapshot", Map::new()))
        .unwrap();
    assert_eq!(body["mode"], "real");
    assert_eq!(body["available"], true);
    // Metrics are read live from the host; the exact count is host-dependent.
    assert!(body["metrics"].is_array());
    assert!(body["metrics"].as_array().unwrap().len() <= 100);
}

#[test]
fn inspect_returns_accepted_execution() {
    let journal = Journal::open_in_memory().unwrap();
    let execution_id = accepted_execution(&journal);
    let handler = AgentHandler::new(&journal);

    let body = handler.handle(&inspect_request(&execution_id)).unwrap();
    assert_eq!(body["status"], "accepted");
    assert_eq!(body["execution_id"], execution_id);
}

#[test]
fn inspect_unknown_execution_reports_execution_unknown() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let body = handler
        .handle(&inspect_request("d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a"))
        .unwrap();
    assert_eq!(body["error"]["code"], "execution_unknown");
}

#[test]
fn cancel_then_inspect_reflects_terminal_state() {
    let journal = Journal::open_in_memory().unwrap();
    let execution_id = accepted_execution(&journal);
    let handler = AgentHandler::new(&journal);

    let first = handler.handle(&cancel_request(&execution_id)).unwrap();
    assert_eq!(first["status"], "requested");

    let after = handler.handle(&inspect_request(&execution_id)).unwrap();
    assert_eq!(after["status"], "cancelled");

    let second = handler.handle(&cancel_request(&execution_id)).unwrap();
    assert_eq!(second["status"], "already_finished");
}

#[test]
fn unimplemented_mutation_returns_unsupported_operation() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let text = format!(
        r#"{{
            "protocol_version": 1,
            "request_id": "{REQUEST_ID}",
            "idempotency_key": "{IDEMPOTENCY_KEY}",
            "operation": "application.restart",
            "resource": {{"kind": "application", "id": "224065ac-4bc7-4ba4-a06d-8c8bca950c32"}},
            "parameters": {{}}
        }}"#
    );
    let parsed = parse_request_str(&text, &Catalog::embedded()).unwrap();
    let body = handler.handle(&parsed).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_operation");
}

fn register_params(url: &str, host: &str) -> Map<String, Value> {
    let mut parameters = Map::new();
    parameters.insert(
        "application".into(),
        json!("224065ac-4bc7-4ba4-a06d-8c8bca950c32"),
    );
    parameters.insert("repository_url".into(), json!(url));
    parameters.insert("server_name".into(), json!(host));
    parameters.insert("provider".into(), json!("php-laravel"));
    parameters.insert("runtime".into(), json!("php"));
    parameters.insert("runtime_version".into(), json!("8.3"));
    parameters
}

#[test]
fn register_application_stores_and_rejects() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let body = handler
        .handle(&request(
            "agent.register_application",
            register_params("https://github.com/maelzx/hostnur.git", "app.example.com"),
        ))
        .unwrap();
    assert_eq!(body["registered"], true);

    let stored = journal
        .application_source(&Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored.server_name, "app.example.com");

    // Loopback URL refused.
    let body = handler
        .handle(&request(
            "agent.register_application",
            register_params("https://127.0.0.1/repo.git", "app.example.com"),
        ))
        .unwrap();
    assert_eq!(body["error"]["code"], "invalid_parameters");

    // Bad hostname refused.
    let body = handler
        .handle(&request(
            "agent.register_application",
            register_params("https://github.com/x/y.git", "bad host"),
        ))
        .unwrap();
    assert_eq!(body["error"]["code"], "invalid_parameters");
}
