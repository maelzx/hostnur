//! Phase 14 root-run provider tests: managed Nginx routing, transactional
//! reload/rollback and a PHP-FPM-backed site.
//!
//! ```bash
//! cargo test --locked --no-run
//! sudo target/debug/deps/provider_vm-<hash> --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hostnur_agent::contract::Uuid;
use hostnur_agent::provider::{
    FpmManager, ManagedPool, ManagedSite, NginxManager, SiteKind, render_pool, render_site,
};
use hostnur_agent::provision::{Accounts, SystemAccounts, policy};

fn require_root() {
    assert!(
        nix::unistd::geteuid().is_root(),
        "this test must run as root on a disposable VM"
    );
}

fn sh(program: &str, args: &[&str]) -> std::process::Output {
    Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run {program}: {error}"))
}

fn ensure_managed_include() {
    std::fs::create_dir_all("/etc/nginx/hostnur-sites").unwrap();
    let include = "/etc/nginx/conf.d/hostnur-sites.conf";
    if !std::path::Path::new(include).exists() {
        std::fs::write(include, "include /etc/nginx/hostnur-sites/*.conf;\n").unwrap();
    }
    let _ = sh("/usr/bin/systemctl", &["start", "nginx"]);
}

fn curl(url: &str, host: &str) -> String {
    let output = sh(
        "/usr/bin/curl",
        &["-s", "-H", &format!("Host: {host}"), url],
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Poll until `check` is true or the timeout elapses (reloads are asynchronous).
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

#[test]
#[ignore = "requires root and a disposable VM"]
fn nginx_site_reload_and_rollback() {
    require_root();
    ensure_managed_include();

    let docroot = tempfile::tempdir().unwrap();
    std::fs::write(docroot.path().join("index.html"), b"hostnur-p14-static").unwrap();
    std::fs::set_permissions(
        docroot.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let site = ManagedSite {
        name: "hn-p14-static".to_string(),
        server_name: "127.0.0.1".to_string(),
        root: docroot.path().to_path_buf(),
        fpm_socket: None,
        kind: SiteKind::Static,
    };
    let manager = NginxManager::default();
    let content = render_site(&site).unwrap();
    manager.apply(&site.name, &content).expect("apply site");

    assert!(
        wait_until(
            || curl("http://127.0.0.1/", "127.0.0.1").contains("hostnur-p14-static"),
            Duration::from_secs(5)
        ),
        "static site did not serve in time"
    );

    // An invalid candidate must fail validation and leave the live file intact.
    let error = manager
        .apply(&site.name, "server { this is not valid nginx\n")
        .unwrap_err();
    assert!(matches!(
        error,
        hostnur_agent::provider::NginxError::ConfigTestFailed(_)
    ));
    let restored = std::fs::read_to_string("/etc/nginx/hostnur-sites/hn-p14-static.conf").unwrap();
    assert!(restored.contains("try_files $uri $uri/ =404;"));

    manager.remove(&site.name).expect("remove site");
    assert!(!std::path::Path::new("/etc/nginx/hostnur-sites/hn-p14-static.conf").exists());
}

#[test]
#[ignore = "requires root and a disposable VM"]
fn php_fpm_site_serves_php() {
    require_root();
    ensure_managed_include();

    let app = Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap();
    let accounts = SystemAccounts::default();
    let name = policy::account_name(&app);
    let _ = accounts.delete(&name);
    let ids = accounts.create(&name).expect("create app account");

    let docroot = tempfile::tempdir().unwrap();
    std::fs::write(
        docroot.path().join("index.php"),
        b"<?php echo 'hostnur-p14-php';",
    )
    .unwrap();
    let _ = sh(
        "/usr/bin/chown",
        &[
            "-R",
            &format!("{}:{}", ids.uid, ids.gid),
            docroot.path().to_str().unwrap(),
        ],
    );
    std::fs::set_permissions(
        docroot.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let pool_name = "hn-p14-php";
    let socket = PathBuf::from(format!("/run/php/{pool_name}.sock"));
    let pool = ManagedPool {
        name: pool_name.to_string(),
        user: name.clone(),
        group: name.clone(),
        socket: socket.clone(),
        max_children: 5,
    };
    let fpm = FpmManager::default();
    fpm.apply(pool_name, &render_pool(&pool).unwrap())
        .expect("apply pool");
    assert!(
        wait_until(|| socket.exists(), Duration::from_secs(5)),
        "fpm socket should appear after reload"
    );

    let site = ManagedSite {
        name: "hn-p14-php".to_string(),
        server_name: "127.0.0.1".to_string(),
        root: docroot.path().to_path_buf(),
        fpm_socket: Some(socket),
        kind: SiteKind::Laravel,
    };
    let manager = NginxManager::default();
    manager
        .apply(&site.name, &render_site(&site).unwrap())
        .expect("apply php site");

    assert!(
        wait_until(
            || curl("http://127.0.0.1/index.php", "127.0.0.1").contains("hostnur-p14-php"),
            Duration::from_secs(5)
        ),
        "php site did not serve in time"
    );

    manager.remove(&site.name).unwrap();
    fpm.remove(pool_name).unwrap();
    accounts.delete(&name).unwrap();
}
