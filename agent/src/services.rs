//! Managed service observation and allowlisted lifecycle control (phase 17.1).
//!
//! Only services in an agent-owned allowlist can be controlled, and control
//! resolves to a fixed systemd unit — never a caller-supplied unit name. There is
//! no path to the host init system, SSH, firewall or unrelated services.

use std::path::PathBuf;
use std::time::Duration;

use crate::contract::ServiceObservation;
use crate::process::{CommandSpec, run};

/// A control action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

impl ServiceAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(ServiceAction::Start),
            "stop" => Some(ServiceAction::Stop),
            "restart" => Some(ServiceAction::Restart),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ServiceAction::Start => "start",
            ServiceAction::Stop => "stop",
            ServiceAction::Restart => "restart",
        }
    }
}

/// An allowlisted managed service (id shown to the panel -> systemd unit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedService {
    pub id: String,
    pub unit: String,
}

/// Service failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    UnknownService(String),
    Tool(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::UnknownService(id) => {
                write!(f, "service `{id}` is not managed by the agent")
            }
            ServiceError::Tool(message) => write!(f, "service tool failed: {message}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Fixed allowlist and controller.
#[derive(Debug, Clone)]
pub struct ServicePolicy {
    pub services: Vec<ManagedService>,
    pub systemctl: PathBuf,
    pub deadline: Duration,
}

impl Default for ServicePolicy {
    fn default() -> Self {
        let list = [
            ("nginx", "nginx"),
            ("php8.3-fpm", "php8.3-fpm"),
            ("php8.4-fpm", "php8.4-fpm"),
            ("mariadb", "mariadb"),
            ("redis-server", "redis-server"),
        ];
        Self {
            services: list
                .iter()
                .map(|(id, unit)| ManagedService {
                    id: (*id).to_string(),
                    unit: (*unit).to_string(),
                })
                .collect(),
            systemctl: PathBuf::from("/usr/bin/systemctl"),
            deadline: Duration::from_secs(60),
        }
    }
}

impl ServicePolicy {
    pub fn get(&self, id: &str) -> Option<&ManagedService> {
        self.services.iter().find(|service| service.id == id)
    }

    /// Current status of every allowlisted service (observed, not assumed).
    pub fn observations(&self) -> Vec<ServiceObservation> {
        self.services
            .iter()
            .map(|service| ServiceObservation {
                name: service.id.clone(),
                status: self.status_of_unit(&service.unit),
            })
            .collect()
    }

    /// Control an allowlisted service, then report its observed status.
    pub fn control(
        &self,
        id: &str,
        action: ServiceAction,
    ) -> Result<ServiceObservation, ServiceError> {
        let service = self
            .get(id)
            .ok_or_else(|| ServiceError::UnknownService(id.to_string()))?;

        let mut spec = CommandSpec::new(&self.systemctl, self.deadline);
        spec.args = vec![action.as_str().to_string(), service.unit.clone()];
        spec.output_cap_bytes = 32_768;
        let outcome = run(&spec).map_err(|error| ServiceError::Tool(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(ServiceError::Tool(format!(
                "systemctl {} {} failed",
                action.as_str(),
                service.unit
            )));
        }

        Ok(ServiceObservation {
            name: service.id.clone(),
            status: self.status_of_unit(&service.unit),
        })
    }

    fn status_of_unit(&self, unit: &str) -> String {
        // `is-active` prints the state (active/inactive/failed/unknown) and exits
        // non-zero for anything but active; we only read stdout.
        let mut spec = CommandSpec::new(&self.systemctl, self.deadline);
        spec.args = vec!["is-active".to_string(), unit.to_string()];
        spec.output_cap_bytes = 4096;
        match run(&spec) {
            Ok(outcome) => {
                let raw = String::from_utf8_lossy(&outcome.stdout);
                map_status(raw.trim()).to_string()
            }
            Err(_) => "unknown".to_string(),
        }
    }
}

/// Map `systemctl is-active` output to a stable status string.
pub fn map_status(raw: &str) -> &'static str {
    match raw {
        "active" => "running",
        "activating" => "starting",
        "deactivating" => "stopping",
        "inactive" => "stopped",
        "failed" => "failed",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_allowlisted_services_are_known() {
        let policy = ServicePolicy::default();
        assert!(policy.get("nginx").is_some());
        assert!(policy.get("php8.4-fpm").is_some());
        assert!(policy.get("ssh").is_none());
        assert!(policy.get("../../../bin/sh").is_none());

        assert!(matches!(
            policy.control("ssh", ServiceAction::Restart),
            Err(ServiceError::UnknownService(_))
        ));
    }

    #[test]
    fn maps_systemd_states() {
        assert_eq!(map_status("active"), "running");
        assert_eq!(map_status("inactive"), "stopped");
        assert_eq!(map_status("failed"), "failed");
        assert_eq!(map_status("weird"), "unknown");
    }
}
