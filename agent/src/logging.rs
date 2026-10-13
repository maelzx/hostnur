//! Bounded, structured and redaction-conscious logging.
//!
//! Events are emitted as single-line JSON records on standard error. The record
//! fields are a fixed allowlist: no configuration values, environment contents,
//! credentials or request bodies are ever serialized. Callers pass only the
//! already-safe values (an event name, a short message, a stable code and an
//! optional correlation id).

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Log severity. Ordered from most to least severe for threshold filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

impl Level {
    /// Parse a lowercase level name. Returns `None` for unknown names.
    pub fn parse(value: &str) -> Option<Level> {
        match value {
            "error" => Some(Level::Error),
            "warn" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

static THRESHOLD: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// Set the global minimum level. Called once at startup with the configured
/// level. Defaults to `info` before initialization.
pub fn init(level: Level) {
    THRESHOLD.store(level as u8, Ordering::Relaxed);
}

/// Whether a message at `level` passes the current threshold.
pub fn enabled(level: Level) -> bool {
    (level as u8) <= THRESHOLD.load(Ordering::Relaxed)
}

/// Emit one structured event to standard error if it passes the threshold.
pub fn log(
    level: Level,
    component: &str,
    event: &str,
    message: &str,
    code: Option<&str>,
    correlation_id: Option<&str>,
) {
    if !enabled(level) {
        return;
    }
    eprintln!(
        "{}",
        render_event(
            now_unix_millis(),
            level,
            component,
            event,
            message,
            code,
            correlation_id
        )
    );
}

/// Render a structured event as a single-line JSON object.
///
/// This is a pure function so output shape and escaping can be tested without
/// capturing standard error. Message/event fields are escaped; message content
/// is never interpreted as JSON.
#[allow(clippy::too_many_arguments)]
pub fn render_event(
    timestamp_millis: u128,
    level: Level,
    component: &str,
    event: &str,
    message: &str,
    code: Option<&str>,
    correlation_id: Option<&str>,
) -> String {
    let mut out = String::with_capacity(128);
    out.push('{');
    out.push_str("\"ts_ms\":");
    out.push_str(&timestamp_millis.to_string());
    out.push_str(",\"level\":");
    push_json_string(&mut out, level.as_str());
    out.push_str(",\"component\":");
    push_json_string(&mut out, component);
    out.push_str(",\"event\":");
    push_json_string(&mut out, event);
    out.push_str(",\"message\":");
    push_json_string(&mut out, message);
    if let Some(code) = code {
        out.push_str(",\"code\":");
        push_json_string(&mut out, code);
    }
    if let Some(correlation_id) = correlation_id {
        out.push_str(",\"correlation_id\":");
        push_json_string(&mut out, correlation_id);
    }
    out.push('}');
    out
}

fn now_unix_millis() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis(),
        Err(_) => 0,
    }
}

/// Append a JSON string literal with correct escaping. Control characters are
/// escaped so a message can never break the one-record-per-line contract.
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_parse_roundtrip() {
        for level in [Level::Error, Level::Warn, Level::Info, Level::Debug] {
            assert_eq!(Level::parse(level.as_str()), Some(level));
        }
        assert_eq!(Level::parse("trace"), None);
        assert_eq!(Level::parse("INFO"), None);
    }

    #[test]
    fn render_event_is_single_line_json() {
        let rendered = render_event(
            42,
            Level::Info,
            "startup",
            "agent.starting",
            "agent starting",
            None,
            Some("corr-1"),
        );
        assert!(!rendered.contains('\n'));
        assert!(rendered.starts_with('{') && rendered.ends_with('}'));
        assert!(rendered.contains("\"ts_ms\":42"));
        assert!(rendered.contains("\"level\":\"info\""));
        assert!(rendered.contains("\"correlation_id\":\"corr-1\""));
    }

    #[test]
    fn render_event_escapes_hostile_message() {
        let rendered = render_event(
            1,
            Level::Error,
            "startup",
            "config.invalid",
            "line1\nline2 \"quoted\" \\slash\\",
            Some("io"),
            None,
        );
        assert!(!rendered.contains('\n'));
        assert!(rendered.contains("line1\\nline2 \\\"quoted\\\" \\\\slash\\\\"));
    }

    #[test]
    fn threshold_filters_levels() {
        init(Level::Warn);
        assert!(enabled(Level::Error));
        assert!(enabled(Level::Warn));
        assert!(!enabled(Level::Info));
        assert!(!enabled(Level::Debug));
        init(Level::Info);
    }
}
