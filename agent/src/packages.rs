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
        Ok(String::from_utf8_lossy(&outcome.stdout).trim().to_string())
    }
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
