//! Stable internal error codes.
//!
//! Internal errors carry a small set of machine-readable codes. The codes are
//! deliberately defined here rather than derived from a generic error crate so
//! that each one maps explicitly onto a contract error code later (see
//! `contracts/v1/README.md`). Messages must never contain configuration values,
//! environment contents, credentials or request bodies.

use std::fmt;

/// Stable machine-readable error codes.
///
/// Phase 10 uses the configuration/lifecycle subset. Transport and operation
/// codes (`unsupported_operation`, `resource_not_found`, `conflict`, ...) are
/// added when their phases land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// The configuration file does not exist or cannot be read.
    ConfigNotFound,
    /// The configuration file is not syntactically valid TOML.
    ConfigParse,
    /// The configuration is syntactically valid but semantically rejected.
    ConfigInvalid,
    /// An operating-system/filesystem operation failed.
    Io,
    /// Signal registration or process lifecycle setup failed.
    Lifecycle,
}

impl ErrorCode {
    /// Stable snake_case representation safe for logs and structured output.
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ConfigNotFound => "config_not_found",
            ErrorCode::ConfigParse => "config_parse",
            ErrorCode::ConfigInvalid => "config_invalid",
            ErrorCode::Io => "io",
            ErrorCode::Lifecycle => "lifecycle",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An agent error with a stable code and a safe, operator-readable message.
#[derive(Debug)]
pub struct AgentError {
    code: ErrorCode,
    message: String,
}

impl AgentError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn config_not_found(path: &std::path::Path) -> Self {
        Self::new(
            ErrorCode::ConfigNotFound,
            format!("configuration file not found: {}", path.display()),
        )
    }

    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self::new(ErrorCode::Io, format!("{context}: {}", err.kind()))
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AgentError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_snake_case() {
        assert_eq!(ErrorCode::ConfigNotFound.as_str(), "config_not_found");
        assert_eq!(ErrorCode::ConfigParse.as_str(), "config_parse");
        assert_eq!(ErrorCode::ConfigInvalid.as_str(), "config_invalid");
        assert_eq!(ErrorCode::Io.as_str(), "io");
        assert_eq!(ErrorCode::Lifecycle.as_str(), "lifecycle");
    }

    #[test]
    fn display_includes_code_and_message() {
        let err = AgentError::new(ErrorCode::ConfigInvalid, "unknown setting `nope`");
        assert_eq!(err.to_string(), "config_invalid: unknown setting `nope`");
    }
}
