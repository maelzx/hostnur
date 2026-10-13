//! Managed application isolation and provisioning (phase 12).
//!
//! Turns an approved application UUID into an isolated Linux identity and a
//! managed filesystem layout. Identity and paths are always derived from the
//! UUID; caller-supplied names/paths are never trusted, and unmanaged host
//! state is refused rather than adopted.

pub mod accounts;
pub mod policy;
pub mod safe_fs;
pub mod service;

pub use accounts::{AccountIds, Accounts, SystemAccounts};
pub use policy::{
    ACCOUNT_HEX_LEN, ACCOUNT_PREFIX, LayoutPaths, Lifecycle, MANAGED_APPS_ROOT, MAX_ACCOUNT_NAME,
    MODE_RELEASES, MODE_ROOT_DIR, MODE_SECRET_FILE, MODE_SECRETS, MODE_SHARED, account_name,
    is_valid_account_name, layout,
};
pub use safe_fs::{EntryMeta, MAX_FILE_BYTES, ManagedRoot};
pub use service::{HostProbe, ProvisionError, ProvisioningService, Reservation, SystemHostProbe};
