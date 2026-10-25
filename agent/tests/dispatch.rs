//! Phase 11.5 dispatch checks: control operations backed by the journal.

use hostnur_agent::contract::{Catalog, OperationRequest, Uuid, parse_request_str};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use hostnur_agent::dispatch::{AgentHandler, mutation_queue};
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
            "operation": "application.rollback",
            "resource": {{"kind": "application", "id": "224065ac-4bc7-4ba4-a06d-8c8bca950c32"}},
            "parameters": {{"release": "00000000-0000-4000-8000-000000000000"}}
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

#[test]
fn service_control_rejects_unmanaged_service() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let mut params = Map::new();
    params.insert("action".into(), json!("restart"));
    let mut req = request("service.control", params);
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "service".to_string(),
        id: "ssh".to_string(),
    });
    let receipt = handler.handle(&req).unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "permission_denied");
}

#[test]
fn register_schedule_validates_and_stores() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let good = || {
        let mut p = Map::new();
        p.insert(
            "schedule".into(),
            json!("33333333-3333-4333-8333-333333333333"),
        );
        p.insert(
            "application".into(),
            json!("224065ac-4bc7-4ba4-a06d-8c8bca950c32"),
        );
        p.insert("program".into(), json!("/usr/bin/php8.4"));
        p.insert("arguments".into(), json!("[\"artisan\",\"schedule:run\"]"));
        p.insert("expression".into(), json!("* * * * *"));
        p.insert("timezone".into(), json!("UTC"));
        request("agent.register_schedule", p)
    };

    let body = handler.handle(&good()).unwrap();
    assert_eq!(body["registered"], true);

    let mut bad = Map::new();
    bad.insert(
        "schedule".into(),
        json!("33333333-3333-4333-8333-333333333333"),
    );
    bad.insert(
        "application".into(),
        json!("224065ac-4bc7-4ba4-a06d-8c8bca950c32"),
    );
    bad.insert("program".into(), json!("relative/bin"));
    bad.insert("expression".into(), json!("* * * * *"));
    bad.insert("timezone".into(), json!("UTC"));
    let body = handler
        .handle(&request("agent.register_schedule", bad))
        .unwrap();
    assert_eq!(body["error"]["code"], "invalid_parameters");
}

#[test]
fn schedule_apply_requires_registration() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let mut req = request("schedule.apply", {
        let mut p = Map::new();
        p.insert(
            "revision".into(),
            json!("44444444-4444-4444-8444-444444444444"),
        );
        p
    });
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "schedule".to_string(),
        id: "33333333-3333-4333-8333-333333333333".to_string(),
    });
    let receipt = handler.handle(&req).unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "resource_not_found");
}

#[test]
fn package_install_rejects_unapproved() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let mut p = Map::new();
    p.insert("package".into(), json!("cowsay"));
    let receipt = handler.handle(&request("package.install", p)).unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "invalid_parameters");
}

#[test]
fn register_database_is_idempotent_and_conflict_aware() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let database = "11111111-2222-4333-8444-555555555555";
    let app = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    let other = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";

    let mut params = Map::new();
    params.insert("database".into(), json!(database));
    params.insert("application".into(), json!(app));

    let body = handler
        .handle(&request("agent.register_database", params.clone()))
        .unwrap();
    assert_eq!(body["registered"], true);
    assert_eq!(body["database"], "hn_db_111111112222");

    // Idempotent re-register.
    let body = handler
        .handle(&request("agent.register_database", params))
        .unwrap();
    assert_eq!(body["registered"], true);

    // Same database id for a different application is a conflict.
    let mut conflict = Map::new();
    conflict.insert("database".into(), json!(database));
    conflict.insert("application".into(), json!(other));
    let body = handler
        .handle(&request("agent.register_database", conflict))
        .unwrap();
    assert_eq!(body["error"]["code"], "conflict");
}

#[test]
fn provision_and_remove_require_registration() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let database = "11111111-2222-4333-8444-555555555555";

    let mut provision = Map::new();
    provision.insert("request".into(), json!(database));
    let receipt = handler
        .handle(&request("database.provision", provision))
        .unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "resource_not_found");

    // Removal is immediate (not task-wrapped).
    let mut remove = Map::new();
    remove.insert("database".into(), json!(database));
    assert_eq!(
        handler
            .handle(&request("agent.remove_database", remove))
            .unwrap()["error"]["code"],
        "resource_not_found"
    );
}

fn unapproved_install() -> OperationRequest {
    let mut parameters = Map::new();
    parameters.insert("package".into(), json!("cowsay"));
    request("package.install", parameters)
}

#[test]
fn queued_mutation_returns_before_running_and_is_enqueued_once() {
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    let (jobs, queue) = mutation_queue();
    handler.configure_jobs(jobs);

    let receipt = handler.handle(&unapproved_install()).unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();
    // Nothing has run yet: the serve loop only accepted and queued it.
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "accepted");

    // A retry with the same idempotency key reuses the execution.
    let again = handler.handle(&unapproved_install()).unwrap();
    assert_eq!(again["execution_id"], receipt["execution_id"]);

    let job = queue.try_recv().expect("one job queued");
    assert!(queue.try_recv().is_err());
    handler.execute_job(job);

    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "invalid_parameters");
    let events: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["message"].as_str().unwrap())
        .collect();
    assert_eq!(events, ["Started.", "Failed."]);
}

#[test]
fn job_cancelled_while_queued_never_runs() {
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    let (jobs, queue) = mutation_queue();
    handler.configure_jobs(jobs);

    let receipt = handler.handle(&unapproved_install()).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap();
    handler.handle(&cancel_request(execution_id)).unwrap();
    handler.execute_job(queue.try_recv().unwrap());

    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "cancelled");
    assert!(body["result"].is_null());
    let last = body["events"].as_array().unwrap().last().unwrap();
    assert!(
        last["message"]
            .as_str()
            .unwrap()
            .starts_with("Cancelled before it started")
    );
}

#[test]
fn job_whose_deadline_passed_in_the_queue_times_out_without_running() {
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    let (jobs, queue) = mutation_queue();
    handler.configure_jobs(jobs);

    let mut req = unapproved_install();
    req.timeout_seconds = Some(1);
    let receipt = handler.handle(&req).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    handler.execute_job(queue.try_recv().unwrap());

    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "timed_out");
    assert_eq!(body["result"]["error"]["code"], "timeout");
}

#[test]
fn full_queue_refuses_with_retryable_agent_unavailable() {
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    let (jobs, _queue) = std::sync::mpsc::sync_channel(0);
    handler.configure_jobs(jobs);

    let receipt = handler.handle(&unapproved_install()).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let body = handler.handle(&inspect_request(execution_id)).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "agent_unavailable");
    assert_eq!(body["result"]["error"]["retryable"], true);
}

#[test]
fn worker_thread_completes_jobs_over_its_own_journal_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.db");
    let journal = Journal::open(&path).unwrap();
    let mut handler = AgentHandler::new(&journal);
    let (jobs, queue) = mutation_queue();
    handler.configure_jobs(jobs);

    let shutdown = Arc::new(AtomicBool::new(false));
    let worker = {
        let shutdown = Arc::clone(&shutdown);
        let path = path.clone();
        std::thread::spawn(move || {
            let journal = Journal::open(&path).unwrap();
            AgentHandler::new(&journal).serve_jobs(&queue, &shutdown);
        })
    };

    let receipt = handler.handle(&unapproved_install()).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap();
    let started = Instant::now();
    let body = loop {
        let body = handler.handle(&inspect_request(execution_id)).unwrap();
        if body["status"] != "accepted" && body["status"] != "running" {
            break body;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "worker never finished"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "invalid_parameters");

    shutdown.store(true, Ordering::Relaxed);
    worker.join().unwrap();
}
