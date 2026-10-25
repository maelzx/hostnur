//! Real TLS lifecycle: Let's Encrypt HTTP-01 via certbot, certificate
//! validation/installation and managed TLS Nginx routing (phase 16).

pub mod certbot;
pub mod certificate;

pub use certbot::{CertbotClient, CertbotOutcome, CertbotRequest, live_paths};
pub use certificate::{CertInfo, CertificateManager, CertificatePaths, from_live_dir};

use std::path::{Path, PathBuf};

/// TLS lifecycle failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsError {
    Invalid(String),
    Tool(String),
    Certificate(String),
    Io(String),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::Invalid(message) => write!(f, "invalid tls request: {message}"),
            TlsError::Tool(message) => write!(f, "tls tool failed: {message}"),
            TlsError::Certificate(message) => write!(f, "certificate rejected: {message}"),
            TlsError::Io(message) => write!(f, "io: {message}"),
        }
    }
}

impl std::error::Error for TlsError {}

/// Where managed sites' installed certificates live (`<dir>/<site>/`).
pub const TLS_DIR: &str = "/etc/hostnur/tls";

/// Base of the per-site ACME HTTP-01 webroots (`<base>/<site>`).
pub const ACME_WEBROOT: &str = "/var/www/hostnur-acme";

/// The certificate installed for a managed site, when both the chain and the
/// key are present. A redeploy uses it to keep serving HTTPS instead of
/// re-rendering the site as plain HTTP.
pub fn installed_site_certificate(tls_dir: &Path, site: &str) -> Option<CertificatePaths> {
    let paths = from_live_dir(&tls_dir.join(site));
    (paths.fullchain.is_file() && paths.key.is_file()).then_some(paths)
}

/// Per-site HTTP-01 webroot beneath a managed base directory.
pub fn webroot_for(base: &Path, name: &str) -> PathBuf {
    base.join(name).join(".well-known").join("acme-challenge")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_site_certificate_needs_both_chain_and_key() {
        let dir = tempfile::tempdir().unwrap();
        assert!(installed_site_certificate(dir.path(), "hn-app").is_none());

        std::fs::create_dir_all(dir.path().join("hn-app")).unwrap();
        std::fs::write(dir.path().join("hn-app/fullchain.pem"), "chain").unwrap();
        assert!(installed_site_certificate(dir.path(), "hn-app").is_none());

        std::fs::write(dir.path().join("hn-app/privkey.pem"), "key").unwrap();
        let paths = installed_site_certificate(dir.path(), "hn-app").unwrap();
        assert_eq!(paths.fullchain, dir.path().join("hn-app/fullchain.pem"));
        assert_eq!(paths.key, dir.path().join("hn-app/privkey.pem"));
    }
}
