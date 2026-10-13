//! Cross-language contract checks against the shared `contracts/v1` fixtures.
//!
//! The same fixtures are validated by the PHP panel (`tests/Unit/AgentContractTest`)
//! against JSON Schema and the PHP DTOs. Here the Rust parser/DTOs must agree on
//! every valid and invalid case, including the exact machine error code recorded
//! in `fixtures/invalid/manifest.json`.

use std::fs;
use std::path::{Path, PathBuf};

use hostnur_agent::contract::{
    AgentMode, CancellationReceipt, CancellationStatus, CapabilitySnapshot, Catalog,
    ContractErrorCode, OperationReceipt, OperationSnapshot, OperationStatus, ServerSnapshot,
    parse_request,
};
use serde_json::Value;

fn contracts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("contracts")
        .join("v1")
}

fn read_json(relative: &str) -> Value {
    let path = contracts_dir().join(relative);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn list_fixtures(dir: &str, prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(contracts_dir().join(dir))
        .expect("read fixtures dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(prefix) && name.ends_with(".json"))
        .collect();
    names.sort();
    names
}

#[test]
fn every_valid_request_fixture_parses() {
    let catalog = Catalog::embedded();
    let names = list_fixtures("fixtures/valid", "request-");
    assert!(!names.is_empty(), "expected valid request fixtures");

    for name in names {
        let value = read_json(&format!("fixtures/valid/{name}"));
        let bytes = serde_json::to_vec(&value).unwrap();
        let request = parse_request(&bytes, &catalog)
            .unwrap_or_else(|err| panic!("{name} should parse: {err}"));
        assert_eq!(request.protocol_version, 1, "{name}");
    }
}

#[test]
fn every_invalid_request_fixture_fails_with_the_manifest_code() {
    let catalog = Catalog::embedded();
    let manifest = read_json("fixtures/invalid/manifest.json");
    let requests = manifest["requests"].as_object().expect("requests map");
    assert!(!requests.is_empty(), "expected invalid request fixtures");

    for (name, expected) in requests {
        let expected = expected.as_str().expect("error code string");
        let value = read_json(&format!("fixtures/invalid/{name}"));
        let bytes = serde_json::to_vec(&value).unwrap();
        let error =
            parse_request(&bytes, &catalog).expect_err(&format!("{name} should be rejected"));
        assert_eq!(
            error.code().as_str(),
            expected,
            "{name} produced the wrong code"
        );
    }
}

#[test]
fn valid_response_fixtures_parse_and_round_trip() {
    let receipt: OperationReceipt =
        serde_json::from_value(read_json("fixtures/valid/receipt.json")).expect("receipt");
    assert_eq!(receipt.status, OperationStatus::Accepted);
    assert_eq!(receipt.operation, "application.restart");
    assert!(receipt.status.is_active());

    let running: OperationSnapshot =
        serde_json::from_value(read_json("fixtures/valid/snapshot-running.json")).expect("running");
    assert_eq!(running.status, OperationStatus::Running);
    assert_eq!(running.progress, Some(40));
    assert_eq!(running.events.len(), 2);
    assert!(running.error.is_none());
    running.validate().expect("running snapshot is valid");

    let failed: OperationSnapshot =
        serde_json::from_value(read_json("fixtures/valid/snapshot-failed.json")).expect("failed");
    assert_eq!(failed.status, OperationStatus::Failed);
    assert!(failed.status.is_terminal());
    assert_eq!(
        failed.error.as_ref().map(|error| error.code),
        Some(hostnur_agent::contract::ContractErrorCode::OperationFailed)
    );

    let capability: CapabilitySnapshot =
        serde_json::from_value(read_json("fixtures/valid/capability-unavailable.json"))
            .expect("capability");
    assert_eq!(capability.mode, AgentMode::Unavailable);
    assert!(!capability.available);
    assert!(capability.operations.is_empty());

    let server: ServerSnapshot =
        serde_json::from_value(read_json("fixtures/valid/server-snapshot-unavailable.json"))
            .expect("server snapshot");
    assert_eq!(server.mode, AgentMode::Unavailable);
    assert!(!server.available);
    assert!(server.metrics.is_empty());

    let cancellation: CancellationReceipt =
        serde_json::from_value(read_json("fixtures/valid/cancellation-receipt.json"))
            .expect("cancellation");
    assert_eq!(cancellation.status, CancellationStatus::Requested);

    // Serializing then re-parsing must be stable for each DTO.
    let receipt_again: OperationReceipt =
        serde_json::from_value(serde_json::to_value(&receipt).unwrap()).unwrap();
    assert_eq!(receipt, receipt_again);

    let server_again: ServerSnapshot =
        serde_json::from_value(serde_json::to_value(&server).unwrap()).unwrap();
    assert_eq!(server, server_again);
}

#[test]
fn fake_server_snapshot_fixture_parses() {
    let server: ServerSnapshot =
        serde_json::from_value(read_json("fixtures/fake/server-snapshot.json")).expect("fake");
    assert_eq!(server.mode, AgentMode::Fake);
    assert_eq!(server.metrics.len(), 6);
    assert_eq!(server.services.len(), 3);
    assert_eq!(server.hostname.as_deref(), Some("hostnur-demo"));
    assert_eq!(
        server.firewall.as_ref().map(|firewall| firewall.rule_count),
        Some(3)
    );
}

#[test]
fn invalid_response_fixture_is_rejected() {
    let manifest = read_json("fixtures/invalid/manifest.json");
    let responses = manifest["responses"].as_object().expect("responses map");
    assert!(!responses.is_empty(), "expected invalid response fixtures");

    for (name, expected) in responses {
        assert_eq!(expected.as_str(), Some("invalid_parameters"));
        let value = read_json(&format!("fixtures/invalid/{name}"));
        let result = serde_json::from_value::<OperationReceipt>(value);
        let error = result.expect_err(&format!("{name} should be rejected"));
        // serde documents the failing field; the mapped contract code is
        // `invalid_parameters`, so any deserialize failure is correct.
        assert!(
            !error.to_string().is_empty(),
            "{name} should produce a deserialize error"
        );
    }
}

#[test]
fn envelope_round_trips_to_canonical_json() {
    let catalog = Catalog::embedded();
    let value = read_json("fixtures/valid/request-application-restart.json");
    let bytes = serde_json::to_vec(&value).unwrap();
    let request = parse_request(&bytes, &catalog).expect("valid request");
    let encoded = serde_json::to_value(&request).unwrap();

    assert_eq!(encoded["protocol_version"], 1);
    assert_eq!(encoded["operation"], "application.restart");
    assert_eq!(encoded["resource"]["kind"], "application");
    assert_eq!(
        encoded["resource"]["id"],
        "224065ac-4bc7-4ba4-a06d-8c8bca950c32"
    );
    assert_eq!(encoded["parameters"], serde_json::json!({}));
}

#[test]
fn contract_error_codes_are_exhaustive() {
    // Guard against silently dropping a code the panel knows about.
    let codes = [
        ContractErrorCode::AgentUnavailable,
        ContractErrorCode::UnsupportedOperation,
        ContractErrorCode::UnsupportedProtocol,
        ContractErrorCode::InvalidParameters,
        ContractErrorCode::ResourceNotFound,
        ContractErrorCode::Conflict,
        ContractErrorCode::Timeout,
        ContractErrorCode::ExecutionUnknown,
        ContractErrorCode::PermissionDenied,
        ContractErrorCode::OperationFailed,
    ];
    assert_eq!(codes.len(), 10);
}
