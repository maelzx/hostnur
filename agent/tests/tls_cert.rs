//! Phase 16.3/16.5 certificate validation, install and expiry (unprivileged).

use std::path::Path;
use std::process::Command;

use hostnur_agent::tls::CertificateManager;
use tempfile::TempDir;

fn generate_self_signed(
    dir: &Path,
    host: &str,
    days: u32,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let key = dir.join("key.pem");
    let cert = dir.join("cert.pem");
    let status = Command::new("/usr/bin/openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            key.to_str().unwrap(),
            "-out",
            cert.to_str().unwrap(),
            "-days",
            &days.to_string(),
            "-subj",
            &format!("/CN={host}"),
            "-addext",
            &format!("subjectAltName=DNS:{host}"),
        ])
        .status()
        .expect("openssl available");
    assert!(status.success());
    (cert, key)
}

#[test]
fn validates_and_installs_supplied_certificate() {
    let dir = TempDir::new().unwrap();
    let (cert, key) = generate_self_signed(dir.path(), "test.example.com", 30);
    let manager = CertificateManager::default();

    let info = manager
        .validate(&cert, &key, "test.example.com")
        .expect("valid cert");
    assert!(info.subject.contains("test.example.com"));

    // Wrong hostname is rejected.
    assert!(manager.validate(&cert, &key, "other.example.com").is_err());

    // Install with bounded modes.
    let dest = dir.path().join("installed");
    manager
        .install_supplied(&cert, &key, "test.example.com", &dest)
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(dest.join("privkey.pem"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(dest.join("fullchain.pem"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn expiry_is_reported() {
    let dir = TempDir::new().unwrap();
    let (cert, _key) = generate_self_signed(dir.path(), "test.example.com", 30);
    let manager = CertificateManager::default();

    assert!(!manager.expires_within(&cert, 1).unwrap());
    assert!(manager.expires_within(&cert, 60).unwrap());
}
