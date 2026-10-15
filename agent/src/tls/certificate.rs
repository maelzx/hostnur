//! Certificate validation and root-owned installation (phase 16.3).
//!
//! Validation uses the fixed `openssl` binary: private-key match, not-expired,
//! and hostname coverage. Installed files are root-owned with bounded modes
//! (`0644` chain, `0600` key).

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, run};

use super::TlsError;

/// Standard certificate file paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificatePaths {
    pub fullchain: PathBuf,
    pub key: PathBuf,
    pub cert: PathBuf,
    pub chain: PathBuf,
}

/// Build the standard paths for a `live/<domain>` directory.
pub fn from_live_dir(live_dir: &Path) -> CertificatePaths {
    CertificatePaths {
        fullchain: live_dir.join("fullchain.pem"),
        key: live_dir.join("privkey.pem"),
        cert: live_dir.join("cert.pem"),
        chain: live_dir.join("chain.pem"),
    }
}

/// Validated certificate metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertInfo {
    pub subject: String,
    pub not_after: String,
}

/// Validates and installs certificates using `openssl`.
#[derive(Debug, Clone)]
pub struct CertificateManager {
    pub openssl: PathBuf,
    pub deadline: Duration,
}

impl Default for CertificateManager {
    fn default() -> Self {
        Self {
            openssl: PathBuf::from("/usr/bin/openssl"),
            deadline: Duration::from_secs(30),
        }
    }
}

impl CertificateManager {
    /// Validate a certificate/key pair for hostname coverage and expiry.
    pub fn validate(&self, cert: &Path, key: &Path, host: &str) -> Result<CertInfo, TlsError> {
        let cert_pub = self.capture(&["x509", "-in", &path(cert), "-noout", "-pubkey"])?;
        let key_pub = self.capture(&["pkey", "-in", &path(key), "-pubout"])?;
        if cert_pub.trim() != key_pub.trim() {
            return Err(TlsError::Certificate(
                "private key does not match certificate".to_string(),
            ));
        }

        // Fails (non-zero) when the certificate is expired.
        self.run(&["x509", "-in", &path(cert), "-noout", "-checkend", "0"])
            .map_err(|_| TlsError::Certificate("certificate is expired".to_string()))?;

        let subject = self.capture(&["x509", "-in", &path(cert), "-noout", "-subject"])?;
        let san = self
            .capture(&[
                "x509",
                "-in",
                &path(cert),
                "-noout",
                "-ext",
                "subjectAltName",
            ])
            .unwrap_or_default();
        // Match by name, not substring: `evil-app.example.com` must not cover
        // `app.example.com`, and `*.example.com` must cover `app.example.com`.
        let names = dns_names(&san);
        let covered = if names.is_empty() {
            subject_common_name(&subject).is_some_and(|cn| cn.eq_ignore_ascii_case(host.trim()))
        } else {
            names.iter().any(|name| matches_name(name, host))
        };
        if !covered {
            return Err(TlsError::Certificate(format!(
                "certificate does not cover `{host}`"
            )));
        }
        let not_after = self.capture(&["x509", "-in", &path(cert), "-noout", "-enddate"])?;

        Ok(CertInfo {
            subject: subject.trim().to_string(),
            not_after: not_after.trim().to_string(),
        })
    }

    /// Validate and install an operator-supplied certificate/key pair.
    ///
    /// The supplied material may be a single PEM file with cert (+ optional
    /// chain); it is validated against `host` before any install.
    pub fn install_supplied(
        &self,
        cert: &Path,
        key: &Path,
        host: &str,
        dest_dir: &Path,
    ) -> Result<CertInfo, TlsError> {
        let info = self.validate(cert, key, host)?;
        let source = CertificatePaths {
            fullchain: cert.to_path_buf(),
            key: key.to_path_buf(),
            cert: cert.to_path_buf(),
            chain: cert.to_path_buf(),
        };
        self.install(&source, dest_dir)?;
        Ok(info)
    }

    /// Whether the certificate expires within `days`.
    pub fn expires_within(&self, cert: &Path, days: u32) -> Result<bool, TlsError> {
        let seconds = u64::from(days).saturating_mul(86_400).to_string();
        let mut spec = CommandSpec::new(&self.openssl, self.deadline);
        spec.args = vec![
            "x509".to_string(),
            "-in".to_string(),
            path(cert),
            "-noout".to_string(),
            "-checkend".to_string(),
            seconds,
        ];
        spec.output_cap_bytes = 65_536;
        let outcome = run(&spec).map_err(|error| TlsError::Tool(error.to_string()))?;
        Ok(!outcome.succeeded())
    }

    /// Copy validated material into a root-owned directory with bounded modes.
    pub fn install(
        &self,
        source: &CertificatePaths,
        dest_dir: &Path,
    ) -> Result<CertificatePaths, TlsError> {
        std::fs::create_dir_all(dest_dir).map_err(io)?;
        set_mode(dest_dir, 0o755)?;

        let dest = CertificatePaths {
            fullchain: dest_dir.join("fullchain.pem"),
            key: dest_dir.join("privkey.pem"),
            cert: dest_dir.join("cert.pem"),
            chain: dest_dir.join("chain.pem"),
        };

        for (from, to, mode) in [
            (&source.fullchain, &dest.fullchain, 0o644),
            (&source.cert, &dest.cert, 0o644),
            (&source.chain, &dest.chain, 0o644),
            (&source.key, &dest.key, 0o600),
        ] {
            std::fs::copy(from, to).map_err(io)?;
            set_mode(to, mode)?;
        }
        Ok(dest)
    }

    fn capture(&self, args: &[&str]) -> Result<String, TlsError> {
        let mut spec = CommandSpec::new(&self.openssl, self.deadline);
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.output_cap_bytes = 65_536;
        let outcome = run(&spec).map_err(|error| TlsError::Tool(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(TlsError::Tool(format!("openssl {args:?} failed")));
        }
        Ok(String::from_utf8_lossy(&outcome.stdout).into_owned())
    }

    fn run(&self, args: &[&str]) -> Result<(), TlsError> {
        let mut spec = CommandSpec::new(&self.openssl, self.deadline);
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.output_cap_bytes = 65_536;
        let outcome = run(&spec).map_err(|error| TlsError::Tool(error.to_string()))?;
        if outcome.succeeded() {
            Ok(())
        } else {
            Err(TlsError::Tool(format!("openssl {args:?} failed")))
        }
    }
}

fn path(value: &Path) -> String {
    value.display().to_string()
}

/// Extract the `DNS:` entries from an openssl `subjectAltName` dump.
fn dns_names(extension: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = extension;
    while let Some(position) = rest.find("DNS:") {
        let after = &rest[position + "DNS:".len()..];
        let name: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ',')
            .collect();
        let consumed = name.len();
        if consumed == 0 {
            rest = after;
            continue;
        }
        names.push(name);
        rest = &after[consumed..];
    }
    names
}

/// Whether a certificate name (exact or `*.` wildcard) covers `host`.
/// A wildcard covers exactly one extra leftmost label.
fn matches_name(name: &str, host: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    let host = host.trim().to_ascii_lowercase();
    if let Some(suffix) = name.strip_prefix("*.") {
        return match host.strip_suffix(&format!(".{suffix}")) {
            Some(prefix) => !prefix.is_empty() && !prefix.contains('.') && !prefix.contains('*'),
            None => false,
        };
    }
    name == host
}

/// The CN from an openssl `-subject` line (`CN = x` or legacy `/CN=x`).
fn subject_common_name(subject: &str) -> Option<&str> {
    let position = subject.find("CN")?;
    let tail = subject[position + 2..].trim_start();
    let tail = tail.strip_prefix('=')?;
    let value = tail
        .split(['\r', '\n', ','])
        .next()?
        .split('/')
        .next()?
        .trim();
    (!value.is_empty()).then_some(value)
}

fn set_mode(path: &Path, mode: u32) -> Result<(), TlsError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(io)
}

fn io(error: std::io::Error) -> TlsError {
    TlsError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_live_dir_paths() {
        let paths = from_live_dir(Path::new("/etc/letsencrypt/live/app.example.com"));
        assert_eq!(
            paths.fullchain,
            PathBuf::from("/etc/letsencrypt/live/app.example.com/fullchain.pem")
        );
        assert_eq!(
            paths.key,
            PathBuf::from("/etc/letsencrypt/live/app.example.com/privkey.pem")
        );
    }

    #[test]
    fn san_names_are_matched_exactly_not_as_substrings() {
        let san =
            "X509v3 Subject Alternative Name: \n    DNS:evil-app.example.com, DNS:app.example.net";
        let names = dns_names(san);
        assert_eq!(names, vec!["evil-app.example.com", "app.example.net"]);
        assert!(!matches_name(&names[0], "app.example.com"));
        assert!(matches_name(&names[0], "evil-app.example.com"));
    }

    #[test]
    fn wildcards_cover_one_leftmost_label() {
        assert!(matches_name("*.example.com", "app.example.com"));
        assert!(matches_name("*.example.com", "APP.Example.COM"));
        assert!(!matches_name("*.example.com", "example.com"));
        assert!(!matches_name("*.example.com", "a.b.example.com"));
        assert!(!matches_name("*", "anything"));
    }

    #[test]
    fn subject_common_name_parsing() {
        assert_eq!(
            subject_common_name("subject=CN = app.example.com"),
            Some("app.example.com")
        );
        assert_eq!(
            subject_common_name("subject= /CN=app.example.com/OU=x"),
            Some("app.example.com")
        );
    }
}
