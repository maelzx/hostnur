//! Root-owned systemd worker and scheduler templates (phase 14).
//!
//! Units run as the application's Linux identity with an approved working
//! directory and restart policy. The `ExecStart` line is a program plus a
//! validated argv (never a root shell string). Scheduler runs are app-scoped,
//! single-instance (a `oneshot` service) and timer-driven.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::CommandSpec;
use crate::process::run as run_process;
use crate::provision::policy;

/// A managed queue worker/daemon.
#[derive(Debug, Clone)]
pub struct WorkerUnit {
    /// Safe slug (derived from app UUID + role).
    pub name: String,
    pub user: String,
    pub group: String,
    /// Absolute working directory under the app layout.
    pub working_dir: PathBuf,
    /// Absolute program (e.g. the PHP CLI).
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Directories the worker may write (e.g. shared storage).
    pub read_write_paths: Vec<PathBuf>,
}

/// A managed scheduler (timer + oneshot service).
#[derive(Debug, Clone)]
pub struct ScheduleUnit {
    pub name: String,
    pub user: String,
    pub group: String,
    pub working_dir: PathBuf,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// `OnCalendar` expression (validated by systemd on load).
    pub on_calendar: String,
}

/// systemd unit failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerError {
    InvalidUnit(String),
    VerifyFailed(String),
    ToolFailed(String),
    Io(String),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerError::InvalidUnit(message) => write!(f, "invalid unit: {message}"),
            WorkerError::VerifyFailed(message) => write!(f, "systemd verify failed: {message}"),
            WorkerError::ToolFailed(message) => write!(f, "systemd tool failed: {message}"),
            WorkerError::Io(message) => write!(f, "io: {message}"),
        }
    }
}

impl std::error::Error for WorkerError {}

/// Render a worker service unit.
pub fn render_worker(unit: &WorkerUnit) -> Result<String, WorkerError> {
    validate_common(
        &unit.name,
        &unit.user,
        &unit.group,
        &unit.working_dir,
        &unit.program,
    )?;
    let mut text = String::new();
    text.push_str("# Managed by Hostnur - do not edit.\n[Unit]\n");
    text.push_str(&format!("Description=Hostnur worker {}\n", unit.name));
    text.push_str("After=network.target\n\n[Service]\n");
    text.push_str("Type=simple\n");
    text.push_str(&format!("User={}\nGroup={}\n", unit.user, unit.group));
    text.push_str(&format!(
        "WorkingDirectory={}\n",
        unit.working_dir.display()
    ));
    text.push_str(&format!(
        "ExecStart={}\n",
        exec_line(&unit.program, &unit.args)?
    ));
    text.push_str("Restart=always\nRestartSec=5\n");
    text.push_str("NoNewPrivileges=true\n");
    text.push_str("ProtectSystem=strict\n");
    text.push_str("ProtectHome=true\n");
    text.push_str("PrivateTmp=true\n");
    if !unit.read_write_paths.is_empty() {
        let paths: Vec<String> = unit
            .read_write_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        text.push_str(&format!("ReadWritePaths={}\n", paths.join(" ")));
    }
    text.push_str("\n[Install]\nWantedBy=multi-user.target\n");
    Ok(text)
}

/// Render a scheduler oneshot service unit.
pub fn render_schedule_service(unit: &ScheduleUnit) -> Result<String, WorkerError> {
    validate_common(
        &unit.name,
        &unit.user,
        &unit.group,
        &unit.working_dir,
        &unit.program,
    )?;
    if unit.on_calendar.trim().is_empty() || unit.on_calendar.contains('\n') {
        return Err(WorkerError::InvalidUnit("invalid OnCalendar".to_string()));
    }
    let mut text = String::new();
    text.push_str("# Managed by Hostnur - do not edit.\n[Unit]\n");
    text.push_str(&format!("Description=Hostnur schedule {}\n", unit.name));
    text.push_str("After=network.target\n\n[Service]\n");
    text.push_str("Type=oneshot\n");
    text.push_str(&format!("User={}\nGroup={}\n", unit.user, unit.group));
    text.push_str(&format!(
        "WorkingDirectory={}\n",
        unit.working_dir.display()
    ));
    text.push_str(&format!(
        "ExecStart={}\n",
        exec_line(&unit.program, &unit.args)?
    ));
    text.push_str(
        "NoNewPrivileges=true\nProtectSystem=strict\nProtectHome=true\nPrivateTmp=true\n",
    );
    Ok(text)
}

/// Render the paired timer unit (single-instance via the oneshot service).
pub fn render_schedule_timer(unit: &ScheduleUnit) -> Result<String, WorkerError> {
    if unit.on_calendar.trim().is_empty() || unit.on_calendar.contains('\n') {
        return Err(WorkerError::InvalidUnit("invalid OnCalendar".to_string()));
    }
    let mut text = String::new();
    text.push_str("# Managed by Hostnur - do not edit.\n[Unit]\n");
    text.push_str(&format!(
        "Description=Hostnur schedule timer {}\n",
        unit.name
    ));
    text.push_str("\n[Timer]\n");
    text.push_str(&format!("OnCalendar={}\n", unit.on_calendar));
    text.push_str("AccuracySec=1s\nPersistent=true\n");
    text.push_str("\n[Install]\nWantedBy=timers.target\n");
    Ok(text)
}

fn validate_common(
    name: &str,
    user: &str,
    group: &str,
    working_dir: &Path,
    program: &Path,
) -> Result<(), WorkerError> {
    if !is_safe_name(name) {
        return Err(WorkerError::InvalidUnit("unsafe unit name".to_string()));
    }
    if !policy::is_valid_account_name(user) || !policy::is_valid_account_name(group) {
        return Err(WorkerError::InvalidUnit(
            "user/group must be managed accounts".to_string(),
        ));
    }
    if !working_dir.is_absolute() || working_dir == Path::new("/") {
        return Err(WorkerError::InvalidUnit(
            "working directory must be absolute".to_string(),
        ));
    }
    if !program.is_absolute() {
        return Err(WorkerError::InvalidUnit(
            "program must be absolute".to_string(),
        ));
    }
    Ok(())
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Build a systemd `ExecStart` value; args are quoted, never shell-expanded.
fn exec_line(program: &Path, args: &[String]) -> Result<String, WorkerError> {
    let mut parts = vec![program.display().to_string()];
    for arg in args {
        if arg.contains('\n') || arg.contains('\r') || arg.contains('\0') {
            return Err(WorkerError::InvalidUnit(
                "argument contains a control character".to_string(),
            ));
        }
        parts.push(systemd_quote(arg));
    }
    Ok(parts.join(" "))
}

fn systemd_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:=@".contains(c))
    {
        return arg.to_string();
    }
    let escaped = arg.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Manages unit files in the root-owned systemd directory.
#[derive(Debug, Clone)]
pub struct WorkerManager {
    pub systemctl: PathBuf,
    pub systemd_analyze: PathBuf,
    pub unit_dir: PathBuf,
    pub deadline: Duration,
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self {
            systemctl: PathBuf::from("/usr/bin/systemctl"),
            systemd_analyze: PathBuf::from("/usr/bin/systemd-analyze"),
            unit_dir: PathBuf::from("/etc/systemd/system"),
            deadline: Duration::from_secs(60),
        }
    }
}

impl WorkerManager {
    fn unit_path(&self, name: &str, suffix: &str) -> Result<PathBuf, WorkerError> {
        if !is_safe_name(name) {
            return Err(WorkerError::InvalidUnit("unsafe unit name".to_string()));
        }
        Ok(self.unit_dir.join(format!("hostnur-{name}{suffix}")))
    }

    /// Write, verify, enable and start a worker unit.
    pub fn apply_worker(&self, unit: &WorkerUnit) -> Result<String, WorkerError> {
        let content = render_worker(unit)?;
        let path = self.unit_path(&unit.name, ".service")?;
        std::fs::write(&path, content).map_err(io)?;
        self.verify(&path)?;
        self.systemctl(&["daemon-reload"])?;
        let unit_name = format!("hostnur-{}.service", unit.name);
        self.systemctl(&["enable", "--now", &unit_name])?;
        Ok(unit_name)
    }

    /// Write, verify and enable a scheduler service + timer.
    pub fn apply_schedule(&self, unit: &ScheduleUnit) -> Result<String, WorkerError> {
        let service = render_schedule_service(unit)?;
        let timer = render_schedule_timer(unit)?;
        let service_path = self.unit_path(&unit.name, ".service")?;
        let timer_path = self.unit_path(&unit.name, ".timer")?;
        std::fs::write(&service_path, service).map_err(io)?;
        std::fs::write(&timer_path, timer).map_err(io)?;
        self.verify(&service_path)?;
        self.verify(&timer_path)?;
        self.systemctl(&["daemon-reload"])?;
        self.systemctl(&["enable", "--now", &format!("hostnur-{}.timer", unit.name)])?;
        Ok(format!("hostnur-{}.timer", unit.name))
    }

    /// Stop and remove a managed unit (service and/or timer).
    pub fn remove(&self, name: &str) -> Result<(), WorkerError> {
        for suffix in [".timer", ".service"] {
            let path = self.unit_path(name, suffix)?;
            if path.exists() {
                let file = path.file_name().unwrap().to_string_lossy().into_owned();
                let _ = self.systemctl(&["disable", "--now", &file]);
                std::fs::remove_file(&path).map_err(io)?;
            }
        }
        self.systemctl(&["daemon-reload"])?;
        Ok(())
    }

    fn verify(&self, path: &Path) -> Result<(), WorkerError> {
        let mut spec = CommandSpec::new(&self.systemd_analyze, self.deadline);
        spec.args = vec!["verify".to_string(), path.display().to_string()];
        spec.output_cap_bytes = 32_768;
        let outcome =
            run_process(&spec).map_err(|error| WorkerError::ToolFailed(error.to_string()))?;
        if outcome.succeeded() {
            Ok(())
        } else {
            Err(WorkerError::VerifyFailed(path.display().to_string()))
        }
    }

    fn systemctl(&self, args: &[&str]) -> Result<(), WorkerError> {
        let mut spec = CommandSpec::new(&self.systemctl, self.deadline);
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.output_cap_bytes = 32_768;
        let outcome =
            run_process(&spec).map_err(|error| WorkerError::ToolFailed(error.to_string()))?;
        if outcome.succeeded() {
            Ok(())
        } else {
            Err(WorkerError::ToolFailed(format!(
                "systemctl {args:?} failed"
            )))
        }
    }
}

fn io(error: std::io::Error) -> WorkerError {
    WorkerError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker() -> WorkerUnit {
        WorkerUnit {
            name: "hn-224065ac4bc7-queue".to_string(),
            user: "hn_224065ac4bc7".to_string(),
            group: "hn_224065ac4bc7".to_string(),
            working_dir: PathBuf::from("/opt/hostnur/apps/x/current"),
            program: PathBuf::from("/usr/bin/php"),
            args: vec!["artisan".to_string(), "queue:work".to_string()],
            read_write_paths: vec![PathBuf::from("/opt/hostnur/apps/x/shared")],
        }
    }

    #[test]
    fn renders_worker_unit() {
        let text = render_worker(&worker()).unwrap();
        assert!(text.contains("User=hn_224065ac4bc7"));
        assert!(text.contains("ExecStart=/usr/bin/php artisan queue:work"));
        assert!(text.contains("Restart=always"));
        assert!(text.contains("ReadWritePaths=/opt/hostnur/apps/x/shared"));
    }

    #[test]
    fn quotes_unsafe_arguments_without_shell() {
        let mut w = worker();
        w.args = vec!["-r".to_string(), "echo 1; rm -rf /".to_string()];
        let text = render_worker(&w).unwrap();
        assert!(text.contains("ExecStart=/usr/bin/php -r \"echo 1; rm -rf /\""));
    }

    #[test]
    fn rejects_unsafe_units() {
        let mut w = worker();
        w.user = "root".to_string();
        assert!(render_worker(&w).is_err());

        let mut w = worker();
        w.name = "../x".to_string();
        assert!(render_worker(&w).is_err());

        let mut w = worker();
        w.program = PathBuf::from("php");
        assert!(render_worker(&w).is_err());
    }

    #[test]
    fn renders_schedule_units() {
        let s = ScheduleUnit {
            name: "hn-224065ac4bc7-sched".to_string(),
            user: "hn_224065ac4bc7".to_string(),
            group: "hn_224065ac4bc7".to_string(),
            working_dir: PathBuf::from("/opt/hostnur/apps/x/current"),
            program: PathBuf::from("/usr/bin/php"),
            args: vec!["artisan".to_string(), "schedule:run".to_string()],
            on_calendar: "*-*-* *:*:00".to_string(),
        };
        let service = render_schedule_service(&s).unwrap();
        assert!(service.contains("Type=oneshot"));
        let timer = render_schedule_timer(&s).unwrap();
        assert!(timer.contains("OnCalendar=*-*-* *:*:00"));
    }
}
