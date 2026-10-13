//! Phase 12.5 root-run isolation and race tests.
//!
//! These require root and a disposable Linux VM; run them directly as root:
//!
//! ```bash
//! cargo test --locked --no-run
//! sudo target/debug/deps/provision_vm-<hash> --ignored --nocapture
//! ```
//!
//! They create real `hn_<uuid>` accounts, a real managed layout under
//! `/opt/hostnur/apps`, and verify mode/ownership, cross-app access and symlink
//! handling as the actual application UIDs.

use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::time::Duration;

use hostnur_agent::contract::Uuid;
use hostnur_agent::journal::Journal;
use hostnur_agent::process::{CommandSpec, run};
use hostnur_agent::provision::{
    Accounts, ManagedRoot, ProvisioningService, SystemAccounts, policy,
};
use tempfile::TempDir;

const MANAGED_ROOT: &str = "/opt/hostnur/apps";
const APP_A: &str = "224065ac-4bc7-4ba4-a06d-8c8bca950c32";
const APP_B: &str = "d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a";

fn require_root() {
    assert!(
        nix::unistd::geteuid().is_root(),
        "this test must run as root on a disposable VM"
    );
}

fn cleanup(accounts: &SystemAccounts, application: &Uuid) {
    let _ = accounts.delete(&policy::account_name(application));
    let _ = std::fs::remove_dir_all(PathBuf::from(MANAGED_ROOT).join(application.as_str()));
}

#[test]
#[ignore = "requires root and a disposable VM"]
fn two_apps_pass_the_permission_matrix() {
    require_root();

    std::fs::create_dir_all(MANAGED_ROOT).expect("create managed root");
    std::fs::set_permissions(MANAGED_ROOT, std::fs::Permissions::from_mode(0o755)).unwrap();

    let accounts = SystemAccounts::default();
    let a = Uuid::parse(APP_A).unwrap();
    let b = Uuid::parse(APP_B).unwrap();
    cleanup(&accounts, &a);
    cleanup(&accounts, &b);

    let journal_dir = TempDir::new().unwrap();
    let journal = Journal::open(&journal_dir.path().join("agent.db")).unwrap();
    let probe = hostnur_agent::provision::SystemHostProbe;
    let service = ProvisioningService::new(&journal, &probe);
    let root = ManagedRoot::open(std::path::Path::new(MANAGED_ROOT)).unwrap();

    let app_a = service
        .reconcile(&a, &root, &accounts)
        .expect("provision app A");
    let app_b = service
        .reconcile(&b, &root, &accounts)
        .expect("provision app B");
    assert_eq!(app_a.lifecycle_state, "ready");
    assert_eq!(app_b.lifecycle_state, "ready");

    let uid_a = app_a.account_uid.unwrap();
    let gid_a = app_a.account_gid.unwrap();
    let uid_b = app_b.account_uid.unwrap();
    assert_ne!(uid_a, uid_b, "apps must not share a UID");

    // Layout modes and ownership.
    let base = PathBuf::from(MANAGED_ROOT).join(APP_A);
    assert_eq!(
        std::fs::metadata(&base).unwrap().mode() & 0o777,
        policy::MODE_ROOT_DIR
    );
    let shared = base.join("shared");
    let shared_meta = std::fs::metadata(&shared).unwrap();
    assert_eq!(shared_meta.mode() & 0o777, policy::MODE_SHARED);
    assert_eq!(shared_meta.uid(), uid_a);
    assert_eq!(shared_meta.gid(), gid_a);
    assert_eq!(
        std::fs::metadata(base.join("shared/secrets"))
            .unwrap()
            .mode()
            & 0o777,
        policy::MODE_SECRETS
    );

    // Write a private secret for each app.
    root.write_file(
        &format!("{APP_A}/shared/secrets/app.key"),
        policy::MODE_SECRET_FILE,
        Some((uid_a, gid_a)),
        b"secret-for-a",
    )
    .unwrap();
    root.write_file(
        &format!("{APP_B}/shared/secrets/app.key"),
        policy::MODE_SECRET_FILE,
        Some((uid_b, app_b.account_gid.unwrap())),
        b"secret-for-b",
    )
    .unwrap();

    let cat = |uid: u32, gid: u32, path: String| {
        let mut spec = CommandSpec::new("/usr/bin/cat", Duration::from_secs(5));
        spec.args = vec![path];
        spec.uid = Some(uid);
        spec.gid = Some(gid);
        run(&spec).expect("run cat")
    };

    // App A reads its own secret.
    let own = cat(
        uid_a,
        gid_a,
        format!("{MANAGED_ROOT}/{APP_A}/shared/secrets/app.key"),
    );
    assert!(own.succeeded(), "A must read its own secret: {own:?}");
    assert_eq!(own.stdout, b"secret-for-a");

    // App A cannot read app B's secret.
    let cross = cat(
        uid_a,
        gid_a,
        format!("{MANAGED_ROOT}/{APP_B}/shared/secrets/app.key"),
    );
    assert!(!cross.succeeded(), "A must not read B's secret: {cross:?}");

    // A symlink inside A's tree pointing at B must never be followed by the
    // privileged writer.
    let sneak = base.join("shared/sneak");
    symlink(PathBuf::from(MANAGED_ROOT).join(APP_B), &sneak).unwrap();
    let escape = root.write_file(
        &format!("{APP_A}/shared/sneak/secrets/app.key"),
        policy::MODE_SECRET_FILE,
        None,
        b"nope",
    );
    assert!(escape.is_err(), "symlinked parent must be refused");

    cleanup(&accounts, &a);
    cleanup(&accounts, &b);
}
