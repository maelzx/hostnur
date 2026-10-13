//! Typed, validated agent configuration.
//!
//! The agent owns a small TOML configuration file (proposed production path
//! `/etc/hostnur/agent.toml`; see `plan.md` phase 11). Parsing is strict: unknown
//! settings, missing required settings, wrong types, unsupported versions and
//! unsafe paths are rejected with a stable code and a message that never echoes
//! configuration values. Parsing has no side effects and never touches the
//! filesystem beyond reading the one file.

use std::path::{Component, Path, PathBuf};

use crate::error::{AgentError, ErrorCode};
use crate::logging::Level;

/// Configuration schema version understood by this build.
pub const CONFIG_VERSION: u32 = 1;

pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 1_048_576;
pub const MIN_MAX_MESSAGE_BYTES: usize = 1024;
pub const MAX_MESSAGE_BYTES_LIMIT: usize = 1_048_576;

pub const DEFAULT_SHUTDOWN_GRACE_SECONDS: u64 = 10;
pub const MIN_SHUTDOWN_GRACE_SECONDS: u64 = 1;
pub const MAX_SHUTDOWN_GRACE_SECONDS: u64 = 300;

/// Default Unix socket mode: owner and group read/write, no world access.
pub const DEFAULT_SOCKET_MODE: u32 = 0o660;
pub const DEFAULT_CONNECTION_READ_TIMEOUT_SECONDS: u64 = 5;
pub const MIN_CONNECTION_READ_TIMEOUT_SECONDS: u64 = 1;
pub const MAX_CONNECTION_READ_TIMEOUT_SECONDS: u64 = 120;

/// Maximum length of a Unix socket path in bytes, leaving room for the NUL
/// terminator in `sockaddr_un.sun_path` (108 on Linux).
const MAX_SOCKET_PATH_BYTES: usize = 107;

/// Top-level settings accepted by this schema version. Anything else is an
/// unknown setting and is rejected.
const KNOWN_KEYS: [&str; 9] = [
    "version",
    "socket_path",
    "state_dir",
    "log_level",
    "max_message_bytes",
    "shutdown_grace_seconds",
    "socket_mode",
    "connection_read_timeout_seconds",
    "allowed_peer_uids",
];

/// Validated agent configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub version: u32,
    pub socket_path: PathBuf,
    pub state_dir: PathBuf,
    pub log_level: Level,
    pub max_message_bytes: usize,
    pub shutdown_grace_seconds: u64,
    pub socket_mode: u32,
    pub connection_read_timeout_seconds: u64,
    /// OS UIDs permitted to connect. Empty means no peer is authorized (fail
    /// closed); the panel/worker identity must be listed explicitly.
    pub allowed_peer_uids: Vec<u32>,
}

impl AgentConfig {
    /// Read and validate the configuration file at `path`.
    pub fn load(path: &Path) -> Result<AgentConfig, AgentError> {
        let text = std::fs::read_to_string(path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => AgentError::config_not_found(path),
            _ => AgentError::io("reading configuration", &err),
        })?;
        AgentConfig::parse(&text)
    }

    /// Parse and validate configuration from a TOML string.
    pub fn parse(text: &str) -> Result<AgentConfig, AgentError> {
        let value: toml::Value = toml::from_str(text).map_err(|_| {
            AgentError::new(ErrorCode::ConfigParse, "configuration is not valid TOML")
        })?;
        AgentConfig::from_value(&value)
    }

    fn from_value(value: &toml::Value) -> Result<AgentConfig, AgentError> {
        let table = value
            .as_table()
            .ok_or_else(|| invalid("the root of the configuration must be a table"))?;

        for key in table.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                return Err(invalid(format!("unknown setting `{key}`")));
            }
        }

        let version = required_integer(table, "version")?;
        let socket_path = PathBuf::from(required_string(table, "socket_path")?);
        let state_dir = PathBuf::from(required_string(table, "state_dir")?);
        let log_level = optional_log_level(table)?;
        let max_message_bytes =
            optional_bounded_usize(table, "max_message_bytes", DEFAULT_MAX_MESSAGE_BYTES)?;
        let shutdown_grace_seconds = optional_bounded_u64(
            table,
            "shutdown_grace_seconds",
            DEFAULT_SHUTDOWN_GRACE_SECONDS,
        )?;
        let socket_mode = optional_bounded_u32(table, "socket_mode", DEFAULT_SOCKET_MODE)?;
        let connection_read_timeout_seconds = optional_bounded_u64(
            table,
            "connection_read_timeout_seconds",
            DEFAULT_CONNECTION_READ_TIMEOUT_SECONDS,
        )?;
        let allowed_peer_uids = optional_u32_array(table, "allowed_peer_uids")?;

        let config = AgentConfig {
            version: u32::try_from(version)
                .map_err(|_| invalid("setting `version` must be a non-negative integer"))?,
            socket_path,
            state_dir,
            log_level,
            max_message_bytes,
            shutdown_grace_seconds,
            socket_mode,
            connection_read_timeout_seconds,
            allowed_peer_uids,
        };
        config.validate()?;
        Ok(config)
    }

    /// Semantic validation of an already-typed configuration.
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.version != CONFIG_VERSION {
            return Err(invalid(format!(
                "unsupported configuration version {} (expected {})",
                self.version, CONFIG_VERSION
            )));
        }

        validate_absolute_path("socket_path", &self.socket_path)?;
        if !self.socket_path.as_os_str().is_empty()
            && self.socket_path.as_os_str().as_encoded_bytes().len() > MAX_SOCKET_PATH_BYTES
        {
            return Err(invalid(format!(
                "`socket_path` must be at most {MAX_SOCKET_PATH_BYTES} bytes"
            )));
        }

        validate_absolute_path("state_dir", &self.state_dir)?;

        if self.max_message_bytes < MIN_MAX_MESSAGE_BYTES
            || self.max_message_bytes > MAX_MESSAGE_BYTES_LIMIT
        {
            return Err(invalid(format!(
                "`max_message_bytes` must be between {MIN_MAX_MESSAGE_BYTES} and {MAX_MESSAGE_BYTES_LIMIT}"
            )));
        }

        if self.shutdown_grace_seconds < MIN_SHUTDOWN_GRACE_SECONDS
            || self.shutdown_grace_seconds > MAX_SHUTDOWN_GRACE_SECONDS
        {
            return Err(invalid(format!(
                "`shutdown_grace_seconds` must be between {MIN_SHUTDOWN_GRACE_SECONDS} and {MAX_SHUTDOWN_GRACE_SECONDS}"
            )));
        }

        if self.socket_mode > 0o777 {
            return Err(invalid(
                "`socket_mode` must be a permission mask (0..=0o777)",
            ));
        }
        if self.socket_mode & 0o002 != 0 {
            return Err(invalid("`socket_mode` must not be world-writable"));
        }

        if self.connection_read_timeout_seconds < MIN_CONNECTION_READ_TIMEOUT_SECONDS
            || self.connection_read_timeout_seconds > MAX_CONNECTION_READ_TIMEOUT_SECONDS
        {
            return Err(invalid(format!(
                "`connection_read_timeout_seconds` must be between {MIN_CONNECTION_READ_TIMEOUT_SECONDS} and {MAX_CONNECTION_READ_TIMEOUT_SECONDS}"
            )));
        }

        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> AgentError {
    AgentError::new(ErrorCode::ConfigInvalid, message)
}

fn required_integer(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<i64, AgentError> {
    match table.get(key) {
        None => Err(invalid(format!("missing required setting `{key}`"))),
        Some(value) => value
            .as_integer()
            .ok_or_else(|| invalid(format!("setting `{key}` must be an integer"))),
    }
}

fn required_string(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<String, AgentError> {
    match table.get(key) {
        None => Err(invalid(format!("missing required setting `{key}`"))),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid(format!("setting `{key}` must be a string"))),
    }
}

fn optional_log_level(table: &toml::map::Map<String, toml::Value>) -> Result<Level, AgentError> {
    match table.get("log_level") {
        None => Ok(Level::Info),
        Some(value) => {
            let raw = value
                .as_str()
                .ok_or_else(|| invalid("setting `log_level` must be a string"))?;
            Level::parse(raw).ok_or_else(|| {
                invalid("setting `log_level` must be one of error, warn, info, debug")
            })
        }
    }
}

fn optional_bounded_usize(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    default: usize,
) -> Result<usize, AgentError> {
    match table.get(key) {
        None => Ok(default),
        Some(value) => {
            let raw = value
                .as_integer()
                .ok_or_else(|| invalid(format!("setting `{key}` must be an integer")))?;
            usize::try_from(raw)
                .map_err(|_| invalid(format!("setting `{key}` must be a non-negative integer")))
        }
    }
}

fn optional_bounded_u64(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    default: u64,
) -> Result<u64, AgentError> {
    match table.get(key) {
        None => Ok(default),
        Some(value) => {
            let raw = value
                .as_integer()
                .ok_or_else(|| invalid(format!("setting `{key}` must be an integer")))?;
            u64::try_from(raw)
                .map_err(|_| invalid(format!("setting `{key}` must be a non-negative integer")))
        }
    }
}

fn optional_bounded_u32(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    default: u32,
) -> Result<u32, AgentError> {
    match table.get(key) {
        None => Ok(default),
        Some(value) => {
            let raw = value
                .as_integer()
                .ok_or_else(|| invalid(format!("setting `{key}` must be an integer")))?;
            u32::try_from(raw)
                .map_err(|_| invalid(format!("setting `{key}` must be a non-negative integer")))
        }
    }
}

fn optional_u32_array(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Vec<u32>, AgentError> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(value) => {
            let items = value
                .as_array()
                .ok_or_else(|| invalid(format!("setting `{key}` must be an array of integers")))?;
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                let raw = item.as_integer().ok_or_else(|| {
                    invalid(format!("setting `{key}` must be an array of integers"))
                })?;
                result.push(u32::try_from(raw).map_err(|_| {
                    invalid(format!(
                        "setting `{key}` must contain non-negative integers"
                    ))
                })?);
            }
            Ok(result)
        }
    }
}

/// Reject relative paths, `..` traversal and the filesystem root. The agent
/// never accepts a caller-supplied path for privileged work; configuration is
/// operator-owned, and this is the first line of defence.
fn validate_absolute_path(label: &str, path: &Path) -> Result<(), AgentError> {
    if path.as_os_str().is_empty() {
        return Err(invalid(format!("`{label}` must not be empty")));
    }
    if !path.is_absolute() {
        return Err(invalid(format!("`{label}` must be an absolute path")));
    }
    if path == Path::new("/") {
        return Err(invalid(format!(
            "`{label}` must not be the filesystem root"
        )));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(invalid(format!("`{label}` must not contain `..`")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_toml() -> String {
        r#"
            version = 1
            socket_path = "/run/hostnur/agent.sock"
            state_dir = "/var/lib/hostnur/agent"
            log_level = "debug"
            max_message_bytes = 65536
            shutdown_grace_seconds = 5
            socket_mode = 432
            connection_read_timeout_seconds = 7
            allowed_peer_uids = [1001, 1002]
        "#
        .to_string()
    }

    #[test]
    fn parses_valid_configuration() {
        let config = AgentConfig::parse(&valid_toml()).expect("valid config");
        assert_eq!(config.version, 1);
        assert_eq!(config.socket_path, PathBuf::from("/run/hostnur/agent.sock"));
        assert_eq!(config.state_dir, PathBuf::from("/var/lib/hostnur/agent"));
        assert_eq!(config.log_level, Level::Debug);
        assert_eq!(config.max_message_bytes, 65536);
        assert_eq!(config.shutdown_grace_seconds, 5);
        assert_eq!(config.socket_mode, 0o660);
        assert_eq!(config.connection_read_timeout_seconds, 7);
        assert_eq!(config.allowed_peer_uids, vec![1001, 1002]);
    }

    #[test]
    fn defaults_peer_allowlist_to_empty_and_mode_to_0660() {
        let text = r#"
            version = 1
            socket_path = "/run/hostnur/agent.sock"
            state_dir = "/var/lib/hostnur/agent"
        "#;
        let config = AgentConfig::parse(text).expect("valid config");
        assert_eq!(config.socket_mode, DEFAULT_SOCKET_MODE);
        assert!(config.allowed_peer_uids.is_empty());
        assert_eq!(
            config.connection_read_timeout_seconds,
            DEFAULT_CONNECTION_READ_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn rejects_world_writable_socket_mode() {
        let text = "version = 1\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"\nsocket_mode = 438";
        // 438 == 0o666 (world-writable)
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(err.message().contains("world-writable"));
    }

    #[test]
    fn applies_defaults_for_optional_settings() {
        let text = r#"
            version = 1
            socket_path = "/run/hostnur/agent.sock"
            state_dir = "/var/lib/hostnur/agent"
        "#;
        let config = AgentConfig::parse(text).expect("valid config");
        assert_eq!(config.log_level, Level::Info);
        assert_eq!(config.max_message_bytes, DEFAULT_MAX_MESSAGE_BYTES);
        assert_eq!(
            config.shutdown_grace_seconds,
            DEFAULT_SHUTDOWN_GRACE_SECONDS
        );
    }

    #[test]
    fn rejects_malformed_toml_without_echoing_values() {
        let text = "version = = 1\nsocket_path = \"secret-value\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigParse);
        assert!(!err.message().contains("secret-value"));
    }

    #[test]
    fn rejects_unknown_setting() {
        let text = format!("{}\nhost_root = \"/\"", valid_toml());
        let err = AgentConfig::parse(&text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(err.message().contains("unknown setting `host_root`"));
    }

    #[test]
    fn rejects_missing_required_setting() {
        let text = "version = 1\nstate_dir = \"/var/lib/hostnur/agent\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(
            err.message()
                .contains("missing required setting `socket_path`")
        );
    }

    #[test]
    fn rejects_wrong_type_without_echoing_value() {
        let text = "version = 1\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = 7";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(
            err.message()
                .contains("setting `state_dir` must be a string")
        );
    }

    #[test]
    fn rejects_relative_paths() {
        let text =
            "version = 1\nsocket_path = \"run/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(
            err.message()
                .contains("`socket_path` must be an absolute path")
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let text = "version = 1\nsocket_path = \"/run/../etc/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(
            err.message()
                .contains("`socket_path` must not contain `..`")
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        let text = "version = 99\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(
            err.message()
                .contains("unsupported configuration version 99")
        );
    }

    #[test]
    fn rejects_out_of_range_limits() {
        let text = "version = 1\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"\nmax_message_bytes = 1";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(err.message().contains("`max_message_bytes`"));

        let text = "version = 1\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"\nshutdown_grace_seconds = 0";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(err.message().contains("`shutdown_grace_seconds`"));
    }

    #[test]
    fn rejects_invalid_log_level() {
        let text = "version = 1\nsocket_path = \"/run/hostnur/agent.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"\nlog_level = \"trace\"";
        let err = AgentConfig::parse(text).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(err.message().contains("`log_level`"));
    }

    #[test]
    fn load_missing_file_reports_config_not_found() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = AgentConfig::load(&dir.path().join("absent.toml")).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::ConfigNotFound);
    }
}
