//! Read-only host observation for `server.snapshot`.
//!
//! Reads a small, fixed set of values from `/proc` and `statvfs`. No writable
//! operation, no shell, no privileged call. Missing values are omitted rather
//! than fabricated, and the observation is bounded.

use serde_json::Number;

use crate::clock;
use crate::contract::{AgentMode, Metric, ServerSnapshot, ServiceObservation};

/// Build a real, read-only server observation.
pub fn server_snapshot() -> ServerSnapshot {
    let mut metrics = Vec::new();

    if let Some(load) = load_average() {
        push_float(&mut metrics, "load_1m", load, None);
    }
    if let Some((total, available)) = memory_bytes() {
        push_u64(&mut metrics, "memory_total_bytes", total, Some("bytes"));
        push_u64(
            &mut metrics,
            "memory_used_bytes",
            total.saturating_sub(available),
            Some("bytes"),
        );
    }
    if let Some(uptime) = uptime_seconds() {
        push_u64(&mut metrics, "uptime_seconds", uptime, Some("seconds"));
    }
    if let Some(used_percent) = root_disk_used_percent() {
        push_float(
            &mut metrics,
            "disk_used_percent",
            used_percent,
            Some("percent"),
        );
    }
    if let Ok(count) = std::thread::available_parallelism() {
        push_u64(&mut metrics, "cpu_count", count.get() as u64, None);
    }
    if let Some(percent) = cpu_percent() {
        push_float(&mut metrics, "cpu_percent", percent, Some("percent"));
    }

    ServerSnapshot {
        protocol_version: 1,
        mode: AgentMode::Real,
        available: true,
        sampled_at: clock::now_rfc3339(),
        metrics,
        services: vec![] as Vec<ServiceObservation>,
        reason: None,
        hostname: hostname(),
        os: pretty_os_name(),
        firewall: None,
    }
}

fn push_u64(metrics: &mut Vec<Metric>, name: &str, value: u64, unit: Option<&str>) {
    if metrics.len() >= 100 {
        return;
    }
    metrics.push(Metric {
        name: name.to_string(),
        value: Number::from(value),
        unit: unit.map(str::to_string),
    });
}

fn push_float(metrics: &mut Vec<Metric>, name: &str, value: f64, unit: Option<&str>) {
    if metrics.len() >= 100 || !value.is_finite() {
        return;
    }
    if let Some(number) = Number::from_f64(value) {
        metrics.push(Metric {
            name: name.to_string(),
            value: number,
            unit: unit.map(str::to_string),
        });
    }
}

fn load_average() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    text.split_whitespace().next()?.parse::<f64>().ok()
}

fn memory_bytes() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = parse_kib(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = parse_kib(rest);
        }
    }
    Some((total?, available.or(total)?))
}

fn parse_kib(value: &str) -> Option<u64> {
    let kib: u64 = value.split_whitespace().next()?.parse().ok()?;
    Some(kib.saturating_mul(1024))
}

fn uptime_seconds() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    let seconds: f64 = text.split_whitespace().next()?.parse().ok()?;
    Some(seconds.max(0.0) as u64)
}

fn root_disk_used_percent() -> Option<f64> {
    let stats = nix::sys::statvfs::statvfs("/").ok()?;
    let block_size = stats.fragment_size();
    let total = stats.blocks().saturating_mul(block_size);
    if total == 0 {
        return None;
    }
    let available = stats.blocks_available().saturating_mul(block_size);
    let used = total.saturating_sub(available);
    Some((used as f64 / total as f64) * 100.0)
}

/// Estimate CPU utilisation over a short interval by sampling `/proc/stat`
/// twice (there is no instantaneous CPU percentage in Linux).
fn cpu_percent() -> Option<f64> {
    let first = read_cpu_times()?;
    std::thread::sleep(std::time::Duration::from_millis(150));
    let second = read_cpu_times()?;

    let total = second.0.checked_sub(first.0)?;
    let idle = second.1.checked_sub(first.1)?;

    if total == 0 {
        return None;
    }

    let busy = total.saturating_sub(idle);
    Some((busy as f64 / total as f64) * 100.0)
}

/// `(total_ticks, idle_ticks)` from the aggregate `cpu` line of `/proc/stat`.
fn read_cpu_times() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let ticks: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse().ok())
        .collect();

    if ticks.len() < 4 {
        return None;
    }

    let total: u64 = ticks.iter().sum();
    // idle + iowait are the non-busy time.
    let idle = ticks[3] + ticks.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

fn hostname() -> Option<String> {
    nix::unistd::gethostname()
        .ok()
        .and_then(|name| name.into_string().ok())
        .filter(|name| !name.is_empty())
}

fn pretty_os_name() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_real_and_bounded() {
        let snapshot = server_snapshot();
        assert_eq!(snapshot.mode, AgentMode::Real);
        assert!(snapshot.available);
        assert!(snapshot.reason.is_none());
        assert!(snapshot.metrics.len() <= 100);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_live_metrics_on_linux() {
        let snapshot = server_snapshot();
        assert!(!snapshot.metrics.is_empty());
        let total = snapshot
            .metrics
            .iter()
            .find(|metric| metric.name == "memory_total_bytes");
        assert!(total.is_some(), "memory_total_bytes should be present");
        assert!(snapshot.hostname.is_some());
    }
}
