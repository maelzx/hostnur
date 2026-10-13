//! Phase 13.2 root-run checkout test: fetch a repository as the application
//! user and pin the resolved commit.
//!
//! ```bash
//! cargo test --locked --no-run
//! sudo target/debug/deps/git_vm-<hash> --ignored --nocapture
//! ```
//!
//! The repository is served locally via `file://` (policy restricts production
//! to HTTPS); this exercises the checkout primitive and identity, not the URL
//! policy, which is unit-tested separately.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hostnur_agent::contract::Uuid;
use hostnur_agent::git::{CheckoutRequest, GitRef, GitRunner};
use hostnur_agent::provision::{Accounts, SystemAccounts, policy};
use tempfile::TempDir;

const APP: &str = "224065ac-4bc7-4ba4-a06d-8c8bca950c32";

fn require_root() {
    assert!(
        nix::unistd::geteuid().is_root(),
        "this test must run as root on a disposable VM"
    );
}

fn git(args: &[&str], cwd: &std::path::Path) {
    let status = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Hostnur Test")
        .env("GIT_AUTHOR_EMAIL", "test@hostnur.invalid")
        .env("GIT_COMMITTER_NAME", "Hostnur Test")
        .env("GIT_COMMITTER_EMAIL", "test@hostnur.invalid")
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn chmod(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn chmod_tree(path: &std::path::Path, spec: &str) {
    let status = Command::new("/usr/bin/chmod")
        .arg("-R")
        .arg(spec)
        .arg(path)
        .status()
        .expect("chmod");
    assert!(status.success());
}

fn make_bare_repo(dir: &std::path::Path) -> PathBuf {
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    git(&["init", "--quiet", "-b", "main"], &work);
    std::fs::write(work.join("README.md"), b"# fixture\n").unwrap();
    git(&["add", "README.md"], &work);
    git(&["commit", "--quiet", "-m", "initial"], &work);

    let bare = dir.join("repo.git");
    Command::new("/usr/bin/git")
        .args(["clone", "--quiet", "--bare"])
        .arg(&work)
        .arg(&bare)
        .status()
        .expect("clone --bare");
    bare
}

#[test]
#[ignore = "requires root and a disposable VM"]
fn checks_out_as_the_application_user_and_pins_the_commit() {
    require_root();

    let accounts = SystemAccounts::default();
    let app = Uuid::parse(APP).unwrap();
    let _ = accounts.delete(&policy::account_name(&app));

    // Create the isolated account first so the served repository can be owned by
    // it: git refuses "dubious ownership" of a file:// remote otherwise.
    let ids = match nix::unistd::User::from_name(&policy::account_name(&app))
        .ok()
        .flatten()
    {
        Some(user) => hostnur_agent::provision::AccountIds {
            uid: user.uid.as_raw(),
            gid: user.gid.as_raw(),
        },
        None => accounts.create(&policy::account_name(&app)).unwrap(),
    };

    let dir = TempDir::new().unwrap();
    let bare = make_bare_repo(dir.path());
    // Traversable and app-owned so the app user can fetch it.
    chmod_tree(dir.path(), "a+rX");
    let status = Command::new("/usr/bin/chown")
        .arg("-R")
        .arg(format!("{}:{}", ids.uid, ids.gid))
        .arg(&bare)
        .status()
        .expect("chown repo");
    assert!(status.success());
    let repo_url = format!("file://{}", bare.display());

    std::fs::create_dir_all("/opt/hostnur/apps").ok();
    let staging_parent = TempDir::new().unwrap();
    chmod(staging_parent.path(), 0o755);
    let staging = staging_parent.path().join("release");
    std::fs::create_dir(&staging).unwrap();

    let _ = std::process::Command::new("/usr/bin/chown")
        .arg("-R")
        .arg(format!("{}:{}", ids.uid, ids.gid))
        .arg(&staging)
        .status();

    let runner = GitRunner {
        allow_protocol: "file".to_string(),
        ..GitRunner::default()
    };
    let reference = GitRef::Name("main".to_string());
    let request = CheckoutRequest {
        repository_url: &repo_url,
        git_ref: &reference,
        staging_dir: &staging,
        uid: ids.uid,
        gid: ids.gid,
        deadline: Duration::from_secs(60),
        askpass: None,
    };

    let outcome = runner.checkout(&request).expect("checkout as app user");
    assert_eq!(outcome.commit.len(), 40);
    assert!(outcome.commit.chars().all(|c| c.is_ascii_hexdigit()));

    let readme = staging.join("README.md");
    assert!(readme.exists(), "working tree must be populated");
    let meta = std::fs::metadata(&readme).unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!(meta.uid(), ids.uid, "checkout files must be app-owned");

    // A bad ref fails and leaves no partial staging tree behind.
    let staging2 = staging_parent.path().join("release2");
    std::fs::create_dir(&staging2).unwrap();
    let _ = std::process::Command::new("/usr/bin/chown")
        .arg("-R")
        .arg(format!("{}:{}", ids.uid, ids.gid))
        .arg(&staging2)
        .status();
    let bad = GitRef::Name("no-such-branch".to_string());
    let request2 = CheckoutRequest {
        repository_url: &repo_url,
        git_ref: &bad,
        staging_dir: &staging2,
        uid: ids.uid,
        gid: ids.gid,
        deadline: Duration::from_secs(30),
        askpass: None,
    };
    assert!(runner.checkout(&request2).is_err());
    assert!(!staging2.exists(), "failed staging must be cleaned up");

    let _ = accounts.delete(&policy::account_name(&app));
}
