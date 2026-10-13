//! Provisioning service: reservation, namespace policy and unmanaged-resource
//! refusal. Later slices extend it to create accounts and the layout.

use std::path::{Path, PathBuf};

use crate::contract::Uuid;
use crate::journal::{Journal, JournalError, ManagedApplication};

use super::policy::{self, Lifecycle};

/// Reports observed host state so policy decisions can be tested without root.
pub trait HostProbe {
    /// Whether an OS account with this name exists.
    fn account_exists(&self, name: &str) -> bool;
    /// Whether any filesystem entry (including a symlink) exists at the path.
    fn path_exists(&self, path: &Path) -> bool;
}

/// Real host probe backed by `getpwnam` and `lstat`.
pub struct SystemHostProbe;

impl HostProbe for SystemHostProbe {
    fn account_exists(&self, name: &str) -> bool {
        matches!(nix::unistd::User::from_name(name), Ok(Some(_)))
    }

    fn path_exists(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }
}

/// Result of reserving an application identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub application: ManagedApplication,
    /// True when the identity already existed (idempotent re-request).
    pub reused: bool,
}

/// Provisioning policy failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionError {
    /// The derived identifier failed validation.
    InvalidIdentifier,
    /// A different application already owns the derived account name.
    AccountCollision { account: String },
    /// An OS account exists that the registry does not own.
    UnmanagedAccount { account: String },
    /// A filesystem entry exists that the registry does not own.
    UnmanagedPath { path: PathBuf },
    /// The stored registry row disagrees with the derived identity.
    RegistryMismatch,
    /// A journal operation failed.
    Registry(String),
}

impl std::fmt::Display for ProvisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvisionError::InvalidIdentifier => f.write_str("invalid application identifier"),
            ProvisionError::AccountCollision { account } => {
                write!(
                    f,
                    "account name `{account}` is already owned by another application"
                )
            }
            ProvisionError::UnmanagedAccount { account } => {
                write!(
                    f,
                    "OS account `{account}` exists but is not managed by the agent"
                )
            }
            ProvisionError::UnmanagedPath { path } => {
                write!(
                    f,
                    "path {} exists but is not managed by the agent",
                    path.display()
                )
            }
            ProvisionError::RegistryMismatch => {
                f.write_str("registry identity does not match the derived identifier")
            }
            ProvisionError::Registry(message) => write!(f, "registry error: {message}"),
        }
    }
}

impl std::error::Error for ProvisionError {}

impl From<JournalError> for ProvisionError {
    fn from(error: JournalError) -> Self {
        ProvisionError::Registry(error.to_string())
    }
}

/// Coordinates registry and host state for managed applications.
pub struct ProvisioningService<'a> {
    journal: &'a Journal,
    probe: &'a dyn HostProbe,
}

impl<'a> ProvisioningService<'a> {
    pub fn new(journal: &'a Journal, probe: &'a dyn HostProbe) -> Self {
        Self { journal, probe }
    }

    /// Reserve an application identity, refusing collisions and unmanaged host
    /// state. Idempotent: an existing reservation is returned unchanged.
    pub fn reserve(
        &self,
        application_id: &Uuid,
        provider: Option<&str>,
        revision: Option<&str>,
    ) -> Result<Reservation, ProvisionError> {
        let account_name = policy::account_name(application_id);
        if !policy::is_valid_account_name(&account_name) {
            return Err(ProvisionError::InvalidIdentifier);
        }
        let paths = policy::layout(application_id);

        if let Some(existing) = self.journal.managed_application(application_id)? {
            if existing.account_name != account_name {
                return Err(ProvisionError::RegistryMismatch);
            }
            return Ok(Reservation {
                application: existing,
                reused: true,
            });
        }

        if let Some(by_name) = self.journal.managed_application_by_account(&account_name)?
            && by_name.application_id != *application_id
        {
            return Err(ProvisionError::AccountCollision {
                account: account_name,
            });
        }

        if self.probe.account_exists(&account_name) {
            return Err(ProvisionError::UnmanagedAccount {
                account: account_name,
            });
        }
        if self.probe.path_exists(&paths.root) {
            return Err(ProvisionError::UnmanagedPath { path: paths.root });
        }

        let application = ManagedApplication {
            application_id: application_id.clone(),
            account_name: account_name.clone(),
            account_group: account_name,
            account_uid: None,
            account_gid: None,
            layout_root: paths.root.to_string_lossy().into_owned(),
            lifecycle_state: Lifecycle::Reserved.as_str().to_string(),
            provider: provider.map(str::to_string),
            revision: revision.map(str::to_string),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let created = self.journal.insert_managed_application(&application)?;
        let stored = self
            .journal
            .managed_application(application_id)?
            .ok_or(ProvisionError::RegistryMismatch)?;

        Ok(Reservation {
            application: stored,
            reused: !created,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct FakeProbe {
        accounts: HashSet<String>,
        paths: HashSet<PathBuf>,
    }

    impl FakeProbe {
        fn empty() -> Self {
            Self {
                accounts: HashSet::new(),
                paths: HashSet::new(),
            }
        }
    }

    impl HostProbe for FakeProbe {
        fn account_exists(&self, name: &str) -> bool {
            self.accounts.contains(name)
        }
        fn path_exists(&self, path: &Path) -> bool {
            self.paths.contains(path)
        }
    }

    fn id(value: &str) -> Uuid {
        Uuid::parse(value).unwrap()
    }

    #[test]
    fn reserve_creates_then_reuses() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        let first = service
            .reserve(&app, Some("php-laravel"), Some("rev-1"))
            .unwrap();
        assert!(!first.reused);
        assert_eq!(first.application.lifecycle_state, "reserved");
        assert_eq!(first.application.account_name, "hn_224065ac4bc7");

        let second = service.reserve(&app, None, None).unwrap();
        assert!(second.reused);
        assert_eq!(second.application, first.application);
    }

    #[test]
    fn refuses_unmanaged_account() {
        let journal = Journal::open_in_memory().unwrap();
        let mut probe = FakeProbe::empty();
        probe.accounts.insert("hn_224065ac4bc7".to_string());
        let service = ProvisioningService::new(&journal, &probe);

        let error = service
            .reserve(&id("224065ac-4bc7-4ba4-a06d-8c8bca950c32"), None, None)
            .unwrap_err();
        assert!(matches!(error, ProvisionError::UnmanagedAccount { .. }));
        // Nothing was reserved as a side effect.
        assert!(
            journal
                .managed_application(&id("224065ac-4bc7-4ba4-a06d-8c8bca950c32"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn refuses_unmanaged_path() {
        let journal = Journal::open_in_memory().unwrap();
        let mut probe = FakeProbe::empty();
        probe.paths.insert(PathBuf::from(
            "/opt/hostnur/apps/224065ac-4bc7-4ba4-a06d-8c8bca950c32",
        ));
        let service = ProvisioningService::new(&journal, &probe);

        let error = service
            .reserve(&id("224065ac-4bc7-4ba4-a06d-8c8bca950c32"), None, None)
            .unwrap_err();
        assert!(matches!(error, ProvisionError::UnmanagedPath { .. }));
    }

    #[test]
    fn refuses_account_name_collision() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let service = ProvisioningService::new(&journal, &probe);

        let app_b = id("d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a");
        let derived = policy::account_name(&app_b);

        // A different application already owns B's derived account name.
        let squatter = ManagedApplication {
            application_id: id("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            account_name: derived.clone(),
            account_group: derived,
            account_uid: None,
            account_gid: None,
            layout_root: "/opt/hostnur/apps/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string(),
            lifecycle_state: "reserved".to_string(),
            provider: None,
            revision: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(journal.insert_managed_application(&squatter).unwrap());

        let error = service.reserve(&app_b, None, None).unwrap_err();
        assert!(
            matches!(error, ProvisionError::AccountCollision { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn refuses_registry_identity_mismatch() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let service = ProvisioningService::new(&journal, &probe);

        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");
        // A row for this id but with a corrupted/foreign account name.
        let corrupt = ManagedApplication {
            application_id: app.clone(),
            account_name: "hn_ffffffffffff".to_string(),
            account_group: "hn_ffffffffffff".to_string(),
            account_uid: None,
            account_gid: None,
            layout_root: "/opt/hostnur/apps/224065ac-4bc7-4ba4-a06d-8c8bca950c32".to_string(),
            lifecycle_state: "reserved".to_string(),
            provider: None,
            revision: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(journal.insert_managed_application(&corrupt).unwrap());

        let error = service.reserve(&app, None, None).unwrap_err();
        assert!(
            matches!(error, ProvisionError::RegistryMismatch),
            "{error:?}"
        );
    }
}
