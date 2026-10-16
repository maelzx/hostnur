//! Approved package installation primitive (phase 14.1).
//!
//! Package requests select an approved catalog id, never raw APT arguments or
//! repository URLs. Installations run a fixed `apt-get` with a minimal
//! environment and a bounded dpkg-lock timeout; lock files are never deleted.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::process::{CommandSpec, ProcessError, run, validate_spec};

/// An approved package catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedPackage {
    /// The real APT package name (never caller-supplied).
    pub apt_name: String,
    /// Optional required repository label (documentation/audit only here).
    pub repository: Option<String>,
    /// Whether the package hosts PHP extensions for a runtime.
    pub runtime: Option<String>,
}

/// Package policy failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    UnknownPackage(String),
    ToolFailed(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::UnknownPackage(name) => {
                write!(f, "package `{name}` is not in the approved catalog")
            }
            PackageError::ToolFailed(detail) => write!(f, "package tool failed: {detail}"),
        }
    }
}

impl std::error::Error for PackageError {}

/// Fixed package allowlist and installer.
#[derive(Debug, Clone)]
pub struct PackagePolicy {
    approved: BTreeMap<String, ApprovedPackage>,
    pub apt_get: PathBuf,
    pub dpkg_query: PathBuf,
    pub deadline: Duration,
    pub lock_timeout_seconds: u32,
}

impl Default for PackagePolicy {
    fn default() -> Self {
        let mut approved = BTreeMap::new();
        let mut add = |id: &str, apt: &str, runtime: Option<&str>| {
            approved.insert(
                id.to_string(),
                ApprovedPackage {
                    apt_name: apt.to_string(),
                    repository: None,
                    runtime: runtime.map(str::to_string),
                },
            );
        };
        add("nginx", "nginx", None);
        add("php8.3-fpm", "php8.3-fpm", Some("php"));
        add("php8.3-cli", "php8.3-cli", Some("php"));
        add("php8.3-mysql", "php8.3-mysql", Some("php"));
        add("php8.3-xml", "php8.3-xml", Some("php"));
        add("php8.3-mbstring", "php8.3-mbstring", Some("php"));
        add("php8.3-curl", "php8.3-curl", Some("php"));
        add("php8.3-zip", "php8.3-zip", Some("php"));
        add("php8.3-intl", "php8.3-intl", Some("php"));
        add("mariadb-server", "mariadb-server", None);
        add("certbot", "certbot", None);
        add("redis-server", "redis-server", None);

        Self {
            approved,
            apt_get: PathBuf::from("/usr/bin/apt-get"),
            dpkg_query: PathBuf::from("/usr/bin/dpkg-query"),
            deadline: Duration::from_secs(900),
            lock_timeout_seconds: 60,
        }
    }
}

impl PackagePolicy {
    pub fn is_approved(&self, id: &str) -> bool {
        self.approved.contains_key(id)
    }

    pub fn entry(&self, id: &str) -> Option<&ApprovedPackage> {
        self.approved.get(id)
    }

    /// Install an approved package and return the resulting dpkg version.
    pub fn install(&self, id: &str) -> Result<String, PackageError> {
        let entry = self
            .approved
            .get(id)
            .ok_or_else(|| PackageError::UnknownPackage(id.to_string()))?;

        let lock_option = format!("DPkg::Lock::Timeout={}", self.lock_timeout_seconds);
        let mut spec = CommandSpec::new(&self.apt_get, self.deadline);
        spec.args = vec![
            "-y".to_string(),
            "-o".to_string(),
            lock_option,
            "--no-install-recommends".to_string(),
            "install".to_string(),
            entry.apt_name.clone(),
        ];
        spec.env = installer_env();
        spec.output_cap_bytes = 65_536;
        validate_spec(&spec)
            .map_err(|error: ProcessError| PackageError::ToolFailed(error.to_string()))?;

        let outcome = run(&spec).map_err(|error| PackageError::ToolFailed(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(PackageError::ToolFailed(format!(
                "apt-get exit code {:?}",
                outcome.exit_code
            )));
        }

        self.installed_version(&entry.apt_name)
    }

    /// Read the installed version via `dpkg-query` (never a caller path).
    pub fn installed_version(&self, apt_name: &str) -> Result<String, PackageError> {
        let mut spec = CommandSpec::new(&self.dpkg_query, Duration::from_secs(30));
        spec.args = vec![
            "-W".to_string(),
            "-f=${Version}".to_string(),
            apt_name.to_string(),
        ];
        spec.env = installer_env();
        spec.output_cap_bytes = 1024;
        let outcome = run(&spec).map_err(|error| PackageError::ToolFailed(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(PackageError::ToolFailed("dpkg-query failed".to_string()));
        }
        if outcome.stdout_truncated {
            return Err(PackageError::ToolFailed(
                "dpkg-query output exceeded the cap".to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&outcome.stdout).trim().to_string())
    }

    /// Read pending package updates via `apt list --upgradable` (bounded,
    /// read-only; uses the cached lists and never runs `apt-get update`).
    pub fn list_updates(&self) -> (Vec<(String, String, String, bool)>, bool) {
        let mut spec = CommandSpec::new(PathBuf::from("/usr/bin/apt"), Duration::from_secs(30));
        spec.args = vec!["list".to_string(), "--upgradable".to_string()];
        spec.env = installer_env();
        spec.output_cap_bytes = 65_536;
        let Ok(outcome) = run(&spec) else {
            return (Vec::new(), false);
        };
        parse_upgradable(&String::from_utf8_lossy(&outcome.stdout))
    }
}

/// Parse `apt list --upgradable` into `(package, current, candidate, security)`.
pub fn parse_upgradable(text: &str) -> (Vec<(String, String, String, bool)>, bool) {
    let mut updates = Vec::new();
    let mut truncated = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Listing") {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 2 {
            continue;
        }

        let Some((package, suite)) = tokens[0].split_once('/') else {
            continue;
        };
        if package.is_empty() {
            continue;
        }

        let candidate = tokens[1].to_string();
        let current = line
            .split("[upgradable from:")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let security = suite.contains("security");

        if updates.len() >= 200 {
            truncated = true;
            break;
        }

        updates.push((package.to_string(), current, candidate, security));
    }

    (updates, truncated)
}

fn installer_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        "/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
    );
    env.insert("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string());
    env.insert("LC_ALL".to_string(), "C".to_string());
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upgradable_listing() {
        let text = "Listing...\nnginx/jammy-updates 1.18.0-0ubuntu1.4 amd64 [upgradable from: 1.18.0-0ubuntu1.2]\nopenssl/jammy-security 3.0.2-0ubuntu1.12 amd64 [upgradable from: 3.0.2-0ubuntu1.10]\n";
        let (updates, truncated) = parse_upgradable(text);
        assert!(!truncated);
        assert_eq!(updates.len(), 2);
        assert_eq!(
            updates[0],
            (
                "nginx".to_string(),
                "1.18.0-0ubuntu1.2".to_string(),
                "1.18.0-0ubuntu1.4".to_string(),
                false
            )
        );
        assert!(updates[1].3, "security suite should be flagged");
    }

    #[test]
    fn refuses_unapproved_packages() {
        let policy = PackagePolicy::default();
        assert!(policy.is_approved("nginx"));
        assert!(!policy.is_approved("cowsay"));
        assert!(matches!(
            policy.install("definitely-not-approved"),
            Err(PackageError::UnknownPackage(_))
        ));
    }

    #[test]
    fn does_not_expose_argument_injection() {
        // A caller cannot smuggle apt flags through a package id.
        let policy = PackagePolicy::default();
        assert!(matches!(
            policy.install("nginx; rm -rf /"),
            Err(PackageError::UnknownPackage(_))
        ));
    }
}
