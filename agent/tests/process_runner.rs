//! Phase 11.4 bounded process primitive checks.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use hostnur_agent::process::{CommandSpec, ProcessPolicy, run};
use nix::sys::signal::kill;
use nix::unistd::Pid;
use tempfile::TempDir;

#[test]
fn captures_output_without_a_shell() {
    let mut spec = CommandSpec::new("/usr/bin/echo", Duration::from_secs(5));
    // If a shell were involved, `;` and `$(...)` would be interpreted.
    spec.args = vec!["a;b".to_string(), "$(whoami)".to_string()];

    let outcome = run(&spec).expect("run echo");
    assert!(outcome.succeeded(), "{outcome:?}");
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert_eq!(stdout.trim_end(), "a;b $(whoami)");
}

#[test]
fn environment_is_not_inherited() {
    let spec = CommandSpec::new("/usr/bin/env", Duration::from_secs(5));
    let outcome = run(&spec).expect("run env");
    // env_clear() with no explicit env yields no variables at all.
    assert!(outcome.stdout.is_empty(), "stdout: {:?}", outcome.stdout);
}

#[test]
fn explicit_environment_is_passed() {
    let mut spec = CommandSpec::new("/usr/bin/env", Duration::from_secs(5));
    let mut env = BTreeMap::new();
    env.insert("HOSTNUR_TEST".to_string(), "1".to_string());
    spec.env = env;

    let outcome = run(&spec).expect("run env");
    assert!(String::from_utf8_lossy(&outcome.stdout).contains("HOSTNUR_TEST=1"));
}

#[test]
fn timeout_kills_the_process_group() {
    let mut spec = CommandSpec::new("/bin/sh", Duration::from_millis(200));
    spec.args = vec!["-c".to_string(), "sleep 30".to_string()];

    let started = Instant::now();
    let outcome = run(&spec).expect("run sleep");
    assert!(outcome.timed_out, "{outcome:?}");
    assert!(started.elapsed() < Duration::from_secs(5));

    // The process-group leader must actually be gone after SIGKILL.
    assert!(
        kill(Pid::from_raw(outcome.pid as i32), None).is_err(),
        "process {} should be dead",
        outcome.pid
    );
}

#[test]
fn output_is_capped_for_flooding_processes() {
    let mut spec = CommandSpec::new("/usr/bin/head", Duration::from_secs(5));
    spec.args = vec![
        "-c".to_string(),
        "65536".to_string(),
        "/dev/zero".to_string(),
    ];
    spec.output_cap_bytes = 1024;

    let outcome = run(&spec).expect("run head");
    assert_eq!(outcome.stdout.len(), 1024);
    assert!(outcome.stdout_truncated);
}

#[test]
fn working_directory_is_honored() {
    let dir = TempDir::new().unwrap();
    let mut spec = CommandSpec::new("/usr/bin/pwd", Duration::from_secs(5));
    spec.working_dir = Some(dir.path().to_path_buf());

    let outcome = run(&spec).expect("run pwd");
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    // The temp dir may resolve through a symlink (e.g. /tmp on macOS); compare
    // the resolved path instead of the raw string.
    let expected = std::fs::canonicalize(dir.path()).unwrap();
    let actual = std::fs::canonicalize(stdout.trim()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires root to switch process identity"]
fn runs_as_target_identity_without_extra_groups() {
    let user = nix::unistd::User::from_name("nobody")
        .ok()
        .flatten()
        .expect("`nobody` account is required for this test");

    let mut spec = CommandSpec::new("/usr/bin/id", Duration::from_secs(5));
    spec.uid = Some(user.uid.as_raw());
    spec.gid = Some(user.gid.as_raw());

    let outcome = run(&spec).expect("run id");
    assert!(outcome.succeeded(), "{outcome:?}");
    let stdout = String::from_utf8_lossy(&outcome.stdout);

    assert!(
        stdout.contains(&format!("uid={}(", user.uid.as_raw())),
        "expected target uid in: {stdout}"
    );
    assert!(
        stdout.contains(&format!("gid={}(", user.gid.as_raw())),
        "expected target gid in: {stdout}"
    );
    // Supplementary groups must be empty: the group list is exactly the target gid.
    let groups = stdout.split("groups=").nth(1).expect("groups field");
    let group_ids: Vec<&str> = groups.trim().split(',').collect();
    assert_eq!(
        group_ids.len(),
        1,
        "unexpected supplementary groups: {stdout}"
    );
    assert!(
        group_ids[0].starts_with(&format!("{}(", user.gid.as_raw())),
        "unexpected group: {stdout}"
    );
}

#[test]
fn policy_rejects_unlisted_programs() {
    let policy = ProcessPolicy::new(["/usr/bin/pwd"]);
    let shell = CommandSpec::new("/bin/sh", Duration::from_secs(1));
    assert!(policy.validate(&shell).is_err());

    let listed = CommandSpec::new("/usr/bin/pwd", Duration::from_secs(1));
    assert!(policy.validate(&listed).is_ok());
}
