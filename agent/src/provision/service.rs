//! Provisioning service: reservation, namespace policy and unmanaged-resource
//! refusal. Later slices extend it to create accounts and the layout.

use std::path::{Path, PathBuf};

use crate::contract::Uuid;
use crate::journal::{Journal, JournalError, ManagedApplication};

use super::accounts::Accounts;
use super::policy::{self, Lifecycle};
use super::safe_fs::ManagedRoot;

/// One layout entry: (relative path, mode, optional owner uid/gid).
type LayoutPlan = Vec<(String, u32, Option<(u32, u32)>)>;

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
    /// A system account-management tool failed.
    AccountToolFailed { tool: String, detail: String },
    /// A managed account could not be resolved after creation.
    AccountMissing { name: String },
    /// Observed identity disagrees with the recorded identity (drift).
    IdentityDrift { name: String },
    /// A managed filesystem operation failed.
    Filesystem(String),
    /// A managed directory does not have the intended mode/ownership.
    LayoutMismatch { path: String },
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
            ProvisionError::AccountToolFailed { tool, detail } => {
                write!(f, "account tool `{tool}` failed: {detail}")
            }
            ProvisionError::AccountMissing { name } => {
                write!(f, "managed account `{name}` was not found after creation")
            }
            ProvisionError::IdentityDrift { name } => {
                write!(f, "identity for `{name}` disagrees with the registry")
            }
            ProvisionError::Filesystem(detail) => write!(f, "filesystem error: {detail}"),
            ProvisionError::LayoutMismatch { path } => {
                write!(f, "managed layout `{path}` has unexpected mode/ownership")
            }
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

    /// Create (or reconcile) the isolated Linux account for an application.
    ///
    /// Idempotent and crash-safe: a reservation with no account creates one; a
    /// reservation whose account already exists adopts the observed identity
    /// (resuming after a crash) unless it disagrees with a recorded identity,
    /// which is drift.
    pub fn provision_identity(
        &self,
        application_id: &Uuid,
        provider: Option<&str>,
        revision: Option<&str>,
        accounts: &dyn Accounts,
    ) -> Result<ManagedApplication, ProvisionError> {
        let reservation = self.reserve(application_id, provider, revision)?;
        let name = reservation.application.account_name.clone();
        let observed = accounts.lookup(&name);

        match (
            observed,
            reservation.application.account_uid,
            reservation.application.account_gid,
        ) {
            (Some(ids), Some(uid), Some(gid)) => {
                if ids.uid != uid || ids.gid != gid {
                    self.record_state(
                        application_id,
                        Lifecycle::Drifted,
                        Some(ids.uid),
                        Some(ids.gid),
                    )?;
                    return Err(ProvisionError::IdentityDrift { name });
                }
            }
            (Some(ids), _, _) => {
                // Resume: account exists but the registry had not recorded it.
                self.record_state(
                    application_id,
                    Lifecycle::AccountCreated,
                    Some(ids.uid),
                    Some(ids.gid),
                )?;
            }
            (None, Some(_), _) => {
                // Registry claims an account that no longer exists.
                self.record_state(application_id, Lifecycle::Drifted, None, None)?;
                return Err(ProvisionError::IdentityDrift { name });
            }
            (None, None, _) => {
                let ids = accounts.create(&name)?;
                self.record_state(
                    application_id,
                    Lifecycle::AccountCreated,
                    Some(ids.uid),
                    Some(ids.gid),
                )?;
            }
        }

        // Before the layout is tightened, so the web server never loses access.
        accounts.grant_web_access(&name)?;

        self.journal
            .managed_application(application_id)?
            .ok_or(ProvisionError::RegistryMismatch)
    }

    fn record_state(
        &self,
        application_id: &Uuid,
        lifecycle: Lifecycle,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<(), ProvisionError> {
        self.journal.update_managed_application_state(
            application_id,
            lifecycle.as_str(),
            uid,
            gid,
        )?;
        Ok(())
    }

    /// Create and verify the managed directory layout beneath `root`.
    ///
    /// Intermediate trusted parents are root-owned; the writable `shared` tree
    /// is owned by the application account. Modes/ownership are read back and
    /// checked before the application is marked ready.
    pub fn prepare_layout(
        &self,
        application_id: &Uuid,
        root: &ManagedRoot,
    ) -> Result<ManagedApplication, ProvisionError> {
        let app = self
            .journal
            .managed_application(application_id)?
            .ok_or(ProvisionError::RegistryMismatch)?;
        let name = app.account_name.clone();
        let uid = app
            .account_uid
            .ok_or(ProvisionError::AccountMissing { name: name.clone() })?;
        let gid = app
            .account_gid
            .ok_or(ProvisionError::AccountMissing { name })?;

        let uuid = application_id.as_str();
        // The root stays owned by the agent (root on a host) so the app account
        // cannot rename `releases` or `current`; only its group grants entry.
        let agent_uid = nix::unistd::geteuid().as_raw();
        let plan: LayoutPlan = vec![
            (
                uuid.to_string(),
                policy::MODE_ROOT_DIR,
                Some((agent_uid, gid)),
            ),
            (format!("{uuid}/releases"), policy::MODE_RELEASES, None),
            (
                format!("{uuid}/shared"),
                policy::MODE_SHARED,
                Some((uid, gid)),
            ),
            (
                format!("{uuid}/shared/secrets"),
                policy::MODE_SECRETS,
                Some((uid, gid)),
            ),
        ];

        for (relative, mode, owner) in &plan {
            root.create_dir(relative, *mode, *owner)
                .map_err(|error| ProvisionError::Filesystem(error.to_string()))?;
        }

        self.record_state(
            application_id,
            Lifecycle::LayoutPrepared,
            Some(uid),
            Some(gid),
        )?;

        for (relative, mode, owner) in &plan {
            let meta = root
                .metadata(relative)
                .map_err(|error| ProvisionError::Filesystem(error.to_string()))?;
            let owner_ok = match owner {
                Some((expected_uid, expected_gid)) => {
                    meta.uid == *expected_uid && meta.gid == *expected_gid
                }
                None => true,
            };
            if meta.mode != *mode || !owner_ok {
                self.record_state(application_id, Lifecycle::Failed, Some(uid), Some(gid))?;
                return Err(ProvisionError::LayoutMismatch {
                    path: relative.clone(),
                });
            }
        }

        self.record_state(application_id, Lifecycle::Ready, Some(uid), Some(gid))?;
        self.journal
            .managed_application(application_id)?
            .ok_or(ProvisionError::RegistryMismatch)
    }

    /// Resume or verify provisioning for an application from its recorded state.
    ///
    /// A partially provisioned application is completed; an already-ready
    /// application is re-verified, and a disagreeing layout is recorded as drift
    /// rather than silently accepted or recreated.
    pub fn reconcile(
        &self,
        application_id: &Uuid,
        root: &ManagedRoot,
        accounts: &dyn Accounts,
    ) -> Result<ManagedApplication, ProvisionError> {
        if self.journal.managed_application(application_id)?.is_none() {
            self.reserve(application_id, None, None)?;
        }
        let current = self
            .journal
            .managed_application(application_id)?
            .ok_or(ProvisionError::RegistryMismatch)?;
        let state =
            Lifecycle::parse(&current.lifecycle_state).ok_or(ProvisionError::RegistryMismatch)?;

        match state {
            Lifecycle::Reserved => {
                self.provision_identity(application_id, None, None, accounts)?;
                self.prepare_layout(application_id, root)
            }
            Lifecycle::AccountCreated => {
                self.provision_identity(application_id, None, None, accounts)?;
                self.prepare_layout(application_id, root)
            }
            Lifecycle::LayoutPrepared | Lifecycle::Ready => {
                self.provision_identity(application_id, None, None, accounts)?;
                match self.prepare_layout(application_id, root) {
                    Err(ProvisionError::LayoutMismatch { path }) => {
                        self.record_state(
                            application_id,
                            Lifecycle::Drifted,
                            current.account_uid,
                            current.account_gid,
                        )?;
                        Err(ProvisionError::LayoutMismatch { path })
                    }
                    other => other,
                }
            }
            Lifecycle::Failed | Lifecycle::Drifted => {
                // Re-run identity then layout; a genuine identity drift is
                // re-detected and preserved rather than overwritten.
                self.provision_identity(application_id, None, None, accounts)?;
                self.prepare_layout(application_id, root)
            }
        }
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

    struct FakeAccounts {
        ids: std::cell::RefCell<
            std::collections::HashMap<String, super::super::accounts::AccountIds>,
        >,
        next: std::cell::Cell<u32>,
        web_members: std::cell::RefCell<HashSet<String>>,
    }

    fn current_uid() -> u32 {
        nix::unistd::getuid().as_raw()
    }

    fn current_gid() -> u32 {
        nix::unistd::getgid().as_raw()
    }

    impl FakeAccounts {
        /// Start from the current process identity so unprivileged tests can
        /// exercise a real `chown` to the same uid/group.
        fn new() -> Self {
            Self {
                ids: std::cell::RefCell::new(std::collections::HashMap::new()),
                next: std::cell::Cell::new(current_uid()),
                web_members: std::cell::RefCell::new(HashSet::new()),
            }
        }
    }

    impl super::super::accounts::Accounts for FakeAccounts {
        fn lookup(&self, name: &str) -> Option<super::super::accounts::AccountIds> {
            self.ids.borrow().get(name).copied()
        }

        fn create(&self, name: &str) -> Result<super::super::accounts::AccountIds, ProvisionError> {
            let uid = self.next.get();
            self.next.set(uid + 1);
            let ids = super::super::accounts::AccountIds {
                uid,
                gid: current_gid(),
            };
            self.ids.borrow_mut().insert(name.to_string(), ids);
            Ok(ids)
        }

        fn delete(&self, name: &str) -> Result<(), ProvisionError> {
            self.ids.borrow_mut().remove(name);
            Ok(())
        }

        fn grant_web_access(&self, name: &str) -> Result<(), ProvisionError> {
            self.web_members.borrow_mut().insert(name.to_string());
            Ok(())
        }
    }

    #[test]
    fn provision_identity_creates_then_is_idempotent() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        let first = service
            .provision_identity(&app, Some("php-laravel"), Some("rev-1"), &accounts)
            .unwrap();
        assert_eq!(first.lifecycle_state, "account_created");
        assert_eq!(first.account_uid, Some(current_uid()));
        assert_eq!(first.account_gid, Some(current_gid()));
        assert_eq!(accounts.next.get(), current_uid() + 1);

        let second = service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        assert_eq!(second, first);
        // No new identity was allocated.
        assert_eq!(accounts.next.get(), current_uid() + 1);
    }

    #[test]
    fn provision_identity_resumes_after_crash() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        // Reserved but the account exists (crash before the registry update).
        service.reserve(&app, None, None).unwrap();
        accounts.create("hn_224065ac4bc7").expect("seed account");

        let resumed = service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        assert_eq!(resumed.lifecycle_state, "account_created");
        assert_eq!(resumed.account_uid, Some(current_uid()));
    }

    #[test]
    fn provision_identity_detects_changed_uid() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        // The account is recreated externally with a different UID.
        accounts.ids.borrow_mut().insert(
            "hn_224065ac4bc7".to_string(),
            super::super::accounts::AccountIds {
                uid: current_uid() + 100,
                gid: current_gid(),
            },
        );

        let error = service
            .provision_identity(&app, None, None, &accounts)
            .unwrap_err();
        assert!(
            matches!(error, ProvisionError::IdentityDrift { .. }),
            "{error:?}"
        );

        let state = journal.managed_application(&app).unwrap().unwrap();
        assert_eq!(state.lifecycle_state, "drifted");
    }

    #[test]
    fn prepare_layout_creates_verified_tree() {
        use std::os::unix::fs::MetadataExt;

        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");
        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        let ready = service.prepare_layout(&app, &root).unwrap();
        assert_eq!(ready.lifecycle_state, "ready");

        let secrets = dir.path().join(app.as_str()).join("shared").join("secrets");
        let meta = std::fs::metadata(&secrets).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o700);
        assert_eq!(meta.uid(), current_uid());

        let releases = dir.path().join(app.as_str()).join("releases");
        assert_eq!(std::fs::metadata(&releases).unwrap().mode() & 0o777, 0o755);

        // Only the app group (which the web server joins) may enter the root.
        let app_root = std::fs::metadata(dir.path().join(app.as_str())).unwrap();
        assert_eq!(app_root.mode() & 0o777, 0o750);
        assert_eq!(app_root.uid(), nix::unistd::geteuid().as_raw());
        assert_eq!(app_root.gid(), current_gid());
        assert!(accounts.web_members.borrow().contains("hn_224065ac4bc7"));
    }

    #[test]
    fn reconcile_tightens_a_root_from_an_older_release() {
        use std::os::unix::fs::PermissionsExt;

        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        service.prepare_layout(&app, &root).unwrap();

        // Releases up to 0.1.5 left the application root world-enterable.
        let app_root = dir.path().join(app.as_str());
        std::fs::set_permissions(&app_root, std::fs::Permissions::from_mode(0o755)).unwrap();
        accounts.web_members.borrow_mut().clear();

        let reconciled = service.reconcile(&app, &root, &accounts).unwrap();
        assert_eq!(reconciled.lifecycle_state, "ready");
        assert_eq!(
            std::fs::metadata(&app_root).unwrap().permissions().mode() & 0o777,
            policy::MODE_ROOT_DIR
        );
        assert!(accounts.web_members.borrow().contains("hn_224065ac4bc7"));
    }

    #[test]
    fn reconcile_resumes_from_partial_state() {
        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        service.prepare_layout(&app, &root).unwrap();

        // Simulate a crash that recorded only the layout-prepared checkpoint.
        journal
            .update_managed_application_state(
                &app,
                "layout_prepared",
                Some(current_uid()),
                Some(current_gid()),
            )
            .unwrap();

        let reconciled = service.reconcile(&app, &root, &accounts).unwrap();
        assert_eq!(reconciled.lifecycle_state, "ready");
    }

    #[test]
    fn reconcile_heals_a_loosened_mode() {
        use std::os::unix::fs::PermissionsExt;

        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");

        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();
        service.prepare_layout(&app, &root).unwrap();

        // Someone loosens the shared tree after provisioning.
        let shared = dir.path().join(app.as_str()).join("shared");
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();

        let reconciled = service.reconcile(&app, &root, &accounts).unwrap();
        assert_eq!(reconciled.lifecycle_state, "ready");
        assert_eq!(
            std::fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
            policy::MODE_SHARED
        );
    }

    #[test]
    fn reconcile_refuses_a_symlinked_layout() {
        use std::os::unix::fs::symlink;

        let journal = Journal::open_in_memory().unwrap();
        let probe = FakeProbe::empty();
        let accounts = FakeAccounts::new();
        let service = ProvisioningService::new(&journal, &probe);
        let app = id("224065ac-4bc7-4ba4-a06d-8c8bca950c32");
        service
            .provision_identity(&app, None, None, &accounts)
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let victim = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();

        // A symlink occupies the expected layout root; it must never be followed.
        symlink(victim.path(), dir.path().join(app.as_str())).unwrap();

        let error = service.reconcile(&app, &root, &accounts).unwrap_err();
        assert!(matches!(error, ProvisionError::Filesystem(_)), "{error:?}");
    }
}
