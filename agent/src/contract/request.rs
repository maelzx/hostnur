//! Strict parsing and validation of the operation request envelope.
//!
//! This is the agent's independent re-validation of a request: the PHP schema is
//! not the root trust boundary. Envelope shape, protocol/operation allowlists,
//! resource kind and per-operation parameter rules are all enforced here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::catalog::{Catalog, OperationDefinition, ParameterRule};
use super::dto::{Uuid, is_uuid};
use super::error::ContractError;
use super::framing::MAX_MESSAGE_BYTES;

/// Maximum number of named parameters in a request.
pub const MAX_PARAMETERS: usize = 64;

/// A validated operation request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationRequest {
    pub protocol_version: i64,
    pub request_id: Uuid,
    pub idempotency_key: Uuid,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceRef>,
    pub parameters: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i64>,
}

/// A managed-resource reference resolved by the agent, never a caller path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceRef {
    pub kind: String,
    pub id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    protocol_version: i64,
    request_id: Uuid,
    idempotency_key: Uuid,
    operation: String,
    #[serde(default)]
    resource: Option<RawResource>,
    parameters: Map<String, Value>,
    #[serde(default)]
    timeout_seconds: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResource {
    kind: String,
    id: Uuid,
}

/// Parse and validate a request from raw message bytes.
pub fn parse_request(bytes: &[u8], catalog: &Catalog) -> Result<OperationRequest, ContractError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ContractError::invalid_parameters(
            "message exceeds the maximum size",
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|_| ContractError::invalid_parameters("request envelope is invalid"))
        .and_then(|raw: RawRequest| validate(raw, catalog))
}

/// Parse and validate a request from a JSON string (tests and in-process use).
pub fn parse_request_str(text: &str, catalog: &Catalog) -> Result<OperationRequest, ContractError> {
    parse_request(text.as_bytes(), catalog)
}

fn validate(raw: RawRequest, catalog: &Catalog) -> Result<OperationRequest, ContractError> {
    if !catalog.supports_protocol(raw.protocol_version) {
        return Err(ContractError::unsupported_protocol(raw.protocol_version));
    }

    let definition = catalog
        .definition(&raw.operation)
        .ok_or_else(|| ContractError::unsupported_operation(&raw.operation))?;

    let resource = validate_resource(definition, raw.resource)?;
    let parameters = validate_parameters(definition, raw.parameters)?;

    if let Some(timeout) = raw.timeout_seconds
        && !(1..=86_400).contains(&timeout)
    {
        return Err(ContractError::invalid_parameters(
            "timeout_seconds must be between 1 and 86400",
        ));
    }

    Ok(OperationRequest {
        protocol_version: raw.protocol_version,
        request_id: raw.request_id,
        idempotency_key: raw.idempotency_key,
        operation: raw.operation,
        resource,
        parameters,
        timeout_seconds: raw.timeout_seconds,
    })
}

fn validate_resource(
    definition: &OperationDefinition,
    resource: Option<RawResource>,
) -> Result<Option<ResourceRef>, ContractError> {
    match (definition.resource.as_deref(), resource) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ContractError::invalid_parameters(
            "this operation does not accept a resource reference",
        )),
        (Some(_), None) => Err(ContractError::invalid_parameters(
            "this operation requires a resource reference",
        )),
        (Some(expected), Some(raw)) => {
            if raw.kind != expected {
                return Err(ContractError::invalid_parameters(
                    "resource kind is not valid for this operation",
                ));
            }
            Ok(Some(ResourceRef {
                kind: raw.kind,
                id: raw.id,
            }))
        }
    }
}

fn validate_parameters(
    definition: &OperationDefinition,
    parameters: Map<String, Value>,
) -> Result<Map<String, Value>, ContractError> {
    if parameters.len() > MAX_PARAMETERS {
        return Err(ContractError::invalid_parameters(format!(
            "at most {MAX_PARAMETERS} parameters are allowed"
        )));
    }

    for key in parameters.keys() {
        if !definition.parameters.contains_key(key) {
            return Err(ContractError::invalid_parameters(format!(
                "unknown parameter `{key}`"
            )));
        }
    }

    for (name, rule) in &definition.parameters {
        match parameters.get(name) {
            None => {
                if rule.required {
                    return Err(ContractError::invalid_parameters(format!(
                        "parameter `{name}` is required"
                    )));
                }
            }
            Some(value) => assert_parameter_type(name, value, rule)?,
        }
    }

    Ok(parameters)
}

fn assert_parameter_type(
    name: &str,
    value: &Value,
    rule: &ParameterRule,
) -> Result<(), ContractError> {
    let valid = match rule.kind.as_str() {
        "uuid" => value.as_str().is_some_and(is_uuid),
        "string" => value
            .as_str()
            .is_some_and(|text| !text.is_empty() && text.len() <= rule.max.unwrap_or(255)),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "enum" => value.as_str().is_some_and(|text| {
            rule.values
                .as_ref()
                .is_some_and(|v| v.iter().any(|x| x == text))
        }),
        "string_array" => {
            let max_items = rule.max_items.unwrap_or(100);
            match value.as_object() {
                Some(map) => {
                    map.len() <= max_items && map.values().all(serde_json::Value::is_string)
                }
                None => false,
            }
        }
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(ContractError::invalid_parameters(format!(
            "parameter `{name}` has an invalid value"
        )))
    }
}

/// Look up the parameter rules for an operation (used by later phases).
pub fn parameter_rules<'a>(
    catalog: &'a Catalog,
    operation: &str,
) -> Option<&'a BTreeMap<String, ParameterRule>> {
    catalog
        .definition(operation)
        .map(|definition| &definition.parameters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::error::ContractErrorCode;

    fn catalog() -> Catalog {
        Catalog::embedded()
    }

    #[test]
    fn parses_a_valid_resource_request() {
        let text = r#"{
            "protocol_version": 1,
            "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
            "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
            "operation": "application.restart",
            "resource": {"kind": "application", "id": "224065ac-4bc7-4ba4-a06d-8c8bca950c32"},
            "parameters": {},
            "timeout_seconds": 60
        }"#;
        let request = parse_request_str(text, &catalog()).expect("valid");
        assert_eq!(request.operation, "application.restart");
        assert_eq!(request.timeout_seconds, Some(60));
        assert_eq!(request.resource.unwrap().kind, "application");
    }

    #[test]
    fn rejects_unknown_operation() {
        let text = r#"{
            "protocol_version": 1,
            "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
            "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
            "operation": "server.obliterate",
            "parameters": {}
        }"#;
        let err = parse_request_str(text, &catalog()).unwrap_err();
        assert_eq!(err.code(), ContractErrorCode::UnsupportedOperation);
    }

    #[test]
    fn rejects_unsupported_protocol() {
        let text = r#"{
            "protocol_version": 999,
            "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
            "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
            "operation": "server.snapshot",
            "parameters": {}
        }"#;
        let err = parse_request_str(text, &catalog()).unwrap_err();
        assert_eq!(err.code(), ContractErrorCode::UnsupportedProtocol);
    }

    #[test]
    fn rejects_extra_envelope_key() {
        let text = r#"{
            "protocol_version": 1,
            "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
            "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
            "operation": "server.snapshot",
            "parameters": {},
            "unexpected": "nope"
        }"#;
        let err = parse_request_str(text, &catalog()).unwrap_err();
        assert_eq!(err.code(), ContractErrorCode::InvalidParameters);
    }

    #[test]
    fn rejects_list_parameters() {
        let text = r#"{
            "protocol_version": 1,
            "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
            "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
            "operation": "service.control",
            "resource": {"kind": "service", "id": "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"},
            "parameters": ["restart"]
        }"#;
        let err = parse_request_str(text, &catalog()).unwrap_err();
        assert_eq!(err.code(), ContractErrorCode::InvalidParameters);
    }

    #[test]
    fn validates_enum_parameter() {
        let base = |action: &str| {
            format!(
                r#"{{
                    "protocol_version": 1,
                    "request_id": "0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55",
                    "idempotency_key": "9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d",
                    "operation": "service.control",
                    "resource": {{"kind": "service", "id": "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"}},
                    "parameters": {{"action": "{action}"}}
                }}"#
            )
        };
        assert!(parse_request_str(&base("restart"), &catalog()).is_ok());
        assert_eq!(
            parse_request_str(&base("destroy"), &catalog())
                .unwrap_err()
                .code(),
            ContractErrorCode::InvalidParameters
        );
    }
}
