//! Phase 15 root-run deployment test: stage a release, activate it behind the
//! agent-owned `current` pointer, health-gate it, redeploy and roll back.
//!
//! ```bash
//! cargo test --locked --no-run
//! sudo target/debug/deps/deploy_vm-<hash> --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hostnur_agent::contract::Uuid;
use hostnur_agent::deploy::{DeployError, DeployInput, Deployer};
use hostnur_agent::git::{GitRef, GitRunner};
use hostnur_agent::journal::Journal;
use hostnur_agent::provider::{FpmManager, NginxManager};
use hostnur_agent::provision::{
    Accounts, ManagedRoot, ProvisioningService, SystemAccounts, SystemHostProbe, policy,
};
use tempfile::TempDir;

const APP: &str = "224065ac-4bc7-4ba4-a06d-8c8bca950c32";
const MANAGED_ROOT: &str = "/opt/hostnur/apps";

fn require_root() {
    assert!(
        nix::unistd::geteuid().is_root(),
        "run as root on a disposable VM"
    );
}

fn sh(program: &str, args: &[&str]) -> std::process::Output {
    Command::new(program).args(args).output().expect("spawn")
}

fn ensure_managed_include() {
    std::fs::create_dir_all("/etc/nginx/hostnur-sites").unwrap();
    let include = "/etc/nginx/conf.d/hostnur-sites.conf";
    if !std::path::Path::new(include).exists() {
        std::fs::write(include, "include /etc/nginx/hostnur-sites/*.conf;\n").unwrap();
    }
    let _ = sh("/usr/bin/systemctl", &["start", "nginx"]);
}

fn curl_marker() -> String {
    String::from_utf8_lossy(
        &sh(
            "/usr/bin/curl",
            &["-s", "-H", "Host: 127.0.0.1", "http://127.0.0.1/"],
        )
        .stdout,
    )
    .into_owned()
}

fn wait_until(mut check: impl FnMut() -> bool, timeout: Duration) -> bool {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

fn git(args: &[&str], cwd: &std::path::Path) -> String {
    let output = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Hostnur")
        .env("GIT_AUTHOR_EMAIL", "t@hostnur.invalid")
        .env("GIT_COMMITTER_NAME", "Hostnur")
        .env("GIT_COMMITTER_EMAIL", "t@hostnur.invalid")
        .output()
        .expect("git");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
#[ignore = "requires root and a disposable VM"]
fn deploy_activate_health_redeploy_and_rollback() {
    require_root();
    ensure_managed_include();

    let app = Uuid::parse(APP).unwrap();
    let accounts = SystemAccounts::default();
    let name = policy::account_name(&app);
    let _ = accounts.delete(&name);
    let _ = std::fs::remove_dir_all(PathBuf::from(MANAGED_ROOT).join(APP));

    let journal_dir = TempDir::new().unwrap();
    let journal = Journal::open(&journal_dir.path().join("agent.db")).unwrap();
    // Reserve the identity before creating the OS account so the registry owns
    // it (an unmanaged account would otherwise be refused).
    let probe = SystemHostProbe;
    ProvisioningService::new(&journal, &probe)
        .reserve(&app, Some("php-laravel"), None)
        .expect("reserve app");
    let ids = accounts.create(&name).expect("create app account");

    // Build a local repository with two commits.
    let dir = TempDir::new().unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(work.join("public")).unwrap();
    git(&["init", "--quiet", "-b", "main"], &work);
    std::fs::write(
        work.join("composer.json"),
        br#"{"name":"hostnur/fixture","require":{},"autoload":{"psr-4":{"Fixture\\":"src/"}}}"#,
    )
    .unwrap();
    std::fs::write(work.join("public/index.php"), b"<?php echo 'marker-v1';").unwrap();
    git(&["add", "."], &work);
    git(&["commit", "--quiet", "-m", "v1"], &work);
    let commit_v1 = git(&["rev-parse", "HEAD"], &work);

    std::fs::write(work.join("public/index.php"), b"<?php echo 'marker-v2';").unwrap();
    git(&["add", "."], &work);
    git(&["commit", "--quiet", "-m", "v2"], &work);
    let commit_v2 = git(&["rev-parse", "HEAD"], &work);

    let bare = dir.path().join("repo.git");
    sh(
        "/usr/bin/git",
        &[
            "clone",
            "--quiet",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    let _ = sh(
        "/usr/bin/chmod",
        &["-R", "a+rX", dir.path().to_str().unwrap()],
    );
    let _ = sh(
        "/usr/bin/chown",
        &[
            "-R",
            &format!("{}:{}", ids.uid, ids.gid),
            bare.to_str().unwrap(),
        ],
    );
    let repo_url = format!("file://{}", bare.display());

    std::fs::create_dir_all(MANAGED_ROOT).unwrap();
    let root = ManagedRoot::open(std::path::Path::new(MANAGED_ROOT)).unwrap();
    let deployer = Deployer {
        journal: &journal,
        accounts: &accounts,
        root: &root,
        git: GitRunner::default(),
        nginx: NginxManager::default(),
        fpm: FpmManager::default(),
        apps_root: PathBuf::from(MANAGED_ROOT),
        secrets_dir: journal_dir.path().join("secrets"),
    };

    let make_input = |release: &str, commit: &str| DeployInput {
        application_id: app.clone(),
        release_id: Uuid::parse(release).unwrap(),
        provider: "php-laravel",
        runtime: "php",
        runtime_version: "8.3",
        repository_url: &repo_url,
        git_ref: GitRef::Commit(commit.to_string()),
        server_name: "127.0.0.1",
        health_path: "/",
        allow_protocol: "file",
        run_migrations: false,
        recovery_acknowledged: false,
        build: true,
        env: BTreeMap::new(),
        execution_id: None,
    };

    // Deploy v1.
    let r1 = "11111111-1111-4111-8111-111111111111";
    let outcome = deployer
        .deploy(&make_input(r1, &commit_v1))
        .expect("deploy v1");
    assert_eq!(outcome.commit, commit_v1);
    assert!(wait_until(
        || curl_marker().contains("marker-v1"),
        Duration::from_secs(5)
    ));

    // The unprivileged Composer build produced an app-owned autoloader.
    use std::os::unix::fs::MetadataExt;
    let autoload = PathBuf::from(MANAGED_ROOT)
        .join(APP)
        .join("releases")
        .join(r1)
        .join("vendor/autoload.php");
    assert!(
        autoload.is_file(),
        "composer build should create vendor/autoload.php"
    );
    assert_eq!(std::fs::metadata(&autoload).unwrap().uid(), ids.uid);

    // Redeploy v2.
    let r2 = "22222222-2222-4222-8222-222222222222";
    deployer
        .deploy(&make_input(r2, &commit_v2))
        .expect("deploy v2");
    assert!(wait_until(
        || curl_marker().contains("marker-v2"),
        Duration::from_secs(5)
    ));

    // Roll back to v1.
    let r1_id = Uuid::parse(r1).unwrap();
    deployer
        .rollback(&app, &r1_id, "127.0.0.1", "/")
        .expect("rollback to v1");
    assert!(wait_until(
        || curl_marker().contains("marker-v1"),
        Duration::from_secs(5)
    ));

    // Migrations without a recovery prerequisite are refused.
    let mut input = make_input(r2, &commit_v2);
    input.run_migrations = true;
    assert!(matches!(
        deployer.deploy(&input),
        Err(DeployError::MigrationPrerequisiteMissing)
    ));

    // Cleanup.
    let _ = std::fs::remove_dir_all(PathBuf::from(MANAGED_ROOT).join(APP));
    accounts.delete(&name).unwrap();
    let _ = sh(
        "/usr/bin/rm",
        &["-f", "/etc/nginx/hostnur-sites/hn-224065ac4bc7.conf"],
    );
    let _ = sh(
        "/usr/bin/rm",
        &["-f", "/etc/php/8.3/fpm/pool.d/hostnur-hn-224065ac4bc7.conf"],
    );
}
