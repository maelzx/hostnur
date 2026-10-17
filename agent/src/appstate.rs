//! Per-application observation and lifecycle control (phase 20.6).
//!
//! Reads a bounded, light-redacted tail of the application's own log files and
//! its storage usage, and starts/stops the application's own `hn-<app>-*`
//! systemd units (workers and scheduled jobs). No arbitrary path, shell string or
//! host-wide unit is ever touched.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::contract::Uuid;
use crate::process::{CommandSpec, run};

/// Bounded log read.
const MAX_LOG_BYTES: u64 = 131_072;
const MAX_LOG_LINES: usize = 500;
/// Bounded unit count for a single lifecycle action.
const MAX_UNITS: usize = 50;
/// Command deadline for a lifecycle action.
const CONTROL_DEADLINE: Duration = Duration::from_secs(30);

/// Read a bounded, light-redacted tail of the newest `.log` under
/// `<apps_root>/<id>/current/storage/logs`.
pub fn read_logs(apps_root: &Path, application_id: &Uuid) -> Result<(Vec<String>, bool), String> {
    let logs_dir = apps_root
        .join(application_id.as_str())
        .join("current")
        .join("storage")
        .join("logs");

    let entries = std::fs::read_dir(&logs_dir).map_err(|error| error.to_string())?;

    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if newest
            .as_ref()
            .is_none_or(|(current, _)| modified >= *current)
        {
            newest = Some((modified, path));
        }
    }

    let (_, path) = newest.ok_or_else(|| "no application log file found".to_string())?;

    let length = std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len();
    let start = length.saturating_sub(MAX_LOG_BYTES);

    let mut file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(start))
        .map_err(|error| error.to_string())?;
    let mut buffer = Vec::new();
    file.take(MAX_LOG_BYTES)
        .read_to_end(&mut buffer)
        .map_err(|error| error.to_string())?;

    let text = String::from_utf8_lossy(&buffer);
    let mut lines: Vec<String> = text.lines().map(redact).collect();

    let mut truncated = start > 0;

    if lines.len() > MAX_LOG_LINES {
        lines = lines.split_off(lines.len() - MAX_LOG_LINES);
        truncated = true;
    }

    Ok((lines, truncated))
}

/// Storage usage of the application's managed tree, as `(used_bytes, total_bytes)`.
pub fn storage(apps_root: &Path, application_id: &Uuid) -> Option<(u64, u64)> {
    let root = apps_root.join(application_id.as_str());
    let stats = nix::sys::statvfs::statvfs(&root).ok()?;
    let block_size = stats.fragment_size();
    let total = stats.blocks().saturating_mul(block_size);
    if total == 0 {
        return None;
    }
    let available = stats.blocks_available().saturating_mul(block_size);
    Some((total.saturating_sub(available), total))
}

/// Start or stop every application-owned `hn-<fragment>-*` service.
pub fn control_units(app_fragment: &str, action: &str) -> Result<Vec<String>, String> {
    let units = list_units(app_fragment)?;

    if units.len() > MAX_UNITS {
        // Refuse rather than leave some units running after a "stop".
        return Err(format!(
            "too many application units ({}) to control safely",
            units.len()
        ));
    }

    let mut controlled = Vec::new();

    for unit in units {
        run_systemctl(&[action, &unit])?;
        controlled.push(unit);
    }

    Ok(controlled)
}

fn list_units(app_fragment: &str) -> Result<Vec<String>, String> {
    let pattern = format!("hn-{app_fragment}-*");
    let outcome = run_systemctl_raw(&[
        "list-units",
        "--all",
        "--plain",
        "--no-legend",
        "--type=service",
        &pattern,
    ])?;

    // A missing match is not an error (systemctl may exit non-zero with no units).
    let text = String::from_utf8_lossy(&outcome);

    let prefix = format!("hn-{app_fragment}-");
    let mut units = Vec::new();

    for line in text.lines() {
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name.starts_with(&prefix)
            && name.ends_with(".service")
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'))
        {
            units.push(name.to_string());
        }
    }

    Ok(units)
}

fn run_systemctl(args: &[&str]) -> Result<(), String> {
    run_systemctl_raw(args)?;
    Ok(())
}

fn run_systemctl_raw(args: &[&str]) -> Result<Vec<u8>, String> {
    let mut spec = CommandSpec::new(Path::new("/usr/bin/systemctl"), CONTROL_DEADLINE);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.output_cap_bytes = 65_536;
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );
    env.insert("LC_ALL".to_string(), "C".to_string());
    spec.env = env;

    let outcome = run(&spec).map_err(|error| format!("could not run systemctl: {error}"))?;
    Ok(outcome.stdout)
}

/// Mask the value of obvious secret-bearing env assignments in log lines.
fn redact(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    for needle in ["password=", "secret=", "app_key=", "token="] {
        if let Some(position) = lower.find(needle) {
            let start = position + needle.len();
            let end = line[start..]
                .find(char::is_whitespace)
                .map(|offset| start + offset)
                .unwrap_or(line.len());
            // Mask the value; never return the secret-bearing line intact.
            return format!("{}[redacted]{}", &line[..start], &line[end..]);
        }
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_obvious_secret_assignments() {
        assert!(redact("APP_KEY=base64:abc").ends_with("[redacted]"));
        assert_eq!(redact("GET /health 200"), "GET /health 200");
    }

    #[test]
    fn reads_missing_logs_as_error() {
        let dir = tempfile::tempdir().unwrap();
        let id = Uuid::parse("224065ac-4bc7-4ba4-a06d-8c8bca950c32").unwrap();
        assert!(read_logs(dir.path(), &id).is_err());
    }
}
