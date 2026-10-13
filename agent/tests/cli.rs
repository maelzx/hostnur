//! End-to-end checks of the compiled `hostnur-agent` binary.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_hostnur-agent");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("run hostnur-agent")
}

fn write_config(dir: &TempDir, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, contents).expect("write config");
    path
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const VALID_CONFIG: &str = r#"
version = 1
socket_path = "/run/hostnur/agent.sock"
state_dir = "/var/lib/hostnur/agent"
"#;

#[test]
fn version_flag_reports_protocol() {
    let output = run(&["--version"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("hostnur-agent"));
    assert!(text.contains("protocol 1"));
}

#[test]
fn help_flag_lists_commands() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Usage:"));
    assert!(text.contains("check-config"));
}

#[test]
fn unknown_command_is_rejected() {
    let output = run(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unknown command"));
}

#[test]
fn check_config_accepts_valid_file() {
    let dir = TempDir::new().expect("temp dir");
    let path = write_config(&dir, "agent.toml", VALID_CONFIG);
    let output = run(&["check-config", "--config", path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("configuration valid"));
}

#[test]
fn check_config_rejects_unknown_setting_without_echoing_value() {
    let dir = TempDir::new().expect("temp dir");
    let contents = format!("{}\nsecret_key = \"top-secret\"\n", VALID_CONFIG);
    let path = write_config(&dir, "agent.toml", &contents);
    let output = run(&["check-config", "--config", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let text = stderr(&output);
    assert!(text.contains("unknown setting `secret_key`"));
    assert!(!text.contains("top-secret"));
}

#[test]
fn check_config_reports_missing_file() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("absent.toml");
    let output = run(&["check-config", "--config", missing.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("config_not_found"));
}

#[test]
fn check_config_does_not_create_paths() {
    let dir = TempDir::new().expect("temp dir");
    let path = write_config(&dir, "agent.toml", VALID_CONFIG);
    // A config check must have no filesystem side effects.
    let before: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    let output = run(&["check-config", "--config", path.to_str().unwrap()]);
    assert!(output.status.success());
    let after: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn check_config_rejects_relative_path_before_reading() {
    // A relative socket path is rejected by validation; the config file itself
    // is read from a temp directory.
    let dir = TempDir::new().expect("temp dir");
    let contents =
        "version = 1\nsocket_path = \"relative.sock\"\nstate_dir = \"/var/lib/hostnur/agent\"\n";
    let path = write_config(&dir, "agent.toml", contents);
    assert!(Path::new(path.to_str().unwrap()).exists());
    let output = run(&["check-config", "--config", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("absolute path"));
}
