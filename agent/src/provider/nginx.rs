//! Managed Nginx site rendering and transactional reload (phase 14.4/14.5).
//!
//! Configuration is agent-rendered from derived values (never caller-uploaded
//! root config). A change is validated with the global `nginx -t` before any
//! reload; a failed validation or reload restores the previous known-good file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, ProcessOutcome, run};

use super::policy::{self, RuntimeError};

/// The document-root style of a managed site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteKind {
    /// Laravel front-controller routing into `public/`.
    Laravel,
    /// Plain static file serving.
    Static,
}

/// A managed site description.
#[derive(Debug, Clone)]
pub struct ManagedSite {
    /// Safe slug used for the config/log file name (derived from the app UUID).
    pub name: String,
    pub server_name: String,
    /// Absolute document root (published release `public/` or static root).
    pub root: PathBuf,
    /// Registered FPM socket for Laravel sites.
    pub fpm_socket: Option<PathBuf>,
    pub kind: SiteKind,
}

/// Nginx failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NginxError {
    InvalidSite(String),
    ConfigTestFailed(String),
    ReloadFailed(String),
    Io(String),
}

impl std::fmt::Display for NginxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NginxError::InvalidSite(message) => write!(f, "invalid site: {message}"),
            NginxError::ConfigTestFailed(message) => write!(f, "nginx -t failed: {message}"),
            NginxError::ReloadFailed(message) => write!(f, "nginx reload failed: {message}"),
            NginxError::Io(message) => write!(f, "io: {message}"),
        }
    }
}

impl std::error::Error for NginxError {}

/// Render a managed server block. All values are derived/validated.
pub fn render_site(site: &ManagedSite) -> Result<String, NginxError> {
    if !is_safe_name(&site.name) {
        return Err(NginxError::InvalidSite("unsafe site name".to_string()));
    }
    policy::validate_hostname(&site.server_name)
        .map_err(|error: RuntimeError| NginxError::InvalidSite(error.to_string()))?;
    if !site.root.is_absolute() || site.root == Path::new("/") {
        return Err(NginxError::InvalidSite(
            "document root must be an absolute non-root path".to_string(),
        ));
    }

    let access_log = format!("/var/log/nginx/{}.access.log", site.name);
    let error_log = format!("/var/log/nginx/{}.error.log", site.name);
    let mut block = String::new();
    block.push_str("# Managed by Hostnur - do not edit.\n");
    block.push_str("server {\n");
    block.push_str("    listen 80;\n");
    block.push_str(&format!("    server_name {};\n", site.server_name));
    block.push_str(&format!("    root {};\n", site.root.display()));
    block.push_str(&format!("    access_log {access_log};\n"));
    block.push_str(&format!("    error_log {error_log};\n"));
    block.push_str("    index index.php index.html;\n");
    block.push_str("\n    location ~ /\\.(?!well-known) {\n        deny all;\n    }\n");

    match site.kind {
        SiteKind::Laravel => {
            let socket = site.fpm_socket.as_ref().ok_or_else(|| {
                NginxError::InvalidSite("laravel site needs an fpm socket".to_string())
            })?;
            if !socket.is_absolute() {
                return Err(NginxError::InvalidSite(
                    "fpm socket must be absolute".to_string(),
                ));
            }
            block.push_str("\n    location / {\n        try_files $uri $uri/ /index.php?$query_string;\n    }\n");
            block.push_str(
                "\n    location ~ \\.php$ {\n        include snippets/fastcgi-php.conf;\n",
            );
            block.push_str(&format!(
                "        fastcgi_pass unix:{};\n",
                socket.display()
            ));
            block.push_str("    }\n");
        }
        SiteKind::Static => {
            block.push_str("\n    location / {\n        try_files $uri $uri/ =404;\n    }\n");
        }
    }

    block.push_str("}\n");
    Ok(block)
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Render an HTTP-only site that serves the ACME challenge (pre-issuance).
pub fn render_http_challenge_site(
    site: &ManagedSite,
    webroot: &Path,
) -> Result<String, NginxError> {
    if !is_safe_name(&site.name) {
        return Err(NginxError::InvalidSite("unsafe site name".to_string()));
    }
    policy::validate_hostname(&site.server_name)
        .map_err(|error: RuntimeError| NginxError::InvalidSite(error.to_string()))?;
    if !site.root.is_absolute() || !webroot.is_absolute() {
        return Err(NginxError::InvalidSite(
            "paths must be absolute".to_string(),
        ));
    }
    let mut out = String::new();
    out.push_str("# Managed by Hostnur - do not edit.\n");
    out.push_str("server {\n    listen 80;\n");
    out.push_str(&format!("    server_name {};\n", site.server_name));
    out.push_str(&format!(
        "    location ^~ /.well-known/acme-challenge/ {{ root {}; }}\n",
        webroot.display()
    ));
    out.push_str(&format!("    root {};\n", site.root.display()));
    out.push_str("    location / { try_files $uri $uri/ =404; }\n}\n");
    Ok(out)
}

/// Render a managed TLS site: HTTP-01 challenge + redirect on :80, TLS on :443.
pub fn render_tls_site(
    site: &ManagedSite,
    webroot: &Path,
    fullchain: &Path,
    private_key: &Path,
) -> Result<String, NginxError> {
    if !is_safe_name(&site.name) {
        return Err(NginxError::InvalidSite("unsafe site name".to_string()));
    }
    policy::validate_hostname(&site.server_name)
        .map_err(|error: RuntimeError| NginxError::InvalidSite(error.to_string()))?;
    if !site.root.is_absolute() || site.root == Path::new("/") {
        return Err(NginxError::InvalidSite(
            "document root must be an absolute non-root path".to_string(),
        ));
    }
    for path in [webroot, fullchain, private_key] {
        if !path.is_absolute() {
            return Err(NginxError::InvalidSite(
                "paths must be absolute".to_string(),
            ));
        }
    }

    let access_log = format!("/var/log/nginx/{}.access.log", site.name);
    let error_log = format!("/var/log/nginx/{}.error.log", site.name);

    let mut out = String::new();
    out.push_str("# Managed by Hostnur - do not edit.\n");
    out.push_str("server {\n    listen 80;\n");
    out.push_str(&format!("    server_name {};\n", site.server_name));
    out.push_str(&format!(
        "    location ^~ /.well-known/acme-challenge/ {{ root {}; }}\n",
        webroot.display()
    ));
    out.push_str("    location / { return 301 https://$host$request_uri; }\n}\n\n");

    out.push_str("server {\n    listen 443 ssl;\n");
    out.push_str(&format!("    server_name {};\n", site.server_name));
    out.push_str(&format!("    ssl_certificate {};\n", fullchain.display()));
    out.push_str(&format!(
        "    ssl_certificate_key {};\n",
        private_key.display()
    ));
    out.push_str(&format!("    root {};\n", site.root.display()));
    out.push_str(&format!("    access_log {access_log};\n"));
    out.push_str(&format!("    error_log {error_log};\n"));
    out.push_str("    index index.php index.html;\n");
    out.push_str("    location ~ /\\.(?!well-known) { deny all; }\n");

    match site.kind {
        SiteKind::Laravel => {
            let socket = site.fpm_socket.as_ref().ok_or_else(|| {
                NginxError::InvalidSite("laravel site needs an fpm socket".to_string())
            })?;
            if !socket.is_absolute() {
                return Err(NginxError::InvalidSite(
                    "fpm socket must be absolute".to_string(),
                ));
            }
            out.push_str("    location / { try_files $uri $uri/ /index.php?$query_string; }\n");
            out.push_str("    location ~ \\.php$ {\n        include snippets/fastcgi-php.conf;\n");
            out.push_str(&format!(
                "        fastcgi_pass unix:{};\n",
                socket.display()
            ));
            out.push_str("    }\n");
        }
        SiteKind::Static => {
            out.push_str("    location / { try_files $uri $uri/ =404; }\n");
        }
    }
    out.push_str("}\n");
    Ok(out)
}

/// Manages files in the root-owned managed include directory.
#[derive(Debug, Clone)]
pub struct NginxManager {
    pub nginx_binary: PathBuf,
    pub sites_dir: PathBuf,
    pub deadline: Duration,
}

impl Default for NginxManager {
    fn default() -> Self {
        Self {
            nginx_binary: PathBuf::from("/usr/sbin/nginx"),
            sites_dir: PathBuf::from("/etc/nginx/hostnur-sites"),
            deadline: Duration::from_secs(30),
        }
    }
}

impl NginxManager {
    fn site_path(&self, name: &str) -> Result<PathBuf, NginxError> {
        if !is_safe_name(name) {
            return Err(NginxError::InvalidSite("unsafe site name".to_string()));
        }
        Ok(self.sites_dir.join(format!("{name}.conf")))
    }

    /// Apply (create/update) a managed site, validating before reload and
    /// restoring the previous file on any failure.
    pub fn apply(&self, name: &str, content: &str) -> Result<(), NginxError> {
        let path = self.site_path(name)?;
        std::fs::create_dir_all(&self.sites_dir).map_err(io)?;
        let previous = std::fs::read_to_string(&path).ok();

        write_atomic(&path, content)?;

        if let Err(error) = self.test_config() {
            restore(&path, previous.as_deref())?;
            return Err(error);
        }
        if let Err(error) = self.reload() {
            restore(&path, previous.as_deref())?;
            let _ = self.test_config();
            let _ = self.reload();
            return Err(error);
        }
        Ok(())
    }

    /// Remove a managed site with the same validate/rollback discipline.
    pub fn remove(&self, name: &str) -> Result<(), NginxError> {
        let path = self.site_path(name)?;
        let Ok(previous) = std::fs::read_to_string(&path) else {
            return Ok(());
        };
        std::fs::remove_file(&path).map_err(io)?;
        if let Err(error) = self.test_config() {
            restore(&path, Some(&previous))?;
            return Err(error);
        }
        if let Err(error) = self.reload() {
            restore(&path, Some(&previous))?;
            let _ = self.test_config();
            let _ = self.reload();
            return Err(error);
        }
        Ok(())
    }

    /// Validate the whole configuration, then reload (files changed outside a
    /// site apply, such as renewed certificates).
    pub fn reload_checked(&self) -> Result<(), NginxError> {
        self.test_config()?;
        self.reload()
    }

    fn test_config(&self) -> Result<(), NginxError> {
        self.run_nginx(&["-t"])
            .map_err(NginxError::ConfigTestFailed)
    }

    fn reload(&self) -> Result<(), NginxError> {
        self.run_nginx(&["-s", "reload"])
            .map_err(NginxError::ReloadFailed)
    }

    fn run_nginx(&self, args: &[&str]) -> Result<(), String> {
        let mut spec = CommandSpec::new(&self.nginx_binary, self.deadline);
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.output_cap_bytes = 65_536;
        let outcome: ProcessOutcome = run(&spec).map_err(|error| error.to_string())?;
        if outcome.succeeded() {
            Ok(())
        } else {
            Err(format!("exit code {:?}", outcome.exit_code))
        }
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<(), NginxError> {
    // Unique per-writer temp name: a config apply on the transport thread and a
    // deploy worker may write the same site concurrently; a shared temp path
    // would let one rename the other's contents into place.
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| NginxError::InvalidSite("site path is not representable".to_string()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let temp = path.parent().unwrap_or(Path::new(".")).join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    if let Err(error) = std::fs::write(&temp, content) {
        return Err(io(error));
    }
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(io(error));
    }
    Ok(())
}

fn restore(path: &Path, previous: Option<&str>) -> Result<(), NginxError> {
    match previous {
        Some(content) => write_atomic(path, content),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io(error)),
        },
    }
}

fn io(error: std::io::Error) -> NginxError {
    NginxError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn laravel_site() -> ManagedSite {
        ManagedSite {
            name: "hn-224065ac4bc7".to_string(),
            server_name: "app.example.com".to_string(),
            root: PathBuf::from(
                "/opt/hostnur/apps/224065ac-4bc7-4ba4-a06d-8c8bca950c32/releases/r1/public",
            ),
            fpm_socket: Some(PathBuf::from("/run/php/hn-224065ac4bc7.sock")),
            kind: SiteKind::Laravel,
        }
    }

    #[test]
    fn renders_laravel_block() {
        let block = render_site(&laravel_site()).unwrap();
        assert!(block.contains("server_name app.example.com;"));
        assert!(block.contains("try_files $uri $uri/ /index.php?$query_string;"));
        assert!(block.contains("fastcgi_pass unix:/run/php/hn-224065ac4bc7.sock;"));
        assert!(block.contains("deny all;"));
    }

    #[test]
    fn rejects_unsafe_values() {
        let mut site = laravel_site();
        site.name = "../evil".to_string();
        assert!(render_site(&site).is_err());

        let mut site = laravel_site();
        site.server_name = "app.example.com; }".to_string();
        assert!(render_site(&site).is_err());

        let mut site = laravel_site();
        site.root = PathBuf::from("relative");
        assert!(render_site(&site).is_err());

        let mut site = laravel_site();
        site.fpm_socket = None;
        assert!(render_site(&site).is_err());
    }

    #[test]
    fn static_site_has_no_php_block() {
        let mut site = laravel_site();
        site.kind = SiteKind::Static;
        site.fpm_socket = None;
        let block = render_site(&site).unwrap();
        assert!(!block.contains("fastcgi_pass"));
        assert!(block.contains("try_files $uri $uri/ =404;"));
    }
}
