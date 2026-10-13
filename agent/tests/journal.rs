//! Phase 11.3 durable execution journal and resource registry checks.

use hostnur_agent::contract::{
    Catalog, OperationRequest, OperationStatus, Uuid, parse_request_str,
};
use hostnur_agent::journal::{Journal, JournalError, RegisteredResource};
use tempfile::TempDir;

const REQUEST_ID: &str = "6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7";
const IDEMPOTENCY_KEY: &str = "cf36a770-2abf-42e6-83cf-a0f042bcfc3c";

fn snapshot_request(idempotency_key: &str) -> OperationRequest {
    let text = format!(
        r#"{{
            "protocol_version": 1,
            "request_id": "{REQUEST_ID}",
            "idempotency_key": "{idempotency_key}",
            "operation": "server.snapshot",
            "parameters": {{}}
        }}"#
    );
    parse_request_str(&text, &Catalog::embedded()).expect("valid request")
}

fn restart_request(idempotency_key: &str) -> OperationRequest {
    let text = format!(
        r#"{{
            "protocol_version": 1,
            "request_id": "{REQUEST_ID}",
            "idempotency_key": "{idempotency_key}",
            "operation": "application.restart",
            "resource": {{"kind": "application", "id": "224065ac-4bc7-4ba4-a06d-8c8bca950c32"}},
            "parameters": {{}}
        }}"#
    );
    parse_request_str(&text, &Catalog::embedded()).expect("valid request")
}

#[test]
fn duplicate_intent_reuses_one_execution() {
    let journal = Journal::open_in_memory().unwrap();
    let first = journal.accept(&snapshot_request(IDEMPOTENCY_KEY)).unwrap();
    assert!(!first.reused);
    assert_eq!(first.status, OperationStatus::Accepted);

    let second = journal.accept(&snapshot_request(IDEMPOTENCY_KEY)).unwrap();
    assert!(second.reused);
    assert_eq!(first.execution_id, second.execution_id);
    assert_eq!(first.accepted_at, second.accepted_at);
}

#[test]
fn same_key_with_different_intent_conflicts() {
    let journal = Journal::open_in_memory().unwrap();
    journal.accept(&snapshot_request(IDEMPOTENCY_KEY)).unwrap();

    let error = journal
        .accept(&restart_request(IDEMPOTENCY_KEY))
        .unwrap_err();
    assert!(matches!(error, JournalError::Conflict));
}

#[test]
fn acceptance_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("agent.db");

    let execution_id = {
        let journal = Journal::open(&path).unwrap();
        let acceptance = journal.accept(&snapshot_request(IDEMPOTENCY_KEY)).unwrap();
        acceptance.execution_id
    };

    // Simulate an agent restart: reopen and submit the same intent again.
    let journal = Journal::open(&path).unwrap();
    let repeated = journal.accept(&snapshot_request(IDEMPOTENCY_KEY)).unwrap();
    assert!(repeated.reused);
    assert_eq!(repeated.execution_id, execution_id);
}

#[test]
fn inspect_unknown_execution_is_none() {
    let journal = Journal::open_in_memory().unwrap();
    let unknown = Uuid::parse("d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a").unwrap();
    assert!(journal.inspect(&unknown).unwrap().is_none());
}

#[test]
fn events_are_ordered_and_cursored() {
    let journal = Journal::open_in_memory().unwrap();
    let execution_id = journal
        .accept(&snapshot_request(IDEMPOTENCY_KEY))
        .unwrap()
        .execution_id;

    journal.append_event(&execution_id, "accepted").unwrap();
    journal.append_event(&execution_id, "running").unwrap();
    journal.append_event(&execution_id, "done").unwrap();

    let all = journal.events_after(&execution_id, 0).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].sequence, 1);
    assert_eq!(all[2].sequence, 3);
    assert_eq!(all[2].message, "done");

    let after_first = journal.events_after(&execution_id, 1).unwrap();
    assert_eq!(after_first.len(), 2);
    assert_eq!(after_first[0].sequence, 2);

    let view = journal.inspect(&execution_id).unwrap().unwrap();
    assert_eq!(view.events.len(), 3);
}

#[test]
fn status_updates_are_persisted() {
    let journal = Journal::open_in_memory().unwrap();
    let execution_id = journal
        .accept(&snapshot_request(IDEMPOTENCY_KEY))
        .unwrap()
        .execution_id;

    journal
        .set_status(&execution_id, OperationStatus::Succeeded, Some(100))
        .unwrap();

    let view = journal.inspect(&execution_id).unwrap().unwrap();
    assert_eq!(view.status, OperationStatus::Succeeded);
    assert_eq!(view.progress, Some(100));
    assert!(view.status.is_terminal());
}

#[test]
fn registry_refuses_to_adopt_an_existing_identity() {
    let journal = Journal::open_in_memory().unwrap();
    let resource = RegisteredResource {
        resource_id: Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap(),
        kind: "application".to_string(),
        provider: Some("php-laravel".to_string()),
        owner_uid: Some(1001),
        owner_gid: Some(1001),
        revision: Some("rev-1".to_string()),
        lifecycle_state: "reserved".to_string(),
    };

    assert!(journal.register_resource(&resource).unwrap());
    // A second registration must not create a duplicate or overwrite state.
    assert!(!journal.register_resource(&resource).unwrap());

    let stored = journal.resource(&resource.resource_id).unwrap().unwrap();
    assert_eq!(stored, resource);

    let unknown = Uuid::parse("d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a").unwrap();
    assert!(journal.resource(&unknown).unwrap().is_none());
}
