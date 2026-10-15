//! Runtime and hostname policy for the PHP/Laravel provider (phase 14.1/14.4).
//!
//! The native Ubuntu 24.04 PHP 8.3/PHP-FPM is the baseline. External PHP
//! repositories/versions are only usable when an operator explicitly configures
//! them; nothing is auto-added and untested versions are refused.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Runtime-policy failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    UnknownRuntime(String),
    UnsupportedVersion { runtime: String, version: String },
    InvalidHostname(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::UnknownRuntime(runtime) => {
                write!(f, "runtime `{runtime}` is not supported")
            }
            RuntimeError::UnsupportedVersion { runtime, version } => {
                write!(
                    f,
                    "runtime `{runtime}` version `{version}` is not supported"
                )
            }
            RuntimeError::InvalidHostname(host) => write!(f, "hostname `{host}` is invalid"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Supported runtimes and versions.
#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    supported: BTreeMap<String, Vec<String>>,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        let mut supported = BTreeMap::new();
        supported.insert("php".to_string(), vec!["8.3".to_string()]);
        Self { supported }
    }
}

impl RuntimePolicy {
    /// Register additional operator-approved versions for a runtime.
    pub fn allow_versions(mut self, runtime: &str, versions: &[&str]) -> Self {
        let entry = self.supported.entry(runtime.to_string()).or_default();
        for version in versions {
            if !entry.iter().any(|existing| existing == version) {
                entry.push((*version).to_string());
            }
        }
        self
    }

    pub fn supports(&self, runtime: &str, version: &str) -> bool {
        self.supported
            .get(runtime)
            .is_some_and(|versions| versions.iter().any(|v| v == version))
    }

    pub fn validate(&self, runtime: &str, version: &str) -> Result<(), RuntimeError> {
        let versions = self
            .supported
            .get(runtime)
            .ok_or_else(|| RuntimeError::UnknownRuntime(runtime.to_string()))?;
        if versions.iter().any(|v| v == version) {
            Ok(())
        } else {
            Err(RuntimeError::UnsupportedVersion {
                runtime: runtime.to_string(),
                version: version.to_string(),
            })
        }
    }

    /// Registered FPM socket for a PHP version.
    pub fn fpm_socket(&self, version: &str) -> PathBuf {
        PathBuf::from(format!("/run/php/php{version}-fpm.sock"))
    }
}

/// Validate a domain name for a managed server block. The exact value passed
/// in is what later gets interpolated, so leading/trailing whitespace is
/// rejected rather than silently trimmed, and the first/last characters must
/// be alphanumeric — which also blocks option-like tokens (`-agree-tos`) from
/// reaching certbot and wildcard names the panel itself refuses.
pub fn validate_hostname(host: &str) -> Result<(), RuntimeError> {
    if host.is_empty() || host.len() > 253 || host.contains(char::is_whitespace) {
        return Err(RuntimeError::InvalidHostname(host.to_string()));
    }
    if host
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
    {
        return Err(RuntimeError::InvalidHostname(host.to_string()));
    }
    if !host.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !host.ends_with(|c: char| c.is_ascii_alphanumeric())
    {
        return Err(RuntimeError::InvalidHostname(host.to_string()));
    }
    if host.contains("..") || host.contains(".-") || host.contains("-.") {
        return Err(RuntimeError::InvalidHostname(host.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_php_is_supported_and_external_opt_in() {
        let policy = RuntimePolicy::default();
        assert!(policy.supports("php", "8.3"));
        assert!(!policy.supports("php", "8.4"));
        assert!(matches!(
            policy.validate("php", "8.4"),
            Err(RuntimeError::UnsupportedVersion { .. })
        ));
        assert!(matches!(
            policy.validate("ruby", "3.2"),
            Err(RuntimeError::UnknownRuntime(_))
        ));

        let policy = policy.allow_versions("php", &["8.4"]);
        assert!(policy.supports("php", "8.4"));
        assert_eq!(
            policy.fpm_socket("8.4"),
            PathBuf::from("/run/php/php8.4-fpm.sock")
        );
    }

    #[test]
    fn hostnames_are_restricted() {
        assert!(validate_hostname("app.example.com").is_ok());
        for bad in [
            "",
            "a b",
            "app..com",
            ".app",
            "app.",
            "app;rm",
            "app/../x",
            "-agree-tos",
            " app.example.com ",
            "*.example.com",
            "app-.example.com",
        ] {
            assert!(validate_hostname(bad).is_err(), "{bad:?} should be invalid");
        }
    }
}
