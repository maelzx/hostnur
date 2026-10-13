//! Let's Encrypt HTTP-01 issuance via a fixed, allowlisted `certbot` (phase 16.1).
//!
//! Domains are validated before argument construction; staging is used first to
//! avoid production rate limits. Renewal is delegated to certbot's own timer, but
//! can also be triggered explicitly.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, ProcessOutcome, run};
use crate::provider::validate_hostname;

use super::TlsError;

/// An issuance request.
#[derive(Debug, Clone)]
pub struct CertbotRequest {
    pub domains: Vec<String>,
    pub email: String,
    /// HTTP-01 webroot (absolute) served by the managed Nginx route.
    pub webroot: PathBuf,
    pub staging: bool,
}

/// Result of a successful issuance.
#[derive(Debug, Clone)]
pub struct CertbotOutcome {
    /// `/etc/letsencrypt/live/<primary-domain>`.
    pub live_dir: PathBuf,
    pub primary_domain: String,
}

/// Fixed certbot driver.
#[derive(Debug, Clone)]
pub struct CertbotClient {
    pub binary: PathBuf,
    pub deadline: Duration,
}

impl Default for CertbotClient {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("/usr/bin/certbot"),
            deadline: Duration::from_secs(600),
        }
    }
}

impl CertbotClient {
    /// Obtain (or keep) a certificate for the given domains.
    pub fn obtain(&self, request: &CertbotRequest) -> Result<CertbotOutcome, TlsError> {
        if request.domains.is_empty() {
            return Err(TlsError::Invalid(
                "at least one domain is required".to_string(),
            ));
        }
        for domain in &request.domains {
            validate_hostname(domain).map_err(|error| TlsError::Invalid(error.to_string()))?;
        }
        if !request.webroot.is_absolute() {
            return Err(TlsError::Invalid("webroot must be absolute".to_string()));
        }
        if !is_valid_email(&request.email) {
            return Err(TlsError::Invalid("invalid ACME contact email".to_string()));
        }

        let mut args = vec![
            "certonly".to_string(),
            "--webroot".to_string(),
            "-w".to_string(),
            request.webroot.display().to_string(),
        ];
        for domain in &request.domains {
            args.push("-d".to_string());
            args.push(domain.clone());
        }
        args.push("--email".to_string());
        args.push(request.email.clone());
        args.push("--agree-tos".to_string());
        args.push("--non-interactive".to_string());
        args.push("--keep-until-expiring".to_string());
        if request.staging {
            args.push("--staging".to_string());
        }

        self.run(&args)?;

        let primary = request.domains[0].clone();
        Ok(CertbotOutcome {
            live_dir: PathBuf::from("/etc/letsencrypt/live").join(&primary),
            primary_domain: primary,
        })
    }

    /// Renew all managed certificates (`certbot renew`).
    pub fn renew(&self) -> Result<(), TlsError> {
        self.run(&["renew".to_string(), "--quiet".to_string()])?;
        Ok(())
    }

    fn run(&self, args: &[String]) -> Result<ProcessOutcome, TlsError> {
        let mut spec = CommandSpec::new(&self.binary, self.deadline);
        spec.args = args.to_vec();
        spec.output_cap_bytes = 262_144;
        let outcome = run(&spec).map_err(|error| TlsError::Tool(error.to_string()))?;
        if outcome.succeeded() {
            Ok(outcome)
        } else {
            Err(TlsError::Tool(format!(
                "certbot exit code {:?}",
                outcome.exit_code
            )))
        }
    }
}

fn is_valid_email(email: &str) -> bool {
    !email.is_empty()
        && email.len() <= 320
        && email.contains('@')
        && !email.starts_with('@')
        && !email.ends_with('@')
        && !email.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Standard live paths for a domain's lineage.
pub fn live_paths(live_dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        live_dir.join("fullchain.pem"),
        live_dir.join("privkey.pem"),
        live_dir.join("cert.pem"),
        live_dir.join("chain.pem"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_requests() {
        let client = CertbotClient::default();
        let base = CertbotRequest {
            domains: vec!["app.example.com".to_string()],
            email: "ops@example.com".to_string(),
            webroot: PathBuf::from("/var/www/acme"),
            staging: true,
        };

        let mut bad = base.clone();
        bad.domains = vec!["bad host".to_string()];
        assert!(matches!(client.obtain(&bad), Err(TlsError::Invalid(_))));

        let mut bad = base.clone();
        bad.email = "not-an-email".to_string();
        assert!(matches!(client.obtain(&bad), Err(TlsError::Invalid(_))));
    }
}
