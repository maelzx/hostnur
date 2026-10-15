//! Bounded Git checkout executed as the application user (phase 13.2/13.3).
//!
//! Runs a fixed Git binary with a validated argument array and a minimal,
//! hardened environment: no system/global config, no credential helper, no SSH,
//! an explicit allowed-protocol list and no interactive prompt. The resolved
//! full commit id is returned so later stages pin an immutable source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, ProcessError, run};

use super::policy::GitRef;

/// Result of a successful checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutOutcome {
    pub commit: String,
    pub duration: Duration,
}

/// Checkout failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutError {
    InvalidRequest(String),
    Git(String),
    Process(String),
}

impl std::fmt::Display for CheckoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckoutError::InvalidRequest(message) => {
                write!(f, "invalid checkout request: {message}")
            }
            CheckoutError::Git(message) => write!(f, "git failed: {message}"),
            CheckoutError::Process(message) => write!(f, "process failed: {message}"),
        }
    }
}

impl std::error::Error for CheckoutError {}

/// A checkout request. `repository_url` and `git_ref` must already be validated
/// by `policy`.
pub struct CheckoutRequest<'a> {
    pub repository_url: &'a str,
    pub git_ref: &'a GitRef,
    /// Absolute, application-owned staging directory (created by the caller).
    pub staging_dir: &'a Path,
    pub uid: u32,
    pub gid: u32,
    pub deadline: Duration,
    /// Optional GIT_ASKPASS helper for a private credential (phase 13.3).
    pub askpass: Option<PathBuf>,
}

/// Fixed Git runner configuration.
#[derive(Debug, Clone)]
pub struct GitRunner {
    pub program: PathBuf,
    /// Comma-separated `GIT_ALLOW_PROTOCOL` value.
    pub allow_protocol: String,
    pub output_cap_bytes: usize,
}

impl Default for GitRunner {
    fn default() -> Self {
        Self {
            program: PathBuf::from("/usr/bin/git"),
            allow_protocol: "https".to_string(),
            output_cap_bytes: 65_536,
        }
    }
}

impl GitRunner {
    /// Clone the ref into `staging_dir` as the app user and resolve its commit.
    pub fn checkout(
        &self,
        request: &CheckoutRequest<'_>,
    ) -> Result<CheckoutOutcome, CheckoutError> {
        if !request.staging_dir.is_absolute() {
            return Err(CheckoutError::InvalidRequest(
                "staging directory must be absolute".to_string(),
            ));
        }
        if request.staging_dir == Path::new("/") {
            return Err(CheckoutError::InvalidRequest(
                "staging directory must not be the filesystem root".to_string(),
            ));
        }
        if request.repository_url.contains(char::is_whitespace) {
            // The staging tree was created for this checkout; remove it here
            // too so early rejections leave nothing behind, like later errors.
            let _ = std::fs::remove_dir_all(request.staging_dir);
            return Err(CheckoutError::InvalidRequest(
                "repository URL contains whitespace".to_string(),
            ));
        }

        let started = std::time::Instant::now();
        let dir = request.staging_dir;
        let reference = request.git_ref.as_str();

        let runner_self = self;
        let attempt = || -> Result<CheckoutOutcome, CheckoutError> {
            runner_self.git(
                request,
                &["-c", "init.defaultBranch=main", "init", "--quiet"],
            )?;
            runner_self.git(
                request,
                &["remote", "add", "origin", request.repository_url],
            )?;
            runner_self.git(
                request,
                &[
                    "-c",
                    "protocol.version=2",
                    "fetch",
                    "--quiet",
                    "--depth",
                    "1",
                    "--no-tags",
                    "origin",
                    reference,
                ],
            )?;
            let resolved = runner_self.git(request, &["rev-parse", "FETCH_HEAD"])?;
            let commit = String::from_utf8_lossy(&resolved.stdout).trim().to_string();
            if !is_commit(&commit) {
                return Err(CheckoutError::Git(
                    "could not resolve a full commit id".to_string(),
                ));
            }
            runner_self.git(
                request,
                &[
                    "-c",
                    "advice.detachedHead=false",
                    "checkout",
                    "--quiet",
                    "--detach",
                    &commit,
                ],
            )?;
            Ok(CheckoutOutcome {
                commit,
                duration: started.elapsed(),
            })
        };

        match attempt() {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                // Preserve the active app; discard only the incomplete staging tree.
                let _ = std::fs::remove_dir_all(dir);
                Err(error)
            }
        }
    }

    fn git(
        &self,
        request: &CheckoutRequest<'_>,
        args: &[&str],
    ) -> Result<crate::process::ProcessOutcome, CheckoutError> {
        let mut spec = CommandSpec::new(&self.program, request.deadline);
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.working_dir = Some(request.staging_dir.to_path_buf());
        spec.env = self.environment(request);
        spec.uid = Some(request.uid);
        spec.gid = Some(request.gid);
        spec.output_cap_bytes = self.output_cap_bytes;

        let outcome =
            run(&spec).map_err(|error: ProcessError| CheckoutError::Process(error.to_string()))?;

        if !outcome.succeeded() {
            // Never echo raw Git output (it can contain server text); report the
            // exit status only.
            return Err(CheckoutError::Git(format!(
                "exit code {:?}",
                outcome.exit_code
            )));
        }
        Ok(outcome)
    }

    fn environment(&self, request: &CheckoutRequest<'_>) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        env.insert(
            "HOME".to_string(),
            request.staging_dir.display().to_string(),
        );
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env.insert("LC_ALL".to_string(), "C".to_string());
        // No system/global config, no interactive prompt, no SSH, HTTPS only.
        env.insert("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string());
        env.insert("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string());
        env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
        env.insert(
            "GIT_ALLOW_PROTOCOL".to_string(),
            self.allow_protocol.clone(),
        );
        env.insert("GIT_SSH_COMMAND".to_string(), "/bin/false".to_string());
        env.insert(
            "GIT_ASKPASS".to_string(),
            request
                .askpass
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "/bin/false".to_string()),
        );
        env
    }
}

fn is_commit(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_staging_dir() {
        let runner = GitRunner::default();
        let reference = GitRef::Name("main".to_string());
        let request = CheckoutRequest {
            repository_url: "https://github.com/example/repo.git",
            git_ref: &reference,
            staging_dir: Path::new("relative"),
            uid: 1000,
            gid: 1000,
            deadline: Duration::from_secs(5),
            askpass: None,
        };
        let error = runner.checkout(&request).unwrap_err();
        assert!(matches!(error, CheckoutError::InvalidRequest(_)));
    }

    #[test]
    fn commit_detection() {
        assert!(is_commit("0f6c5f2a4c4b4f1e9c9b2f1c8a7d6e550f6c5f2a"));
        assert!(!is_commit("main"));
        assert!(!is_commit("0f6c5f2a"));
        assert!(!is_commit("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"));
    }
}
