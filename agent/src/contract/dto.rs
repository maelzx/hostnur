//! Rust mirror of the `contracts/v1` response DTOs and the shared value types.
//!
//! The provider side (Rust) must produce exactly the shapes the panel parses in
//! `panel/app/Agent/Dto`. Deserialization is strict (`deny_unknown_fields`);
//! semantic checks that serde cannot express live in `validate` helpers.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::error::ContractError;

/// A UUID in canonical RFC 4122 hyphenated form (any version). Matches Laravel's
/// `Str::isUuid` acceptance: `8-4-4-4-12` hexadecimal groups.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Uuid(String);

impl Uuid {
    pub fn parse(value: &str) -> Result<Uuid, ContractError> {
        if is_uuid(value) {
            Ok(Uuid(value.to_ascii_lowercase()))
        } else {
            Err(ContractError::invalid_parameters("value must be a UUID"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Uuid {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Uuid::parse(&value)
    }
}

impl<'de> Deserialize<'de> for Uuid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UuidVisitor;

        impl Visitor<'_> for UuidVisitor {
            type Value = Uuid;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a UUID string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Uuid, E> {
                Uuid::parse(value).map_err(|_| de::Error::custom("invalid UUID"))
            }
        }

        deserializer.deserialize_str(UuidVisitor)
    }
}

/// Whether `value` is a canonical hyphenated UUID of any version.
pub fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let is_hyphen_position = matches!(index, 8 | 13 | 18 | 23);
        if is_hyphen_position {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// Operational status of an accepted execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Accepted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl OperationStatus {
    /// A status whose outcome can still change without another submission.
    pub const fn is_active(self) -> bool {
        matches!(self, OperationStatus::Accepted | OperationStatus::Running)
    }

    pub const fn is_terminal(self) -> bool {
        !self.is_active()
    }
}

/// Cancellation outcome reported by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationStatus {
    Requested,
    Unsupported,
    AlreadyFinished,
}

/// Agent availability mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Unavailable,
    Fake,
    Real,
}

/// A safe agent error attached to a failed snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentError {
    pub code: super::error::ContractErrorCode,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

/// Acceptance receipt. Acceptance never means completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationReceipt {
    pub protocol_version: i64,
    pub request_id: Uuid,
    pub execution_id: Uuid,
    pub operation: String,
    pub status: OperationStatus,
    pub accepted_at: String,
}

/// One bounded progress event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEvent {
    pub sequence: i64,
    pub at: String,
    pub message: String,
}

/// Durable execution state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSnapshot {
    pub protocol_version: i64,
    pub execution_id: Uuid,
    pub request_id: Uuid,
    pub operation: String,
    pub status: OperationStatus,
    pub progress: Option<i64>,
    pub events: Vec<SnapshotEvent>,
    pub result: Option<serde_json::Value>,
    pub error: Option<AgentError>,
    pub updated_at: String,
}

impl OperationSnapshot {
    /// Semantic checks not expressible in serde.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Some(progress) = self.progress
            && !(0..=100).contains(&progress)
        {
            return Err(ContractError::invalid_parameters(
                "snapshot progress must be between 0 and 100",
            ));
        }
        Ok(())
    }
}

/// Acknowledgement that a cancellation request was received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationReceipt {
    pub protocol_version: i64,
    pub execution_id: Uuid,
    pub status: CancellationStatus,
    pub requested_at: String,
}

/// Protocol/mode/operation support and availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySnapshot {
    pub protocol_version: i64,
    pub mode: AgentMode,
    pub available: bool,
    pub operations: Vec<String>,
    pub sampled_at: String,
    pub reason: Option<String>,
}

/// One sampled server metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub value: serde_json::Number,
    pub unit: Option<String>,
}

/// One observed service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceObservation {
    pub name: String,
    pub status: String,
}

/// Read-only firewall summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirewallSummary {
    pub enabled: bool,
    pub incoming_policy: String,
    pub rule_count: i64,
    #[serde(default)]
    pub rules: Vec<FirewallRule>,
}

/// A single observed firewall rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirewallRule {
    pub port: i64,
    pub protocol: String,
    pub source: String,
    pub action: String,
}

/// Bounded read-only server observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSnapshot {
    pub protocol_version: i64,
    pub mode: AgentMode,
    pub available: bool,
    pub sampled_at: String,
    pub metrics: Vec<Metric>,
    pub services: Vec<ServiceObservation>,
    pub reason: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub firewall: Option<FirewallSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_and_uppercase_uuids() {
        assert!(is_uuid("224065ac-4bc7-4ba4-a06d-8c8bca950c32"));
        assert!(is_uuid("224065AC-4BC7-4BA4-A06D-8C8BCA950C32"));
        assert_eq!(
            Uuid::parse("224065AC-4BC7-4BA4-A06D-8C8BCA950C32")
                .unwrap()
                .as_str(),
            "224065ac-4bc7-4ba4-a06d-8c8bca950c32"
        );
    }

    #[test]
    fn rejects_malformed_uuids() {
        for value in [
            "not-a-uuid",
            "224065ac4bc74ba4a06d8c8bca950c32",
            "224065ac-4bc7-4ba4-a06d-8c8bca950c3",
            "224065ac-4bc7-4ba4-a06d-8c8bca950c3z",
            "224065ac_4bc7_4ba4_a06d_8c8bca950c32",
        ] {
            assert!(!is_uuid(value), "{value} should be rejected");
        }
    }

    #[test]
    fn status_activity() {
        assert!(OperationStatus::Accepted.is_active());
        assert!(OperationStatus::Running.is_active());
        assert!(OperationStatus::Succeeded.is_terminal());
        assert!(OperationStatus::TimedOut.is_terminal());
    }
}
