//! Phase 11.5 dispatch checks: control operations backed by the journal.

use hostnur_agent::contract::{Catalog, OperationRequest, Uuid, parse_request_str};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use hostnur_agent::deploy::DeployRuntime;
use hostnur_agent::dispatch::{AgentHandler, mutation_queue};
use hostnur_agent::git::GitRunner;
use hostnur_agent::journal::Journal;
use hostnur_agent::provider::{FpmManager, NginxManager, RuntimePolicy};
use hostnur_agent::provision::SystemAccounts;
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
fn rollback_without_a_deploy_runtime_is_unsupported() {
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

fn self_signed(dir: &std::path::Path, host: &str) -> (String, String) {
    let key = dir.join("key.pem");
    let cert = dir.join("cert.pem");
    let status = std::process::Command::new("/usr/bin/openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args(["-days", "30", "-subj", &format!("/CN={host}")])
        .args(["-addext", &format!("subjectAltName=DNS:{host}")])
        .status()
        .expect("openssl available");
    assert!(status.success());
    (
        std::fs::read_to_string(cert).unwrap(),
        std::fs::read_to_string(key).unwrap(),
    )
}

fn register_certificate(
    handler: &AgentHandler<'_>,
    hostname: &str,
    material: Option<(&str, &str)>,
) -> Value {
    let mut parameters = Map::new();
    parameters.insert("certificate".into(), json!(DOMAIN_ID));
    parameters.insert("hostname".into(), json!(hostname));
    parameters.insert(
        "application".into(),
        json!("224065ac-4bc7-4ba4-a06d-8c8bca950c32"),
    );
    if let Some((pem, key)) = material {
        parameters.insert("certificate_pem".into(), json!(pem));
        parameters.insert("private_key".into(), json!(key));
    }
    handler
        .handle(&request("agent.register_certificate", parameters))
        .unwrap()
}

fn install_certificate(handler: &AgentHandler<'_>) -> Value {
    let mut parameters = Map::new();
    parameters.insert("certificate".into(), json!("panel-certificate-1"));
    let mut req = request("certificate.install", parameters);
    req.idempotency_key = hostnur_agent::ids::new_uuid_v4();
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "domain".to_string(),
        id: DOMAIN_ID.to_string(),
    });
    let receipt = handler.handle(&req).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap().to_string();
    handler.handle(&inspect_request(&execution_id)).unwrap()
}

const DOMAIN_ID: &str = "7a0c5e1d-3b2f-4c8a-9e6d-1f2a3b4c5d6e";

#[test]
fn supplied_certificate_material_must_come_together_and_be_pem() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);

    let mut parameters = Map::new();
    parameters.insert("certificate".into(), json!(DOMAIN_ID));
    parameters.insert("hostname".into(), json!("app.example.com"));
    parameters.insert(
        "certificate_pem".into(),
        json!("-----BEGIN CERTIFICATE-----"),
    );
    let body = handler
        .handle(&request("agent.register_certificate", parameters))
        .unwrap();
    assert_eq!(body["error"]["code"], "invalid_parameters");

    let body = register_certificate(&handler, "app.example.com", Some(("not a pem", "nor this")));
    assert_eq!(body["error"]["code"], "invalid_parameters");
}

#[test]
fn supplied_certificate_is_staged_root_only_and_replaced_on_reregistration() {
    use std::os::unix::fs::PermissionsExt;

    let secrets = tempfile::tempdir().unwrap();
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    handler.configure_secrets_dir(secrets.path().to_path_buf());
    let (pem, key) = self_signed(secrets.path(), "app.example.com");

    let body = register_certificate(&handler, "app.example.com", Some((&pem, &key)));
    assert_eq!(body["registered"], true);

    let staged = secrets.path().join("certificates").join(DOMAIN_ID);
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&staged), 0o700);
    assert_eq!(mode(&staged.join("fullchain.pem")), 0o600);
    assert_eq!(mode(&staged.join("privkey.pem")), 0o600);
    assert_eq!(
        std::fs::read_to_string(staged.join("privkey.pem")).unwrap(),
        key
    );

    // Registering again replaces the staged pair instead of failing.
    let body = register_certificate(&handler, "app.example.com", Some((&pem, &key)));
    assert_eq!(body["registered"], true);
}

#[test]
fn certificate_install_requires_staged_material() {
    let secrets = tempfile::tempdir().unwrap();
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    handler.configure_secrets_dir(secrets.path().to_path_buf());

    let body = install_certificate(&handler);
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "resource_not_found");

    register_certificate(&handler, "app.example.com", None);
    let body = install_certificate(&handler);
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "resource_not_found");
    assert!(
        body["result"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("staged")
    );
}

#[test]
fn certificate_install_rejects_material_for_another_hostname_before_touching_the_host() {
    let secrets = tempfile::tempdir().unwrap();
    let journal = Journal::open_in_memory().unwrap();
    let mut handler = AgentHandler::new(&journal);
    handler.configure_secrets_dir(secrets.path().to_path_buf());
    let (pem, key) = self_signed(secrets.path(), "other.example.com");

    register_certificate(&handler, "app.example.com", Some((&pem, &key)));
    let body = install_certificate(&handler);
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "invalid_parameters");
    assert!(
        body["result"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not cover")
    );
}

fn deploy_runtime(db_path: std::path::PathBuf, root: &std::path::Path) -> DeployRuntime {
    DeployRuntime {
        db_path,
        apps_root: root.join("apps"),
        accounts: SystemAccounts::default(),
        git: GitRunner::default(),
        nginx: NginxManager::default(),
        fpm: FpmManager::default(),
        allow_protocol: "https".to_string(),
        secrets_dir: root.join("secrets"),
    }
}

#[test]
fn rollback_is_advertised_with_deploy_and_reports_an_unknown_release() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.db");
    let journal = Journal::open(&path).unwrap();
    let handler = AgentHandler::with_deploy(
        &journal,
        deploy_runtime(path.clone(), dir.path()),
        RuntimePolicy::default(),
    );

    let capabilities = handler
        .handle(&request("agent.capabilities", Map::new()))
        .unwrap();
    let operations = capabilities["operations"].as_array().unwrap();
    assert!(operations.contains(&json!("application.rollback")));

    let mut parameters = Map::new();
    parameters.insert(
        "release".into(),
        json!("00000000-0000-4000-8000-000000000000"),
    );
    let mut req = request("application.rollback", parameters);
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "application".to_string(),
        id: "224065ac-4bc7-4ba4-a06d-8c8bca950c32".to_string(),
    });
    let receipt = handler.handle(&req).unwrap();
    assert_eq!(receipt["status"], "accepted");
    let execution_id = receipt["execution_id"].as_str().unwrap();

    let started = Instant::now();
    let body = loop {
        let body = handler.handle(&inspect_request(execution_id)).unwrap();
        if body["status"] != "accepted" && body["status"] != "running" {
            break body;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "rollback never finished"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(body["status"], "failed");
    assert_eq!(body["result"]["error"]["code"], "resource_not_found");
}

#[test]
fn rollback_rejects_a_non_uuid_release() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.db");
    let journal = Journal::open(&path).unwrap();
    let handler = AgentHandler::with_deploy(
        &journal,
        deploy_runtime(path.clone(), dir.path()),
        RuntimePolicy::default(),
    );
    let mut parameters = Map::new();
    parameters.insert("release".into(), json!("../../etc"));
    let mut req = request("application.rollback", parameters);
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "application".to_string(),
        id: "224065ac-4bc7-4ba4-a06d-8c8bca950c32".to_string(),
    });
    let body = handler.handle(&req).unwrap();
    assert_eq!(body["error"]["code"], "invalid_parameters");
}

struct ControlFixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    identity: String,
}

fn control_fixture() -> ControlFixture {
    use age::secrecy::ExposeSecret;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("panel/database")).unwrap();
    let panel = rusqlite::Connection::open(root.join("panel/database/database.sqlite")).unwrap();
    panel
        .execute_batch(
            "CREATE TABLE users (email TEXT); INSERT INTO users VALUES ('a@example.com');",
        )
        .unwrap();
    std::fs::write(root.join("panel/.env"), "APP_KEY=base64:test\n").unwrap();
    std::fs::create_dir_all(root.join("etc")).unwrap();
    std::fs::write(root.join("etc/agent.toml"), "version = 1\n").unwrap();
    std::os::unix::fs::symlink("/etc/passwd", root.join("etc/linked")).unwrap();
    std::fs::create_dir_all(root.join("secrets")).unwrap();
    std::fs::write(root.join("secrets/db-password"), "pw").unwrap();

    let identity = age::x25519::Identity::generate();
    ControlFixture {
        _dir: dir,
        root,
        identity: identity.to_string().expose_secret().to_string(),
    }
}

fn control_handler<'j>(journal: &'j Journal, fixture: &ControlFixture) -> AgentHandler<'j> {
    let identity: age::x25519::Identity = fixture.identity.parse().unwrap();
    let mut handler = AgentHandler::new(journal);
    handler.configure_backup(
        Some(identity.to_public().to_string()),
        fixture.root.join("backups"),
    );
    handler.configure_control_paths(fixture.root.join("panel"), fixture.root.join("etc"));
    handler.configure_secrets_dir(fixture.root.join("secrets"));
    handler
}

fn create_backup(handler: &AgentHandler<'_>, scope: &str) -> (String, Value) {
    create_backup_with(handler, scope, Map::new())
}

fn create_backup_with(
    handler: &AgentHandler<'_>,
    scope: &str,
    create_parameters: Map<String, Value>,
) -> (String, Value) {
    let backup = hostnur_agent::ids::new_uuid_v4();
    let mut parameters = Map::new();
    parameters.insert("backup".into(), json!(backup.as_str()));
    parameters.insert("scope".into(), json!(scope));
    let registered = handler
        .handle(&request("agent.register_backup", parameters))
        .unwrap();
    assert_eq!(registered["registered"], true);

    let mut req = request("backup.create", create_parameters);
    req.idempotency_key = hostnur_agent::ids::new_uuid_v4();
    req.resource = Some(hostnur_agent::contract::ResourceRef {
        kind: "backup".to_string(),
        id: backup.as_str().to_string(),
    });
    let receipt = handler.handle(&req).unwrap();
    let execution_id = receipt["execution_id"].as_str().unwrap().to_string();
    (
        backup.as_str().to_string(),
        handler.handle(&inspect_request(&execution_id)).unwrap(),
    )
}

/// Decrypt a sealed artifact and list its archive member paths.
fn sealed_members(path: &std::path::Path, identity: &str) -> Vec<String> {
    let sealed = std::fs::read(path).unwrap();
    let plaintext = hostnur_agent::backup::open(identity, &sealed).unwrap();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&plaintext[..]));
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .display()
                .to_string()
                .trim_end_matches('/')
                .to_string()
        })
        .collect()
}

#[test]
fn control_state_backup_captures_the_control_plane_and_cleans_up() {
    let fixture = control_fixture();
    let journal = Journal::open(&fixture.root.join("agent.db")).unwrap();
    let handler = control_handler(&journal, &fixture);

    let (backup, body) = create_backup(&handler, "control_state");
    assert_eq!(body["status"], "succeeded", "{body}");
    assert_eq!(body["result"]["scope"], "control_state");
    assert_eq!(body["result"]["consistency"], "snapshot");
    assert_eq!(
        body["result"]["contents"]["control"],
        json!([
            "panel_database",
            "panel_env",
            "agent_journal",
            "agent_secrets",
            "agent_config"
        ])
    );

    let backups = fixture.root.join("backups");
    let members = sealed_members(
        &backups.join(format!("{backup}.tar.gz.age")),
        &fixture.identity,
    );
    for expected in [
        "control/panel/database.sqlite",
        "control/panel/.env",
        "control/agent/agent.db",
        "control/agent/secrets/db-password",
        "control/etc-hostnur/agent.toml",
    ] {
        assert!(
            members.iter().any(|member| member == expected),
            "{expected} missing from {members:?}"
        );
    }
    // Links are never followed into the archive.
    assert!(!members.iter().any(|member| member.ends_with("linked")));

    // Only the sealed artifact and its manifest remain: no staging, no plaintext.
    let mut left: Vec<String> = std::fs::read_dir(&backups)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            format!("{backup}.manifest.json"),
            format!("{backup}.tar.gz.age")
        ]
    );
}

#[test]
fn control_state_snapshot_of_the_panel_database_is_readable() {
    let fixture = control_fixture();
    let journal = Journal::open(&fixture.root.join("agent.db")).unwrap();
    let handler = control_handler(&journal, &fixture);
    let (backup, body) = create_backup(&handler, "control_state");
    assert_eq!(body["status"], "succeeded");

    let sealed = std::fs::read(fixture.root.join(format!("backups/{backup}.tar.gz.age"))).unwrap();
    let plaintext = hostnur_agent::backup::open(&fixture.identity, &sealed).unwrap();
    let out = tempfile::tempdir().unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(&plaintext[..]))
        .unpack(out.path())
        .unwrap();
    let restored =
        rusqlite::Connection::open(out.path().join("control/panel/database.sqlite")).unwrap();
    let email: String = restored
        .query_row("SELECT email FROM users", [], |row| row.get(0))
        .unwrap();
    assert_eq!(email, "a@example.com");
}

#[test]
fn server_backup_includes_control_state_and_a_databases_directory() {
    let fixture = control_fixture();
    let journal = Journal::open(&fixture.root.join("agent.db")).unwrap();
    let handler = control_handler(&journal, &fixture);

    let (backup, body) = create_backup(&handler, "server");
    assert_eq!(body["status"], "succeeded", "{body}");
    assert_eq!(body["result"]["contents"]["databases"], 0);
    assert_eq!(body["result"]["contents"]["applications"], 0);

    let members = sealed_members(
        &fixture.root.join(format!("backups/{backup}.tar.gz.age")),
        &fixture.identity,
    );
    assert!(members.iter().any(|member| member == "control/panel/.env"));
    assert!(members.iter().any(|member| member == "databases"));
}

fn policy_parameters(policy: &str, retention_days: Value) -> Map<String, Value> {
    let mut parameters = Map::new();
    parameters.insert("policy".into(), json!(policy));
    parameters.insert("retention_days".into(), retention_days);
    parameters
}

#[test]
fn retention_prunes_only_the_policys_expired_local_artifacts() {
    let fixture = control_fixture();
    let journal = Journal::open(&fixture.root.join("agent.db")).unwrap();
    let handler = control_handler(&journal, &fixture);
    let policy = "3f2b1c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
    let other_policy = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";
    let backups = fixture.root.join("backups");
    let age_by = |id: &str, days: u64| {
        let old = std::time::SystemTime::now() - Duration::from_secs(days * 86_400);
        std::fs::File::options()
            .write(true)
            .open(backups.join(format!("{id}.tar.gz.age")))
            .unwrap()
            .set_modified(old)
            .unwrap();
    };

    let (expired, _) = create_backup_with(
        &handler,
        "control_state",
        policy_parameters(policy, json!(7)),
    );
    let (recent, _) = create_backup_with(
        &handler,
        "control_state",
        policy_parameters(policy, json!(7)),
    );
    let (foreign, _) = create_backup_with(
        &handler,
        "control_state",
        policy_parameters(other_policy, json!(7)),
    );
    age_by(&expired, 10);
    age_by(&recent, 3);
    age_by(&foreign, 10);

    let (latest, body) = create_backup_with(
        &handler,
        "control_state",
        policy_parameters(policy, json!(7)),
    );
    assert_eq!(body["status"], "succeeded", "{body}");
    assert_eq!(body["result"]["pruned"], json!([expired]));

    assert!(!backups.join(format!("{expired}.tar.gz.age")).exists());
    assert!(!backups.join(format!("{expired}.manifest.json")).exists());
    for kept in [&recent, &foreign, &latest] {
        assert!(
            backups.join(format!("{kept}.tar.gz.age")).exists(),
            "{kept} was pruned"
        );
    }
}

#[test]
fn retention_days_out_of_range_is_rejected_before_any_work() {
    let fixture = control_fixture();
    let journal = Journal::open(&fixture.root.join("agent.db")).unwrap();
    let handler = control_handler(&journal, &fixture);

    for days in [json!(0), json!(3651), json!(-1)] {
        let (backup, body) = create_backup_with(
            &handler,
            "control_state",
            policy_parameters("3f2b1c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d", days),
        );
        assert_eq!(body["status"], "failed");
        assert_eq!(body["result"]["error"]["code"], "invalid_parameters");
        assert!(
            !fixture
                .root
                .join(format!("backups/{backup}.tar.gz.age"))
                .exists()
        );
    }
}

#[test]
fn registration_stores_a_normalized_health_path_and_rejects_unsafe_ones() {
    let journal = Journal::open_in_memory().unwrap();
    let handler = AgentHandler::new(&journal);
    let app = Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap();
    let register = |health: Option<&str>| {
        let mut parameters =
            register_params("https://github.com/example/app.git", "app.example.com");
        if let Some(path) = health {
            parameters.insert("health_path".into(), json!(path));
        }
        handler
            .handle(&request("agent.register_application", parameters))
            .unwrap()
    };

    assert_eq!(register(None)["registered"], true);
    assert_eq!(
        journal
            .application_source(&app)
            .unwrap()
            .unwrap()
            .health_path,
        "/"
    );

    assert_eq!(register(Some("up"))["registered"], true);
    assert_eq!(
        journal
            .application_source(&app)
            .unwrap()
            .unwrap()
            .health_path,
        "/up"
    );

    assert_eq!(register(Some("/health/ready"))["registered"], true);
    assert_eq!(
        journal
            .application_source(&app)
            .unwrap()
            .unwrap()
            .health_path,
        "/health/ready"
    );

    for unsafe_path in [
        "../etc/passwd",
        "up?x=1",
        "up\r\nHost: evil",
        "http://evil/",
        "up#frag",
    ] {
        let body = register(Some(unsafe_path));
        assert_eq!(
            body["error"]["code"], "invalid_parameters",
            "{unsafe_path} accepted"
        );
    }
    assert_eq!(
        journal
            .application_source(&app)
            .unwrap()
            .unwrap()
            .health_path,
        "/health/ready"
    );
}
