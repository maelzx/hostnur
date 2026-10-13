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

/// Per-site HTTP-01 webroot beneath a managed base directory.
pub fn webroot_for(base: &Path, name: &str) -> PathBuf {
    base.join(name).join(".well-known").join("acme-challenge")
}
