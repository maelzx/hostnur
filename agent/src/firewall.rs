//! Firewall observation and guarded rule management (phase 20.8).
//!
//! Reads `ufw status verbose` into a bounded summary + rule list, and applies
//! bounded rule changes. Ports 80 and 443 are always protected: they cannot be
//! removed through the generic rule API, and 80 may only be closed via the
//! explicit `https_only` path (and only while 443 is open). Arguments are passed
//! to `ufw` as an explicit array — never a shell string.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::contract::{FirewallRule, FirewallSummary};
use crate::process::{CommandSpec, ProcessOutcome, run};

/// Ports the agent refuses to manage generically.
const PROTECTED_PORTS: [i64; 2] = [80, 443];

/// A firewall observer and guarded rule manager.
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

/// Firewall mutation failures, mapped to contract error codes by the dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirewallError {
    /// A protected web port (80/443) was targeted.
    ProtectedPort,
    /// HTTPS-only was requested but 443 is not open.
    HttpsRequired,
    /// A parameter was invalid or unsafe.
    InvalidInput(String),
    /// The `ufw` command failed.
    Tool(String),
}

impl std::fmt::Display for FirewallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProtectedPort => write!(
                f,
                "ports 80 and 443 are protected and cannot be changed directly"
            ),
            Self::HttpsRequired => write!(f, "port 443 must be open before port 80 can be closed"),
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::Tool(message) => write!(f, "{message}"),
        }
    }
}

impl FirewallPolicy {
    /// Snapshot the firewall, or `None` when unsupported/unavailable.
    pub fn snapshot(&self) -> Option<FirewallSummary> {
        let outcome = self
            .run_ufw(&["status".to_string(), "verbose".to_string()])
            .ok()?;
        // `ufw status` needs root; a failure means we cannot observe it.
        if !outcome.succeeded() {
            return None;
        }
        let text = String::from_utf8_lossy(&outcome.stdout);
        parse_ufw_status(&text)
    }

    /// Add an allow/deny rule. Ports 80/443 are refused.
    pub fn add_rule(
        &self,
        port: i64,
        protocol: &str,
        source: Option<&str>,
        action: &str,
    ) -> Result<(), FirewallError> {
        validate_port(port)?;
        validate_protocol(protocol)?;
        guard_protected(port)?;
        if action != "allow" && action != "deny" {
            return Err(FirewallError::InvalidInput(
                "action must be allow or deny".into(),
            ));
        }
        let source = normalize_source(source)?;
        let outcome = self.run_ufw(&rule_args(action, port, protocol, &source))?;
        require_success(&outcome)
    }

    /// Remove a rule matching the given spec. Ports 80/443 are refused.
    pub fn remove_rule(
        &self,
        port: i64,
        protocol: &str,
        source: Option<&str>,
        action: &str,
    ) -> Result<(), FirewallError> {
        validate_port(port)?;
        validate_protocol(protocol)?;
        guard_protected(port)?;
        if action != "allow" && action != "deny" {
            return Err(FirewallError::InvalidInput(
                "action must be allow or deny".into(),
            ));
        }
        let source = normalize_source(source)?;
        let mut args = vec!["delete".to_string()];
        args.extend(rule_args(action, port, protocol, &source));
        let outcome = self.run_ufw(&args)?;
        require_success(&outcome)
    }

    /// Serve HTTPS only: closing 80 requires 443 to be open first.
    pub fn web_https_only(&self, https_only: bool) -> Result<(), FirewallError> {
        if https_only {
            let summary = self.snapshot().ok_or(FirewallError::HttpsRequired)?;
            let has_443 = summary
                .rules
                .iter()
                .any(|rule| rule.port == 443 && rule.action == "allow");
            if !has_443 {
                return Err(FirewallError::HttpsRequired);
            }
            let outcome = self.run_ufw(&[
                "delete".to_string(),
                "allow".to_string(),
                "80/tcp".to_string(),
            ])?;
            require_success(&outcome)?;
        } else {
            let outcome = self.run_ufw(&["allow".to_string(), "80/tcp".to_string()])?;
            require_success(&outcome)?;
        }
        Ok(())
    }

    fn run_ufw(&self, args: &[String]) -> Result<ProcessOutcome, FirewallError> {
        let mut spec = CommandSpec::new(&self.ufw, self.deadline);
        spec.args = args.to_vec();
        spec.output_cap_bytes = 65_536;
        // `ufw` is a script that shells out to iptables/sysctl; it needs PATH.
        let mut env = BTreeMap::new();
        env.insert(
            "PATH".to_string(),
            "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        );
        env.insert("LC_ALL".to_string(), "C".to_string());
        env.insert("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string());
        spec.env = env;
        run(&spec).map_err(|error| FirewallError::Tool(format!("could not run ufw: {error}")))
    }
}

fn validate_port(port: i64) -> Result<(), FirewallError> {
    if !(1..=65535).contains(&port) {
        return Err(FirewallError::InvalidInput(
            "port must be between 1 and 65535".into(),
        ));
    }
    Ok(())
}

fn validate_protocol(protocol: &str) -> Result<(), FirewallError> {
    if protocol != "tcp" && protocol != "udp" {
        return Err(FirewallError::InvalidInput(
            "protocol must be tcp or udp".into(),
        ));
    }
    Ok(())
}

fn guard_protected(port: i64) -> Result<(), FirewallError> {
    if PROTECTED_PORTS.contains(&port) {
        return Err(FirewallError::ProtectedPort);
    }
    Ok(())
}

fn normalize_source(source: Option<&str>) -> Result<String, FirewallError> {
    let value = source.unwrap_or("any").trim();
    if value.eq_ignore_ascii_case("any") {
        return Ok("any".to_string());
    }
    if value.is_empty() || value.len() > 64 {
        return Err(FirewallError::InvalidInput("source is invalid".into()));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '/' | '-' | '_'))
    {
        return Err(FirewallError::InvalidInput(
            "source contains invalid characters".into(),
        ));
    }
    Ok(value.to_string())
}

fn rule_args(action: &str, port: i64, protocol: &str, source: &str) -> Vec<String> {
    if source == "any" {
        vec![action.to_string(), format!("{port}/{protocol}")]
    } else {
        vec![
            action.to_string(),
            "from".to_string(),
            source.to_string(),
            "to".to_string(),
            "any".to_string(),
            "port".to_string(),
            port.to_string(),
            "proto".to_string(),
            protocol.to_string(),
        ]
    }
}

fn require_success(outcome: &ProcessOutcome) -> Result<(), FirewallError> {
    if outcome.succeeded() {
        Ok(())
    } else {
        Err(FirewallError::Tool(tool_message(outcome)))
    }
}

fn tool_message(outcome: &ProcessOutcome) -> String {
    let mut text = String::from_utf8_lossy(&outcome.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&outcome.stderr).trim().to_string();
    if !err.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&err);
    }
    if text.is_empty() {
        text = "ufw command failed".to_string();
    }
    text.chars().take(300).collect()
}

/// Parse `ufw status verbose` output into a summary with its rule list.
pub fn parse_ufw_status(text: &str) -> Option<FirewallSummary> {
    let mut enabled = None;
    let mut incoming_policy = String::new();
    let mut rules: Vec<FirewallRule> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(status) = trimmed.strip_prefix("Status:") {
            enabled = Some(status.trim().eq_ignore_ascii_case("active"));
        } else if let Some(default) = trimmed.strip_prefix("Default:") {
            // e.g. "deny (incoming), allow (outgoing), disabled (routed)"
            if let Some(before) = default.split("(incoming)").next() {
                incoming_policy = before.trim().to_string();
            }
        } else if let Some(rule) = parse_rule_line(trimmed)
            && rules.len() < 100
        {
            rules.push(rule);
        }
    }

    let enabled = enabled?;
    if incoming_policy.is_empty() {
        incoming_policy = "unknown".to_string();
    }

    let rule_count = rules.len() as i64;

    Some(FirewallSummary {
        enabled,
        incoming_policy,
        rule_count,
        rules,
    })
}

fn parse_rule_line(trimmed: &str) -> Option<FirewallRule> {
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() < 3 {
        return None;
    }

    // The "To" column is first (optionally followed by "(v6)").
    let to = tokens[0];
    let mut index = 1;
    if tokens.get(1) == Some(&"(v6)") {
        index = 2;
    }

    // Find the action within the (incoming) rule row.
    let (action_index, action) = tokens
        .iter()
        .enumerate()
        .skip(index)
        .find_map(|(i, token)| match token.to_ascii_lowercase().as_str() {
            "allow" => Some((i, "allow")),
            "deny" => Some((i, "deny")),
            _ => None,
        })?;

    // Only incoming rules are reported.
    if !matches!(tokens.get(action_index + 1), Some(dir) if dir.eq_ignore_ascii_case("in")) {
        return None;
    }

    let (port, protocol) = parse_to_column(to)?;

    let source_tokens = tokens.get(action_index + 2..).unwrap_or(&[]);
    let mut source = source_tokens.join(" ").replace("(v6)", "");
    source = source.trim().to_string();
    if source.is_empty() || source.eq_ignore_ascii_case("anywhere") {
        source = "any".to_string();
    }

    Some(FirewallRule {
        port,
        protocol,
        source,
        action: action.to_string(),
    })
}

fn parse_to_column(to: &str) -> Option<(i64, String)> {
    let (port, protocol) = to.split_once('/')?;
    let port: i64 = port.trim().parse().ok()?;
    let protocol = protocol.trim().to_ascii_lowercase();
    if !(1..=65535).contains(&port) || (protocol != "tcp" && protocol != "udp") {
        return None;
    }
    Some((port, protocol))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVE: &str = "Status: active\nLogging: on (low)\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nNew profiles: skip\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW IN    Anywhere\n80/tcp                     ALLOW IN    Anywhere\n443/tcp                    ALLOW IN    Anywhere\n";

    #[test]
    fn parses_active_status_and_rules() {
        let summary = parse_ufw_status(ACTIVE).unwrap();
        assert!(summary.enabled);
        assert_eq!(summary.incoming_policy, "deny");
        assert_eq!(summary.rule_count, 3);
        assert_eq!(
            summary.rules,
            vec![
                FirewallRule {
                    port: 22,
                    protocol: "tcp".into(),
                    source: "any".into(),
                    action: "allow".into()
                },
                FirewallRule {
                    port: 80,
                    protocol: "tcp".into(),
                    source: "any".into(),
                    action: "allow".into()
                },
                FirewallRule {
                    port: 443,
                    protocol: "tcp".into(),
                    source: "any".into(),
                    action: "allow".into()
                },
            ]
        );
    }

    #[test]
    fn parses_inactive_status() {
        let summary = parse_ufw_status("Status: inactive\n").unwrap();
        assert!(!summary.enabled);
        assert!(summary.rules.is_empty());
    }

    #[test]
    fn unknown_without_status_line() {
        assert!(parse_ufw_status("command not found").is_none());
    }

    #[test]
    fn refuses_protected_ports() {
        let policy = FirewallPolicy::default();
        assert_eq!(
            policy.add_rule(80, "tcp", None, "allow"),
            Err(FirewallError::ProtectedPort)
        );
        assert_eq!(
            policy.remove_rule(443, "tcp", None, "allow"),
            Err(FirewallError::ProtectedPort)
        );
    }

    #[test]
    fn rejects_invalid_inputs_before_running_ufw() {
        let policy = FirewallPolicy::default();
        assert!(matches!(
            policy.add_rule(70000, "tcp", None, "allow"),
            Err(FirewallError::InvalidInput(_))
        ));
        assert!(matches!(
            policy.add_rule(8080, "sctp", None, "allow"),
            Err(FirewallError::InvalidInput(_))
        ));
        assert!(matches!(
            policy.add_rule(8080, "tcp", None, "drop"),
            Err(FirewallError::InvalidInput(_))
        ));
        assert!(matches!(
            policy.add_rule(8080, "tcp", Some("bad; source"), "allow"),
            Err(FirewallError::InvalidInput(_))
        ));
    }

    #[test]
    fn builds_any_source_rule_args() {
        assert_eq!(
            rule_args("allow", 8080, "tcp", "any"),
            vec!["allow", "8080/tcp"]
        );
        assert_eq!(
            rule_args("deny", 5432, "tcp", "10.0.0.0/8"),
            vec![
                "deny",
                "from",
                "10.0.0.0/8",
                "to",
                "any",
                "port",
                "5432",
                "proto",
                "tcp"
            ]
        );
    }
}
