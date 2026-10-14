//! Cron → systemd `OnCalendar` conversion (phase 17.3).
//!
//! Supports standard 5-field cron (minute hour day-of-month month day-of-week)
//! with `*`, values, ranges, steps (`*/n`, `a-b/n`) and comma lists. Steps are
//! expanded to explicit lists up to a bounded size. When both day-of-month and
//! day-of-week are restricted (cron's OR semantics), two `OnCalendar` lines are
//! emitted. Unsupported or out-of-range input is rejected rather than
//! approximated. The result is additionally validated by `systemd-analyze`
//! before any timer is installed.

/// Maximum expanded values per field (defensive bound).
const MAX_VALUES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronError(String);

impl std::fmt::Display for CronError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid cron expression: {}", self.0)
    }
}

impl std::error::Error for CronError {}

const DOW_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Convert a 5-field cron expression and timezone to systemd `OnCalendar` lines.
pub fn to_on_calendar(expression: &str, timezone: &str) -> Result<Vec<String>, CronError> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(CronError(
            "expected 5 fields (minute hour day month weekday)".to_string(),
        ));
    }
    if !valid_timezone(timezone) {
        return Err(CronError("invalid timezone identifier".to_string()));
    }

    let minute = numeric_token(fields[0], 0, 59, 2)?;
    let hour = numeric_token(fields[1], 0, 23, 2)?;
    let dom = numeric_token(fields[2], 1, 31, 2)?;
    let month = numeric_token(fields[3], 1, 12, 2)?;
    let dow = dow_token(fields[4])?;

    let time = format!("{hour}:{minute}:00");
    let dom_restricted = fields[2] != "*";
    let dow_restricted = fields[4] != "*";

    let mut lines = Vec::new();
    match (dom_restricted, dow_restricted) {
        (false, false) => lines.push(format!("*-{month}-{dom} {time} {timezone}")),
        (false, true) => lines.push(format!("{dow} *-{month}-* {time} {timezone}")),
        (true, false) => lines.push(format!("*-{month}-{dom} {time} {timezone}")),
        (true, true) => {
            // cron OR semantics -> two independent OnCalendar lines.
            lines.push(format!("{dow} *-{month}-* {time} {timezone}"));
            lines.push(format!("*-{month}-{dom} {time} {timezone}"));
        }
    }
    Ok(lines)
}

fn valid_timezone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.contains(char::is_whitespace)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+' | '.'))
}

/// Expand a numeric cron field into an `OnCalendar` token, zero-padded.
fn numeric_token(spec: &str, min: i64, max: i64, pad: usize) -> Result<String, CronError> {
    let values = expand(spec, min, max)?;
    Ok(token_from_values(&values, min, max, pad))
}

/// Day-of-week token using systemd names ("Mon..Fri", "Mon,Wed").
fn dow_token(spec: &str) -> Result<String, CronError> {
    if spec == "*" {
        return Ok("*".to_string());
    }
    let values = expand(spec, 0, 7)?;
    let mut names: Vec<&str> = values
        .iter()
        .map(|value| DOW_NAMES[(*value % 7) as usize])
        .collect();
    names.dedup();
    if names.len() == 7 {
        return Ok("*".to_string());
    }
    if names.len() == 1 {
        return Ok(names[0].to_string());
    }
    Ok(names.join(","))
}

/// Expand a cron field into a sorted, deduplicated list of values.
fn expand(spec: &str, min: i64, max: i64) -> Result<Vec<i64>, CronError> {
    if spec.is_empty() {
        return Err(CronError("empty field".to_string()));
    }
    let mut out: Vec<i64> = Vec::new();
    for part in spec.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step: i64 = step
                    .parse()
                    .map_err(|_| CronError(format!("bad step in `{part}`")))?;
                if step < 1 {
                    return Err(CronError("step must be >= 1".to_string()));
                }
                (range, step)
            }
            None => (part, 1),
        };

        let (start, end) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (
                a.parse()
                    .map_err(|_| CronError(format!("bad range in `{part}`")))?,
                b.parse()
                    .map_err(|_| CronError(format!("bad range in `{part}`")))?,
            )
        } else {
            let value: i64 = range
                .parse()
                .map_err(|_| CronError(format!("bad value `{part}`")))?;
            (value, value)
        };

        if start < min || end > max || start > end {
            return Err(CronError(format!("value out of range in `{part}`")));
        }

        let mut value = start;
        while value <= end {
            out.push(value);
            value += step;
            if out.len() > MAX_VALUES {
                return Err(CronError("field expands to too many values".to_string()));
            }
        }
    }

    out.sort_unstable();
    out.dedup();
    Ok(out)
}

fn token_from_values(values: &[i64], min: i64, max: i64, pad: usize) -> String {
    if values.len() as i64 == max - min + 1 {
        return "*".to_string();
    }
    let format = |value: i64| format!("{value:0pad$}", pad = pad);
    if values.len() == 1 {
        return format(values[0]);
    }
    let contiguous = values.windows(2).all(|pair| pair[1] == pair[0] + 1);
    if contiguous {
        return format!(
            "{}..{}",
            format(values[0]),
            format(values[values.len() - 1])
        );
    }
    values
        .iter()
        .map(|value| format(*value))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_at_time() {
        assert_eq!(
            to_on_calendar("0 8 * * *", "UTC").unwrap(),
            vec!["*-*-* 08:00:00 UTC".to_string()]
        );
    }

    #[test]
    fn every_fifteen_minutes() {
        assert_eq!(
            to_on_calendar("*/15 * * * *", "UTC").unwrap(),
            vec!["*-*-* *:00,15,30,45:00 UTC".to_string()]
        );
    }

    #[test]
    fn weekly_range_uses_names() {
        assert_eq!(
            to_on_calendar("0 0 * * 1-5", "UTC").unwrap(),
            vec!["Mon,Tue,Wed,Thu,Fri *-*-* 00:00:00 UTC".to_string()]
        );
    }

    #[test]
    fn monthly_at_first() {
        assert_eq!(
            to_on_calendar("30 2 1 * *", "UTC").unwrap(),
            vec!["*-*-01 02:30:00 UTC".to_string()]
        );
    }

    #[test]
    fn dom_and_dow_emit_two_lines() {
        let lines = to_on_calendar("0 0 1 * 1", "Asia/Kuala_Lumpur").unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("Mon "));
        assert!(lines[1].starts_with("*-*-01 "));
    }

    #[test]
    fn every_hour_and_minutely() {
        assert_eq!(
            to_on_calendar("0 * * * *", "UTC").unwrap(),
            vec!["*-*-* *:00:00 UTC".to_string()]
        );
        assert_eq!(
            to_on_calendar("* * * * *", "UTC").unwrap(),
            vec!["*-*-* *:*:00 UTC".to_string()]
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(to_on_calendar("60 * * * *", "UTC").is_err());
        assert!(to_on_calendar("* * * *", "UTC").is_err());
        assert!(to_on_calendar("@reboot", "UTC").is_err());
        assert!(to_on_calendar("* * * * *", "not a tz").is_err());
        assert!(to_on_calendar("* * * * 8", "UTC").is_err());
    }
}
