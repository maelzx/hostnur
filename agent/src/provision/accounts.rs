//! Isolated Linux account creation for managed applications (phase 12.2).
//!
//! Uses fixed, allowlisted system tools invoked with a validated argument array
//! (never a shell), then verifies the resulting identity from the NSS database
//! rather than trusting command output. Managed accounts have no login shell and
//! never join the panel socket group.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, ProcessError, run, validate_spec};

use super::policy;
use super::service::ProvisionError;

/// Resolved Linux identity of a managed account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountIds {
    pub uid: u32,
    pub gid: u32,
}

/// Account operations, abstracted so provisioning policy is testable without root.
pub trait Accounts {
    fn lookup(&self, name: &str) -> Option<AccountIds>;
    fn create(&self, name: &str) -> Result<AccountIds, ProvisionError>;
    fn delete(&self, name: &str) -> Result<(), ProvisionError>;
}

/// Real account management via `groupadd`/`useradd`/`userdel`/`groupdel`.
#[derive(Debug, Clone)]
pub struct SystemAccounts {
    pub groupadd: PathBuf,
    pub useradd: PathBuf,
    pub userdel: PathBuf,
    pub groupdel: PathBuf,
    pub shell: String,
    pub deadline: Duration,
}

impl Default for SystemAccounts {
    fn default() -> Self {
        Self {
            groupadd: PathBuf::from("/usr/sbin/groupadd"),
            useradd: PathBuf::from("/usr/sbin/useradd"),
            userdel: PathBuf::from("/usr/sbin/userdel"),
            groupdel: PathBuf::from("/usr/sbin/groupdel"),
            shell: "/usr/sbin/nologin".to_string(),
            deadline: Duration::from_secs(30),
        }
    }
}

impl SystemAccounts {
    fn run_tool(&self, program: &Path, args: &[String]) -> Result<(), ProvisionError> {
        let mut spec = CommandSpec::new(program, self.deadline);
        spec.args = args.to_vec();
        spec.output_cap_bytes = 4096;

        validate_spec(&spec).map_err(|error: ProcessError| ProvisionError::AccountToolFailed {
            tool: program.display().to_string(),
            detail: error.to_string(),
        })?;

        let outcome = run(&spec).map_err(|error| ProvisionError::AccountToolFailed {
            tool: program.display().to_string(),
            detail: error.to_string(),
        })?;

        if !outcome.succeeded() {
            return Err(ProvisionError::AccountToolFailed {
                tool: program.display().to_string(),
                detail: format!("exit code {:?}", outcome.exit_code),
            });
        }
        Ok(())
    }
}

impl Accounts for SystemAccounts {
    fn lookup(&self, name: &str) -> Option<AccountIds> {
        let user = nix::unistd::User::from_name(name).ok().flatten()?;
        Some(AccountIds {
            uid: user.uid.as_raw(),
            gid: user.gid.as_raw(),
        })
    }

    /// Idempotent against partial failures: a group that already exists (or a
    /// user created after a lost race) is adopted after read-back verification,
    /// so a retried provision never wedges on `groupadd` exit 9/10.
    fn create(&self, name: &str) -> Result<AccountIds, ProvisionError> {
        if !policy::is_valid_account_name(name) {
            return Err(ProvisionError::InvalidIdentifier);
        }

        let group = nix::unistd::Group::from_name(name).map_err(|error| {
            ProvisionError::AccountToolFailed {
                tool: "getgrnam".to_string(),
                detail: error.to_string(),
            }
        })?;
        if group.is_none() {
            self.run_tool(&self.groupadd, &["--system".to_string(), name.to_string()])?;
        }

        if self.lookup(name).is_none() {
            self.run_tool(
                &self.useradd,
                &[
                    "--system".to_string(),
                    "--no-create-home".to_string(),
                    "--shell".to_string(),
                    self.shell.clone(),
                    "--gid".to_string(),
                    name.to_string(),
                    name.to_string(),
                ],
            )?;
        }

        // The result is verified from NSS, not trusted from tool output: the
        // primary group must be the derived one and the shell must be no-login.
        let user = nix::unistd::User::from_name(name)
            .ok()
            .flatten()
            .ok_or_else(|| ProvisionError::AccountMissing {
                name: name.to_string(),
            })?;
        let group = nix::unistd::Group::from_name(name).ok().flatten();
        let group_matches = group
            .as_ref()
            .is_some_and(|group| group.gid == user.gid && group.name == *name);
        let shell_matches = user.shell == std::path::Path::new(&self.shell);
        if !group_matches || !shell_matches {
            return Err(ProvisionError::IdentityDrift {
                name: name.to_string(),
            });
        }
        Ok(AccountIds {
            uid: user.uid.as_raw(),
            gid: user.gid.as_raw(),
        })
    }

    fn delete(&self, name: &str) -> Result<(), ProvisionError> {
        if !policy::is_valid_account_name(name) {
            return Err(ProvisionError::InvalidIdentifier);
        }
        // Best-effort removal of a partially created identity.
        let _ = self.run_tool(&self.userdel, &[name.to_string()]);
        let _ = self.run_tool(&self.groupdel, &[name.to_string()]);
        Ok(())
    }
}
