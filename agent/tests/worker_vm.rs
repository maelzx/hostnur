//! Phase 14 root-run worker/scheduler tests: app-identity systemd units.
//!
//! ```bash
//! cargo test --locked --no-run
//! sudo target/debug/deps/worker_vm-<hash> --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hostnur_agent::contract::Uuid;
use hostnur_agent::provider::{ScheduleUnit, WorkerManager, WorkerUnit};
use hostnur_agent::provision::{Accounts, SystemAccounts, policy};

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

fn out(program: &str, args: &[&str]) -> String {
    String::from_utf8_lossy(&sh(program, args).stdout)
        .trim()
        .to_string()
}

#[test]
#[ignore = "requires root and a disposable VM"]
fn worker_and_scheduler_run_as_the_app_user() {
    require_root();

    let app = Uuid::parse(APP).unwrap();
    let accounts = SystemAccounts::default();
    let name = policy::account_name(&app);
    let _ = accounts.delete(&name);
    let ids = accounts.create(&name).expect("create app account");

    let app_root = PathBuf::from(MANAGED_ROOT).join(APP);
    let shared = app_root.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(
        shared.join("worker.php"),
        b"<?php while (true) { sleep(1); }",
    )
    .unwrap();
    let _ = sh(
        "/usr/bin/chown",
        &[
            "-R",
            &format!("{}:{}", ids.uid, ids.gid),
            app_root.to_str().unwrap(),
        ],
    );

    let manager = WorkerManager::default();
    let worker_name = "hn-224065ac4bc7-queue".to_string();
    let _ = manager.remove(&worker_name);

    let unit = WorkerUnit {
        name: worker_name.clone(),
        user: name.clone(),
        group: name.clone(),
        working_dir: app_root.clone(),
        program: PathBuf::from("/usr/bin/php"),
        args: vec![shared.join("worker.php").display().to_string()],
        read_write_paths: vec![shared.clone()],
    };
    let service = manager.apply_worker(&unit).expect("apply worker");
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(
        out("/usr/bin/systemctl", &["is-active", &service]),
        "active"
    );
    let pid = out(
        "/usr/bin/systemctl",
        &["show", "-p", "MainPID", "--value", &service],
    );
    assert_ne!(pid, "0", "worker should have a main pid");
    let uid = out("/usr/bin/ps", &["-o", "uid=", "-p", &pid]);
    assert_eq!(
        uid.trim(),
        ids.uid.to_string(),
        "worker must run as the app user"
    );

    // Scheduler timer.
    let schedule_name = "hn-224065ac4bc7-sched".to_string();
    let _ = manager.remove(&schedule_name);
    let schedule = ScheduleUnit {
        name: schedule_name.clone(),
        user: name.clone(),
        group: name.clone(),
        working_dir: app_root.clone(),
        program: PathBuf::from("/usr/bin/php"),
        args: vec![shared.join("worker.php").display().to_string()],
        on_calendar: "*-*-* *:*:00".to_string(),
    };
    let timer = manager.apply_schedule(&schedule).expect("apply schedule");
    assert_eq!(out("/usr/bin/systemctl", &["is-active", &timer]), "active");

    manager.remove(&worker_name).unwrap();
    manager.remove(&schedule_name).unwrap();
    let _ = std::fs::remove_dir_all(&app_root);
    accounts.delete(&name).unwrap();
}
