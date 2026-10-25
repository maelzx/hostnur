//! Carry certbot renewals into the managed TLS directory.
//!
//! Issuance copies a certbot lineage (`/etc/letsencrypt/live/<host>`) into
//! `/etc/hostnur/tls/<site>`, which is what Nginx serves. certbot's own timer
//! renews the lineage, so each site records the lineage it was copied from in a
//! `lineage` marker, and a certbot deploy hook (`hostnur-agent renew-hook`)
//! re-validates the renewed material and copies it to every site that names
//! that lineage. A supplied (uploaded) certificate clears the marker, so a
//! later renewal never replaces it.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::provider::validate_hostname;

use super::{CertificateManager, TlsError, from_live_dir};

/// certbot's live lineage directory.
pub const LETSENCRYPT_LIVE: &str = "/etc/letsencrypt/live";

/// Deploy hooks certbot runs after every successful renewal.
pub const DEPLOY_HOOKS_DIR: &str = "/etc/letsencrypt/renewal-hooks/deploy";

/// File name of the managed hook inside [`DEPLOY_HOOKS_DIR`].
pub const HOOK_NAME: &str = "hostnur";

/// Per-site file naming the certbot lineage the certificate was copied from.
pub const LINEAGE_MARKER: &str = "lineage";

fn io(error: std::io::Error) -> TlsError {
    TlsError::Io(error.to_string())
}

/// Record that the certificate in `site_dir` comes from `lineage`.
pub fn record_lineage(site_dir: &Path, lineage: &Path) -> Result<(), TlsError> {
    std::fs::write(
        site_dir.join(LINEAGE_MARKER),
        format!("{}\n", lineage.display()),
    )
    .map_err(io)
}

/// Forget the lineage of `site_dir` (its certificate was supplied directly).
pub fn clear_lineage(site_dir: &Path) -> Result<(), TlsError> {
    match std::fs::remove_file(site_dir.join(LINEAGE_MARKER)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(io(error)),
        _ => Ok(()),
    }
}

fn recorded_lineage(site_dir: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(site_dir.join(LINEAGE_MARKER)).ok()?;
    let line = contents.trim();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

fn site_dirs(tls_dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(tls_dir) else {
        return Vec::new();
    };
    let mut sites: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .collect();
    sites.sort();
    sites
}

/// Sites whose installed certificate was copied from `lineage`.
pub fn sites_for_lineage(tls_dir: &Path, lineage: &Path) -> Vec<String> {
    site_dirs(tls_dir)
        .into_iter()
        .filter(|(_, dir)| recorded_lineage(dir).as_deref() == Some(lineage))
        .map(|(name, _)| name)
        .collect()
}

/// Give a lineage marker to sites installed before markers existed: a site
/// whose installed chain is byte-identical to a live lineage's chain was copied
/// from it. Returns the adopted `(site, lineage)` pairs.
pub fn adopt_lineages(tls_dir: &Path, live_dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(live_dir) else {
        return Vec::new();
    };
    let lineages: Vec<(PathBuf, Vec<u8>)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            let chain = std::fs::read(path.join("fullchain.pem")).ok()?;
            Some((path, chain))
        })
        .collect();

    let mut adopted = Vec::new();
    for (name, dir) in site_dirs(tls_dir) {
        if dir.join(LINEAGE_MARKER).exists() {
            continue;
        }
        let Ok(installed) = std::fs::read(dir.join("fullchain.pem")) else {
            continue;
        };
        if let Some((lineage, _)) = lineages.iter().find(|(_, chain)| *chain == installed)
            && record_lineage(&dir, lineage).is_ok()
        {
            adopted.push((name, lineage.clone()));
        }
    }
    adopted
}

/// The hook certbot runs; it hands the renewal to the agent binary.
pub fn hook_script(agent: &Path) -> String {
    format!(
        "#!/bin/sh\n\
         # Managed by hostnur-agent: copy renewed certificates to Hostnur sites.\n\
         exec '{}' renew-hook\n",
        agent.display()
    )
}

/// Install (or update) the certbot deploy hook. Skipped when certbot's
/// configuration directory does not exist.
pub fn install_hook(hooks_dir: &Path, agent: &Path) -> Result<bool, TlsError> {
    use std::os::unix::fs::PermissionsExt;

    let Some(letsencrypt) = hooks_dir.parent().and_then(Path::parent) else {
        return Ok(false);
    };
    if !letsencrypt.is_dir() {
        return Ok(false);
    }
    std::fs::create_dir_all(hooks_dir).map_err(io)?;
    let path = hooks_dir.join(HOOK_NAME);
    let script = hook_script(agent);
    if std::fs::read_to_string(&path).is_ok_and(|current| current == script) {
        return Ok(false);
    }
    let temp = hooks_dir.join(format!(".{HOOK_NAME}.tmp.{}", std::process::id()));
    let mut file = std::fs::File::create(&temp).map_err(io)?;
    file.write_all(script.as_bytes()).map_err(io)?;
    file.sync_all().map_err(io)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755)).map_err(io)?;
    std::fs::rename(&temp, &path).map_err(io)?;
    Ok(true)
}

/// Copy a renewed lineage to every site recorded against it, then validate and
/// reload Nginx through `reload`. The renewed material is validated (key match,
/// not expired, covers the lineage host) before any site is touched. Returns
/// the refreshed sites; a lineage no site uses is not an error.
pub fn refresh_renewed(
    lineage: &Path,
    live_dir: &Path,
    tls_dir: &Path,
    certs: &CertificateManager,
    reload: impl FnOnce() -> Result<(), String>,
) -> Result<Vec<String>, TlsError> {
    let host = lineage
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|_| lineage.parent() == Some(live_dir))
        .ok_or_else(|| {
            TlsError::Invalid(format!("not a certbot lineage: {}", lineage.display()))
        })?;
    validate_hostname(host).map_err(|error| TlsError::Invalid(error.to_string()))?;

    let sites = sites_for_lineage(tls_dir, lineage);
    if sites.is_empty() {
        return Ok(sites);
    }

    let renewed = from_live_dir(lineage);
    certs.validate(&renewed.fullchain, &renewed.key, host)?;
    for site in &sites {
        certs.install(&renewed, &tls_dir.join(site))?;
    }
    reload().map_err(TlsError::Tool)?;
    Ok(sites)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(tls: &Path, name: &str, chain: &str) -> PathBuf {
        let dir = tls.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fullchain.pem"), chain).unwrap();
        dir
    }

    #[test]
    fn markers_select_the_sites_of_a_lineage() {
        let dir = tempfile::tempdir().unwrap();
        let lineage = Path::new("/etc/letsencrypt/live/app.example.com");
        let a = site(dir.path(), "hn-a", "a");
        site(dir.path(), "hn-b", "b");

        record_lineage(&a, lineage).unwrap();
        assert_eq!(sites_for_lineage(dir.path(), lineage), ["hn-a"]);

        clear_lineage(&a).unwrap();
        clear_lineage(&a).unwrap();
        assert!(sites_for_lineage(dir.path(), lineage).is_empty());
    }

    #[test]
    fn adopts_only_byte_identical_chains_without_a_marker() {
        let dir = tempfile::tempdir().unwrap();
        let tls = dir.path().join("tls");
        let live = dir.path().join("live");
        let lineage = live.join("app.example.com");
        std::fs::create_dir_all(&lineage).unwrap();
        std::fs::write(lineage.join("fullchain.pem"), "issued").unwrap();

        site(&tls, "hn-copied", "issued");
        site(&tls, "hn-uploaded", "something else");
        let marked = site(&tls, "hn-marked", "issued");
        record_lineage(&marked, Path::new("/elsewhere")).unwrap();

        let adopted = adopt_lineages(&tls, &live);
        assert_eq!(adopted, [("hn-copied".to_string(), lineage.clone())]);
        assert_eq!(sites_for_lineage(&tls, &lineage), ["hn-copied"]);
        // Running again changes nothing.
        assert!(adopt_lineages(&tls, &live).is_empty());
    }

    #[test]
    fn refuses_a_path_outside_the_live_directory() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live");
        let error = refresh_renewed(
            &dir.path().join("elsewhere/app.example.com"),
            &live,
            dir.path(),
            &CertificateManager::default(),
            || panic!("must not reload"),
        )
        .unwrap_err();
        assert!(matches!(error, TlsError::Invalid(_)));
    }

    #[test]
    fn an_unused_lineage_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live");
        let sites = refresh_renewed(
            &live.join("panel.example.com"),
            &live,
            dir.path(),
            &CertificateManager::default(),
            || panic!("must not reload"),
        )
        .unwrap();
        assert!(sites.is_empty());
    }

    #[test]
    fn installs_an_executable_hook_once() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let hooks = dir.path().join("letsencrypt/renewal-hooks/deploy");
        let agent = Path::new("/usr/local/bin/hostnur-agent");
        assert!(!install_hook(&hooks, agent).unwrap(), "no certbot, no hook");

        std::fs::create_dir_all(dir.path().join("letsencrypt")).unwrap();
        assert!(install_hook(&hooks, agent).unwrap());
        assert!(!install_hook(&hooks, agent).unwrap());
        let path = hooks.join(HOOK_NAME);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), hook_script(agent));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}
