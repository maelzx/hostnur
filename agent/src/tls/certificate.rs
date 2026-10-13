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
        if !san.contains(host) && !subject.contains(host) {
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
}
