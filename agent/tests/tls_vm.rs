//! Phase 16 root-run TLS test: staging HTTP-01 issuance through the agent
//! primitives, certificate validation/install and managed TLS routing.
//!
//! Run on a publicly reachable host whose domain resolves to it:
//! `sudo target/x86_64-unknown-linux-gnu/debug/deps/tls_vm-<hash> --ignored --nocapture`

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use hostnur_agent::provider::{
    ManagedSite, NginxManager, SiteKind, render_http_challenge_site, render_tls_site,
};
use hostnur_agent::tls::{CertbotClient, CertbotRequest, CertificateManager, from_live_dir};

const DOMAIN: &str = "hostnur-p16.kom.my";
const EMAIL: &str = "maelzx@duck.com";
const WEBROOT: &str = "/var/www/hostnur-acme";
const DOCROOT: &str = "/var/www/hostnur-tls";
const SITE: &str = "hn-p16-tls";

fn require_root() {
    assert!(
        nix::unistd::geteuid().is_root(),
        "run as root on the public host"
    );
}

fn sh(program: &str, args: &[&str]) -> std::process::Output {
    Command::new(program).args(args).output().expect("spawn")
}

fn curl_https() -> String {
    String::from_utf8_lossy(&sh("/usr/bin/curl", &["-sk", &format!("https://{DOMAIN}/")]).stdout)
        .into_owned()
}

#[test]
#[ignore = "requires a publicly reachable host with the domain pointed at it"]
fn issues_validates_installs_and_serves_tls() {
    require_root();

    std::fs::create_dir_all(DOCROOT).unwrap();
    std::fs::write(docroot_path("index.html"), b"hostnur-p16-tls-ok").unwrap();
    std::fs::create_dir_all(WEBROOT).unwrap();

    let manager = NginxManager::default();
    let site = ManagedSite {
        name: SITE.to_string(),
        server_name: DOMAIN.to_string(),
        root: PathBuf::from(DOCROOT),
        fpm_socket: None,
        kind: SiteKind::Static,
    };

    // Pre-issuance: serve the challenge over HTTP.
    manager
        .apply(
            SITE,
            &render_http_challenge_site(&site, std::path::Path::new(WEBROOT)).unwrap(),
        )
        .expect("apply challenge site");

    // Staging issuance through the agent's certbot driver.
    let certbot = CertbotClient::default();
    let outcome = certbot
        .obtain(&CertbotRequest {
            domains: vec![DOMAIN.to_string()],
            email: EMAIL.to_string(),
            webroot: PathBuf::from(WEBROOT),
            staging: true,
        })
        .expect("certbot staging issuance");

    // Validate + install root-owned material.
    let live = from_live_dir(&outcome.live_dir);
    let certs = CertificateManager::default();
    let info = certs
        .validate(&live.fullchain, &live.key, DOMAIN)
        .expect("validate issued certificate");
    assert!(info.not_after.contains("GMT"), "{}", info.not_after);
    let installed = certs
        .install(&live, std::path::Path::new("/etc/hostnur/tls/hn-p16"))
        .expect("install certificate");

    // Activate TLS routing and verify an actual handshake.
    manager
        .apply(
            SITE,
            &render_tls_site(
                &site,
                std::path::Path::new(WEBROOT),
                &installed.fullchain,
                &installed.key,
            )
            .unwrap(),
        )
        .expect("apply tls site");

    let mut served = false;
    for _ in 0..25 {
        if curl_https().contains("hostnur-p16-tls-ok") {
            served = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(served, "TLS site did not serve");

    manager.remove(SITE).unwrap();
}

fn docroot_path(name: &str) -> PathBuf {
    PathBuf::from(DOCROOT).join(name)
}
