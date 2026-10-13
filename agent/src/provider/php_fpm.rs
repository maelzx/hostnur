//! Managed PHP-FPM pool rendering and transactional reload (phase 14.3/14.5).
//!
//! One pool per application runs workers as the application's Linux identity
//! with an agent-owned socket. The pool file lives in the root-owned pool
//! directory; changes are validated with the FPM config test before reload and
//! restored on failure.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{CommandSpec, ProcessOutcome, run};
use crate::provision::policy;

/// A managed FPM pool description.
#[derive(Debug, Clone)]
pub struct ManagedPool {
    /// Safe slug for the pool/config name (derived from the app UUID).
    pub name: String,
    pub user: String,
    pub group: String,
    /// Absolute agent-owned socket path.
    pub socket: PathBuf,
    pub max_children: u32,
}

/// FPM failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FpmError {
    InvalidPool(String),
    ConfigTestFailed(String),
    ReloadFailed(String),
    Io(String),
}

impl std::fmt::Display for FpmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FpmError::InvalidPool(message) => write!(f, "invalid pool: {message}"),
            FpmError::ConfigTestFailed(message) => write!(f, "fpm -t failed: {message}"),
            FpmError::ReloadFailed(message) => write!(f, "fpm reload failed: {message}"),
            FpmError::Io(message) => write!(f, "io: {message}"),
        }
    }
}

impl std::error::Error for FpmError {}

pub fn render_pool(pool: &ManagedPool) -> Result<String, FpmError> {
    if !is_safe_name(&pool.name) {
        return Err(FpmError::InvalidPool("unsafe pool name".to_string()));
    }
    if !policy::is_valid_account_name(&pool.user) || !policy::is_valid_account_name(&pool.group) {
        return Err(FpmError::InvalidPool(
            "pool user/group must be managed accounts".to_string(),
        ));
    }
    if !pool.socket.is_absolute() || pool.socket == Path::new("/") {
        return Err(FpmError::InvalidPool("socket must be absolute".to_string()));
    }
    if pool.max_children == 0 || pool.max_children > 32 {
        return Err(FpmError::InvalidPool(
            "max_children out of range".to_string(),
        ));
    }

    let log = format!("/var/log/php-fpm-{}.log", pool.name);
    Ok(format!(
        "; Managed by Hostnur - do not edit.\n\
         [{name}]\n\
         user = {user}\n\
         group = {group}\n\
         listen = {socket}\n\
         listen.owner = www-data\n\
         listen.group = www-data\n\
         listen.mode = 0660\n\
         pm = ondemand\n\
         pm.max_children = {max_children}\n\
         pm.process_idle_timeout = 10s\n\
         pm.max_requests = 500\n\
         catch_workers_output = yes\n\
         php_admin_value[error_log] = {log}\n",
        name = pool.name,
        user = pool.user,
        group = pool.group,
        socket = pool.socket.display(),
        max_children = pool.max_children,
        log = log,
    ))
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Manages files in the root-owned FPM pool directory.
#[derive(Debug, Clone)]
pub struct FpmManager {
    pub fpm_binary: PathBuf,
    pub systemctl: PathBuf,
    pub service_name: String,
    pub pool_dir: PathBuf,
    pub deadline: Duration,
}

impl Default for FpmManager {
    fn default() -> Self {
        Self {
            fpm_binary: PathBuf::from("/usr/sbin/php-fpm8.3"),
            systemctl: PathBuf::from("/usr/bin/systemctl"),
            service_name: "php8.3-fpm".to_string(),
            pool_dir: PathBuf::from("/etc/php/8.3/fpm/pool.d"),
            deadline: Duration::from_secs(30),
        }
    }
}

impl FpmManager {
    fn pool_path(&self, name: &str) -> Result<PathBuf, FpmError> {
        if !is_safe_name(name) {
            return Err(FpmError::InvalidPool("unsafe pool name".to_string()));
        }
        Ok(self.pool_dir.join(format!("hostnur-{name}.conf")))
    }

    pub fn apply(&self, name: &str, content: &str) -> Result<(), FpmError> {
        let path = self.pool_path(name)?;
        std::fs::create_dir_all(&self.pool_dir).map_err(io)?;
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

    pub fn remove(&self, name: &str) -> Result<(), FpmError> {
        let path = self.pool_path(name)?;
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

    fn test_config(&self) -> Result<(), FpmError> {
        run_checked(&self.fpm_binary, &["-t"], self.deadline).map_err(FpmError::ConfigTestFailed)
    }

    fn reload(&self) -> Result<(), FpmError> {
        run_checked(
            &self.systemctl,
            &["reload", &self.service_name],
            self.deadline,
        )
        .map_err(FpmError::ReloadFailed)
    }
}

fn run_checked(program: &Path, args: &[&str], deadline: Duration) -> Result<(), String> {
    let mut spec = CommandSpec::new(program, deadline);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.output_cap_bytes = 32_768;
    let outcome: ProcessOutcome = run(&spec).map_err(|error| error.to_string())?;
    if outcome.succeeded() {
        Ok(())
    } else {
        Err(format!("exit code {:?}", outcome.exit_code))
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<(), FpmError> {
    let temp = path.with_extension("conf.tmp");
    std::fs::write(&temp, content).map_err(io)?;
    std::fs::rename(&temp, path).map_err(io)?;
    Ok(())
}

fn restore(path: &Path, previous: Option<&str>) -> Result<(), FpmError> {
    match previous {
        Some(content) => write_atomic(path, content),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io(error)),
        },
    }
}

fn io(error: std::io::Error) -> FpmError {
    FpmError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> ManagedPool {
        ManagedPool {
            name: "hn-224065ac4bc7".to_string(),
            user: "hn_224065ac4bc7".to_string(),
            group: "hn_224065ac4bc7".to_string(),
            socket: PathBuf::from("/run/php/hn-224065ac4bc7.sock"),
            max_children: 5,
        }
    }

    #[test]
    fn renders_pool() {
        let block = render_pool(&pool()).unwrap();
        assert!(block.contains("[hn-224065ac4bc7]"));
        assert!(block.contains("user = hn_224065ac4bc7"));
        assert!(block.contains("listen = /run/php/hn-224065ac4bc7.sock"));
        assert!(block.contains("pm.max_children = 5"));
    }

    #[test]
    fn rejects_unsafe_pools() {
        let mut bad = pool();
        bad.name = "../x".to_string();
        assert!(render_pool(&bad).is_err());

        let mut bad = pool();
        bad.user = "root".to_string();
        assert!(render_pool(&bad).is_err());

        let mut bad = pool();
        bad.max_children = 0;
        assert!(render_pool(&bad).is_err());
    }
}
