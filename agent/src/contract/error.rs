//! Contract error codes shared with the PHP panel (`contracts/v1`).

use std::fmt;

/// Machine-readable contract error code. The string values match
/// `panel/app/Enums/ErrorCode.php` and `contracts/v1/README.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractErrorCode {
    AgentUnavailable,
    UnsupportedOperation,
    UnsupportedProtocol,
    InvalidParameters,
    ResourceNotFound,
    Conflict,
    Timeout,
    ExecutionUnknown,
    PermissionDenied,
    OperationFailed,
}

impl ContractErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            ContractErrorCode::AgentUnavailable => "agent_unavailable",
            ContractErrorCode::UnsupportedOperation => "unsupported_operation",
            ContractErrorCode::UnsupportedProtocol => "unsupported_protocol",
            ContractErrorCode::InvalidParameters => "invalid_parameters",
            ContractErrorCode::ResourceNotFound => "resource_not_found",
            ContractErrorCode::Conflict => "conflict",
            ContractErrorCode::Timeout => "timeout",
            ContractErrorCode::ExecutionUnknown => "execution_unknown",
            ContractErrorCode::PermissionDenied => "permission_denied",
            ContractErrorCode::OperationFailed => "operation_failed",
        }
    }
}

impl fmt::Display for ContractErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for ContractErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ContractErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "agent_unavailable" => Ok(ContractErrorCode::AgentUnavailable),
            "unsupported_operation" => Ok(ContractErrorCode::UnsupportedOperation),
            "unsupported_protocol" => Ok(ContractErrorCode::UnsupportedProtocol),
            "invalid_parameters" => Ok(ContractErrorCode::InvalidParameters),
            "resource_not_found" => Ok(ContractErrorCode::ResourceNotFound),
            "conflict" => Ok(ContractErrorCode::Conflict),
            "timeout" => Ok(ContractErrorCode::Timeout),
            "execution_unknown" => Ok(ContractErrorCode::ExecutionUnknown),
            "permission_denied" => Ok(ContractErrorCode::PermissionDenied),
            "operation_failed" => Ok(ContractErrorCode::OperationFailed),
            other => Err(serde::de::Error::custom(format!(
                "unknown contract error code `{other}`"
            ))),
        }
    }
}

/// A contract validation failure with a stable code and a safe message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    code: ContractErrorCode,
    message: String,
}

impl ContractError {
    pub fn new(code: ContractErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_parameters(message: impl Into<String>) -> Self {
        Self::new(ContractErrorCode::InvalidParameters, message)
    }

    pub fn unsupported_protocol(version: i64) -> Self {
        Self::new(
            ContractErrorCode::UnsupportedProtocol,
            format!("protocol version {version} is not supported"),
        )
    }

    pub fn unsupported_operation(operation: &str) -> Self {
        Self::new(
            ContractErrorCode::UnsupportedOperation,
            format!("operation `{operation}` is not in the catalog"),
        )
    }

    pub fn code(&self) -> ContractErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_match_php_values() {
        assert_eq!(
            ContractErrorCode::AgentUnavailable.as_str(),
            "agent_unavailable"
        );
        assert_eq!(
            ContractErrorCode::UnsupportedOperation.as_str(),
            "unsupported_operation"
        );
        assert_eq!(
            ContractErrorCode::UnsupportedProtocol.as_str(),
            "unsupported_protocol"
        );
        assert_eq!(
            ContractErrorCode::InvalidParameters.as_str(),
            "invalid_parameters"
        );
        assert_eq!(
            ContractErrorCode::ResourceNotFound.as_str(),
            "resource_not_found"
        );
        assert_eq!(ContractErrorCode::Conflict.as_str(), "conflict");
        assert_eq!(ContractErrorCode::Timeout.as_str(), "timeout");
        assert_eq!(
            ContractErrorCode::ExecutionUnknown.as_str(),
            "execution_unknown"
        );
        assert_eq!(
            ContractErrorCode::PermissionDenied.as_str(),
            "permission_denied"
        );
        assert_eq!(
            ContractErrorCode::OperationFailed.as_str(),
            "operation_failed"
        );
    }

    #[test]
    fn error_code_json_roundtrip() {
        let json = serde_json::to_string(&ContractErrorCode::OperationFailed).unwrap();
        assert_eq!(json, "\"operation_failed\"");
        let parsed: ContractErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ContractErrorCode::OperationFailed);
        assert!(serde_json::from_str::<ContractErrorCode>("\"nope\"").is_err());
    }
}
