//! Minimal HTTP health probe for activation gating (phase 15).
//!
//! Bounded `Host`-targeted GET over a local TCP connection; the probe uses a
//! registered hostname and path, never a caller-supplied URL. No external HTTP
//! client dependency.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Result of a health probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResult {
    pub status: u16,
    pub healthy: bool,
    pub detail: String,
}

/// A bounded health probe.
#[derive(Debug, Clone)]
pub struct HealthClient {
    pub host_header: String,
    pub port: u16,
    pub path: String,
    pub timeout: Duration,
    pub expect_body_contains: Option<String>,
}

impl HealthClient {
    pub fn new(host_header: impl Into<String>) -> Self {
        Self {
            host_header: host_header.into(),
            port: 80,
            path: "/".to_string(),
            timeout: Duration::from_secs(5),
            expect_body_contains: None,
        }
    }

    /// Perform one probe. Never treats a connection error as healthy.
    pub fn probe(&self) -> HealthResult {
        match self.try_probe() {
            Ok((status, body)) => {
                let mut healthy = (200..400).contains(&status);
                if let Some(expected) = &self.expect_body_contains
                    && !body.contains(expected)
                {
                    healthy = false;
                }
                HealthResult {
                    status,
                    healthy,
                    detail: format!("status {status}"),
                }
            }
            Err(error) => HealthResult {
                status: 0,
                healthy: false,
                detail: error,
            },
        }
    }

    fn try_probe(&self) -> Result<(u16, String), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let mut stream = TcpStream::connect(&addr).map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| error.to_string())?;

        let request = format!(
            "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.path, self.host_header
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;

        let mut response = String::new();
        stream
            .take(64 * 1024)
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;

        let status = response
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| "no HTTP status line".to_string())?;

        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default();

        Ok((status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_connection_is_unhealthy() {
        let client = HealthClient {
            host_header: "127.0.0.1".to_string(),
            port: 1, // nothing listens here
            path: "/".to_string(),
            timeout: Duration::from_millis(200),
            expect_body_contains: None,
        };
        let result = client.probe();
        assert!(!result.healthy);
    }
}
