//! Read-only firewall observation (phase 17.5).
//!
//! Reports the supported backend and a sampled summary. There is deliberately no
//! mutation path: the agent never changes firewall rules.

use std::path::PathBuf;
use std::time::Duration;

use crate::contract::FirewallSummary;
use crate::process::{CommandSpec, run};

/// A read-only firewall observer.
#[derive(Debug, Clone)]
pub struct FirewallPolicy {
    pub ufw: PathBuf,
    pub deadline: Duration,
}

impl Default for FirewallPolicy {
    fn default() -> Self {
        Self {
            ufw: PathBuf::from("/usr/sbin/ufw"),
            deadline: Duration::from_secs(15),
        }
    }
}

impl FirewallPolicy {
    /// Snapshot the firewall, or `None` when unsupported/unavailable.
    pub fn snapshot(&self) -> Option<FirewallSummary> {
        let mut spec = CommandSpec::new(&self.ufw, self.deadline);
        spec.args = vec!["status".to_string(), "verbose".to_string()];
        spec.output_cap_bytes = 65_536;
        // `ufw` is a script that shells out to iptables/sysctl; it needs PATH.
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            "PATH".to_string(),
            "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        );
        env.insert("LC_ALL".to_string(), "C".to_string());
        spec.env = env;
        let outcome = run(&spec).ok()?;
        // `ufw status` needs root; a failure means we cannot observe it.
        if !outcome.succeeded() {
            return None;
        }
        let text = String::from_utf8_lossy(&outcome.stdout);
        parse_ufw_status(&text)
    }
}

/// Parse `ufw status verbose` output into a summary.
pub fn parse_ufw_status(text: &str) -> Option<FirewallSummary> {
    let mut enabled = None;
    let mut incoming_policy = String::new();
    let mut rule_count: i64 = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(status) = trimmed.strip_prefix("Status:") {
            let status = status.trim();
            enabled = Some(status.eq_ignore_ascii_case("active"));
        } else if let Some(default) = trimmed.strip_prefix("Default:") {
            // e.g. "deny (incoming), allow (outgoing), disabled (routed)"
            if let Some(before) = default.split("(incoming)").next() {
                incoming_policy = before.trim().to_string();
            }
        } else if (trimmed.contains("ALLOW") || trimmed.contains("DENY")) && !trimmed.contains("To")
        {
            rule_count += 1;
        }
    }

    let enabled = enabled?;
    if incoming_policy.is_empty() {
        incoming_policy = "unknown".to_string();
    }

    Some(FirewallSummary {
        enabled,
        incoming_policy,
        rule_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_active_status() {
        let text = "Status: active\nLogging: on (low)\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nNew profiles: skip\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW IN    Anywhere\n80/tcp                     ALLOW IN    Anywhere\n443/tcp                    ALLOW IN    Anywhere\n";
        let summary = parse_ufw_status(text).unwrap();
        assert!(summary.enabled);
        assert_eq!(summary.incoming_policy, "deny");
        assert_eq!(summary.rule_count, 3);
    }

    #[test]
    fn parses_inactive_status() {
        let text = "Status: inactive\n";
        let summary = parse_ufw_status(text).unwrap();
        assert!(!summary.enabled);
    }

    #[test]
    fn unknown_without_status_line() {
        assert!(parse_ufw_status("command not found").is_none());
    }
}
