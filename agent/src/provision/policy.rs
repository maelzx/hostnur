//! Managed-application namespace, layout and lifecycle policy (phase 12).
//!
//! Every managed path and account name is derived from the application UUID, so
//! no caller-supplied string can name a privileged path or account. Display
//! names/slugs are deliberately not used for identity.

use std::path::{Path, PathBuf};

use crate::contract::Uuid;

/// Root under which all managed application trees live. Agent/root-owned.
pub const MANAGED_APPS_ROOT: &str = "/opt/hostnur/apps";

/// Account-name prefix for managed application identities.
pub const ACCOUNT_PREFIX: &str = "hn_";

/// Number of UUID hex characters used in an account name.
pub const ACCOUNT_HEX_LEN: usize = 12;

/// POSIX user/group name length limit.
pub const MAX_ACCOUNT_NAME: usize = 32;

/// Web server account that serves static files and talks to the FPM sockets.
/// It joins each application group so it can enter the application root.
pub const WEB_SERVER_ACCOUNT: &str = "www-data";

/// Directory/file modes for the managed layout. The application root is
/// group-owned by the application group, so other applications (and other
/// local users) cannot enter it at all; only the app account and the web
/// server, a member of the group, can.
pub const MODE_ROOT_DIR: u32 = 0o750;
pub const MODE_RELEASES: u32 = 0o755;
pub const MODE_SHARED: u32 = 0o750;
pub const MODE_SECRETS: u32 = 0o700;
pub const MODE_SECRET_FILE: u32 = 0o600;

/// Derive the bounded account name for an application UUID, e.g.
/// `hn_224065ac4bc7`.
pub fn account_name(application_id: &Uuid) -> String {
    let hex: String = application_id
        .as_str()
        .chars()
        .filter(|character| *character != '-')
        .collect();
    let fragment = &hex[..ACCOUNT_HEX_LEN.min(hex.len())];
    format!("{ACCOUNT_PREFIX}{fragment}")
}

/// Defensive validation of a derived account name. Derived names are always
/// valid; this guards against a corrupted registry value being reused as an OS
/// account name.
pub fn is_valid_account_name(name: &str) -> bool {
    if name.len() > MAX_ACCOUNT_NAME {
        return false;
    }
    let Some(fragment) = name.strip_prefix(ACCOUNT_PREFIX) else {
        return false;
    };
    fragment.len() == ACCOUNT_HEX_LEN && fragment.chars().all(|c| c.is_ascii_hexdigit())
}

/// Derived layout paths for an application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPaths {
    pub root: PathBuf,
    pub releases: PathBuf,
    pub shared: PathBuf,
    pub secrets: PathBuf,
    pub current: PathBuf,
}

impl LayoutPaths {
    /// Every directory that provisioning creates, in order.
    pub fn directories(&self) -> [&Path; 4] {
        [
            self.root.as_path(),
            self.releases.as_path(),
            self.shared.as_path(),
            self.secrets.as_path(),
        ]
    }
}

/// Compute the managed layout for an application UUID.
pub fn layout(application_id: &Uuid) -> LayoutPaths {
    let root = Path::new(MANAGED_APPS_ROOT).join(application_id.as_str());
    LayoutPaths {
        releases: root.join("releases"),
        shared: root.join("shared"),
        secrets: root.join("shared").join("secrets"),
        current: root.join("current"),
        root,
    }
}

/// Lifecycle of a managed application. Partial states are recorded so a crash
/// can be reconciled rather than restarting from scratch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// Row reserved in the registry; no host changes yet.
    Reserved,
    /// Linux group/account created (uid/gid recorded).
    AccountCreated,
    /// Directory layout created with the intended modes/ownership.
    LayoutPrepared,
    /// Fully provisioned and validated.
    Ready,
    /// Provisioning failed part-way; recovery required.
    Failed,
    /// Observed host state disagrees with the registry.
    Drifted,
}

impl Lifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Lifecycle::Reserved => "reserved",
            Lifecycle::AccountCreated => "account_created",
            Lifecycle::LayoutPrepared => "layout_prepared",
            Lifecycle::Ready => "ready",
            Lifecycle::Failed => "failed",
            Lifecycle::Drifted => "drifted",
        }
    }

    pub fn parse(value: &str) -> Option<Lifecycle> {
        Some(match value {
            "reserved" => Lifecycle::Reserved,
            "account_created" => Lifecycle::AccountCreated,
            "layout_prepared" => Lifecycle::LayoutPrepared,
            "ready" => Lifecycle::Ready,
            "failed" => Lifecycle::Failed,
            "drifted" => Lifecycle::Drifted,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> Uuid {
        Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap()
    }

    #[test]
    fn derives_bounded_account_name() {
        let name = account_name(&id());
        assert_eq!(name, "hn_224065ac4bc7");
        assert!(name.len() <= MAX_ACCOUNT_NAME);
        assert!(is_valid_account_name(&name));
    }

    #[test]
    fn rejects_invalid_account_names() {
        for name in [
            "",
            "root",
            "hn_",
            "hn_short",
            "hn_GGGGGGGGGGGG",
            "xhn_224065ac4bc7",
            "hn_224065ac4bc7extra",
        ] {
            assert!(!is_valid_account_name(name), "{name} should be invalid");
        }
    }

    #[test]
    fn layout_is_beneath_managed_root() {
        let paths = layout(&id());
        assert_eq!(
            paths.root,
            PathBuf::from("/opt/hostnur/apps/224065ac-4bc7-4ba4-a06d-8c8bca950c32")
        );
        assert_eq!(paths.secrets, paths.shared.join("secrets"));
        assert!(paths.root.starts_with(MANAGED_APPS_ROOT));
    }

    #[test]
    fn lifecycle_round_trips() {
        for state in [
            Lifecycle::Reserved,
            Lifecycle::AccountCreated,
            Lifecycle::LayoutPrepared,
            Lifecycle::Ready,
            Lifecycle::Failed,
            Lifecycle::Drifted,
        ] {
            assert_eq!(Lifecycle::parse(state.as_str()), Some(state));
        }
        assert_eq!(Lifecycle::parse("bogus"), None);
    }
}
