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

/// A certbot-style lineage `live/<host>` holding a fresh self-signed pair.
fn lineage(live: &Path, host: &str) -> std::path::PathBuf {
    let dir = live.join(host);
    std::fs::create_dir_all(&dir).unwrap();
    let (cert, key) = generate_self_signed(&dir, host, 90);
    std::fs::copy(&cert, dir.join("fullchain.pem")).unwrap();
    std::fs::copy(&cert, dir.join("chain.pem")).unwrap();
    std::fs::rename(&key, dir.join("privkey.pem")).unwrap();
    dir
}

#[test]
fn renewal_refreshes_only_the_sites_of_the_lineage() {
    use hostnur_agent::tls::renewal;

    let dir = TempDir::new().unwrap();
    let live = dir.path().join("live");
    let tls = dir.path().join("tls");
    let renewed = lineage(&live, "app.example.com");
    for site in ["hn-issued", "hn-uploaded"] {
        std::fs::create_dir_all(tls.join(site)).unwrap();
        std::fs::write(tls.join(site).join("fullchain.pem"), "old").unwrap();
    }
    renewal::record_lineage(&tls.join("hn-issued"), &renewed).unwrap();

    let mut reloaded = false;
    let sites = renewal::refresh_renewed(
        &renewed,
        &live,
        &tls,
        &CertificateManager::default(),
        || {
            reloaded = true;
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(sites, ["hn-issued"]);
    assert!(reloaded);
    assert_eq!(
        std::fs::read(tls.join("hn-issued/fullchain.pem")).unwrap(),
        std::fs::read(renewed.join("fullchain.pem")).unwrap()
    );
    assert!(tls.join("hn-issued/privkey.pem").is_file());
    assert_eq!(
        std::fs::read_to_string(tls.join("hn-uploaded/fullchain.pem")).unwrap(),
        "old"
    );
}

#[test]
fn renewal_with_invalid_material_touches_no_site() {
    use hostnur_agent::tls::renewal;

    let dir = TempDir::new().unwrap();
    let live = dir.path().join("live");
    let tls = dir.path().join("tls");
    let renewed = lineage(&live, "app.example.com");
    // The renewed key no longer matches the chain.
    let other = lineage(&dir.path().join("other"), "app.example.com");
    std::fs::copy(other.join("privkey.pem"), renewed.join("privkey.pem")).unwrap();
    std::fs::create_dir_all(tls.join("hn-issued")).unwrap();
    std::fs::write(tls.join("hn-issued/fullchain.pem"), "old").unwrap();
    renewal::record_lineage(&tls.join("hn-issued"), &renewed).unwrap();

    let result = renewal::refresh_renewed(
        &renewed,
        &live,
        &tls,
        &CertificateManager::default(),
        || panic!("must not reload"),
    );

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(tls.join("hn-issued/fullchain.pem")).unwrap(),
        "old"
    );
}
