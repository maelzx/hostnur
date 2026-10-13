//! Real deployment orchestration for a `php-laravel`/PHP release (phase 15).
//!
//! Sequence: provision identity/layout -> stage a unique release and check out
//! the pinned source as the app user -> write per-release environment -> prepare
//! the FPM pool and Nginx route -> atomically switch the agent-owned `current`
//! pointer -> reload -> health-gate. A failed activation restores the previous
//! known-good release. Migrations are opt-in and refused without a recovery
//! prerequisite.

pub mod health;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::contract::Uuid;
use crate::git::{CheckoutRequest, GitRunner};
use crate::journal::Journal;
use crate::provider::{
    FpmManager, ManagedPool, ManagedSite, NginxManager, SiteKind, render_pool, render_site,
};
use crate::provision::{
    Accounts, ManagedRoot, ProvisioningService, SystemHostProbe, policy as provision_policy,
};

pub use health::{HealthClient, HealthResult};

/// Inputs for deploying one release.
pub struct DeployInput<'a> {
    pub application_id: Uuid,
    pub release_id: Uuid,
    pub provider: &'a str,
    pub runtime: &'a str,
    pub runtime_version: &'a str,
    pub repository_url: &'a str,
    pub git_ref: crate::git::GitRef,
    pub server_name: &'a str,
    pub allow_protocol: &'a str,
    pub run_migrations: bool,
    pub env: BTreeMap<String, String>,
}

/// Deployment failure.
#[derive(Debug)]
pub enum DeployError {
    Provision(String),
    Source(String),
    Provider(String),
    MigrationPrerequisiteMissing,
    MissingLayout,
    Health(HealthResult),
    Io(String),
}

impl std::fmt::Display for DeployError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployError::Provision(m) => write!(f, "provisioning failed: {m}"),
            DeployError::Source(m) => write!(f, "source failed: {m}"),
            DeployError::Provider(m) => write!(f, "provider failed: {m}"),
            DeployError::MigrationPrerequisiteMissing => {
                f.write_str("migrations require a recovery prerequisite that is not configured")
            }
            DeployError::MissingLayout => f.write_str("managed layout is missing"),
            DeployError::Health(result) => write!(f, "health check failed: {}", result.detail),
            DeployError::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for DeployError {}

/// A completed deployment.
#[derive(Debug, Clone)]
pub struct DeploymentOutcome {
    pub release_id: Uuid,
    pub commit: String,
    pub previous_release: Option<String>,
    pub health: HealthResult,
}

/// Orchestrates deployments against the local host.
pub struct Deployer<'a> {
    pub journal: &'a Journal,
    pub accounts: &'a dyn Accounts,
    pub root: &'a ManagedRoot,
    pub git: GitRunner,
    pub nginx: NginxManager,
    pub fpm: FpmManager,
    pub apps_root: PathBuf,
}

impl<'a> Deployer<'a> {
    pub fn site_name(&self, application_id: &Uuid) -> String {
        let fragment = provision_policy::account_name(application_id);
        format!("hn-{}", fragment.trim_start_matches("hn_"))
    }

    fn app_root(&self, application_id: &Uuid) -> PathBuf {
        self.apps_root.join(application_id.as_str())
    }

    fn pool_socket(&self, application_id: &Uuid) -> PathBuf {
        PathBuf::from(format!("/run/php/{}.sock", self.site_name(application_id)))
    }

    /// Deploy and activate a release.
    pub fn deploy(&self, input: &DeployInput<'_>) -> Result<DeploymentOutcome, DeployError> {
        if input.run_migrations {
            return Err(DeployError::MigrationPrerequisiteMissing);
        }

        let service = ProvisioningService::new(self.journal, &SystemHostProbe);
        let app = service
            .reconcile(&input.application_id, self.root, self.accounts)
            .map_err(|error| DeployError::Provision(error.to_string()))?;
        let uid = app.account_uid.ok_or(DeployError::MissingLayout)?;
        let gid = app.account_gid.ok_or(DeployError::MissingLayout)?;

        let release_rel = format!(
            "{}/releases/{}",
            input.application_id.as_str(),
            input.release_id.as_str()
        );
        self.root
            .create_dir(&release_rel, 0o755, Some((uid, gid)))
            .map_err(|error| DeployError::Io(error.to_string()))?;
        let release_dir = self.apps_root.join(&release_rel);

        // Source: check out the pinned ref as the app user.
        let checkout = CheckoutRequest {
            repository_url: input.repository_url,
            git_ref: &input.git_ref,
            staging_dir: &release_dir,
            uid,
            gid,
            deadline: Duration::from_secs(300),
            askpass: None,
        };
        let outcome = GitRunner {
            allow_protocol: input.allow_protocol.to_string(),
            ..self.git.clone()
        }
        .checkout(&checkout)
        .map_err(|error| DeployError::Source(error.to_string()))?;

        // Per-release environment (app-owned, private).
        if !input.env.is_empty() {
            let env_text = input
                .env
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("\n");
            self.root
                .write_file(
                    &format!("{release_rel}/.env"),
                    0o600,
                    Some((uid, gid)),
                    env_text.as_bytes(),
                )
                .map_err(|error| DeployError::Io(error.to_string()))?;
        }

        // Provider config: pool + route pointing at the agent-owned `current`.
        let pool = ManagedPool {
            name: self.site_name(&input.application_id),
            user: app.account_name.clone(),
            group: app.account_group.clone(),
            socket: self.pool_socket(&input.application_id),
            max_children: 5,
        };
        let pool_config =
            render_pool(&pool).map_err(|error| DeployError::Provider(error.to_string()))?;
        self.fpm
            .apply(&pool.name, &pool_config)
            .map_err(|error| DeployError::Provider(error.to_string()))?;

        let site = ManagedSite {
            name: self.site_name(&input.application_id),
            server_name: input.server_name.to_string(),
            root: self.app_root(&input.application_id).join("current/public"),
            fpm_socket: Some(pool.socket.clone()),
            kind: SiteKind::Laravel,
        };
        let site_config =
            render_site(&site).map_err(|error| DeployError::Provider(error.to_string()))?;
        self.nginx
            .apply(&site.name, &site_config)
            .map_err(|error| DeployError::Provider(error.to_string()))?;

        // Activation: atomically switch `current`, then reload and health-gate.
        let previous = self.current_target(&input.application_id);
        self.switch_current(&input.application_id, &release_dir)?;

        if let Err(error) = self.reload_all() {
            self.restore_current(&input.application_id, previous.as_deref())?;
            let _ = self.reload_all();
            return Err(error);
        }

        let mut health = HealthClient::new(input.server_name);
        health.expect_body_contains = None;
        // Reloads settle asynchronously; poll briefly before declaring failure.
        let mut result = health.probe();
        for _ in 0..20 {
            if result.healthy {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
            result = health.probe();
        }
        if !result.healthy {
            self.restore_current(&input.application_id, previous.as_deref())?;
            let _ = self.reload_all();
            return Err(DeployError::Health(result));
        }

        Ok(DeploymentOutcome {
            release_id: input.release_id.clone(),
            commit: outcome.commit,
            previous_release: previous.map(|path| path.to_string_lossy().into_owned()),
            health: result,
        })
    }

    /// Roll back `current` to a previously deployed release directory.
    pub fn rollback(
        &self,
        application_id: &Uuid,
        release_id: &Uuid,
        server_name: &str,
    ) -> Result<HealthResult, DeployError> {
        let release_dir = self
            .app_root(application_id)
            .join("releases")
            .join(release_id.as_str());
        if !release_dir.is_dir() {
            return Err(DeployError::MissingLayout);
        }
        self.switch_current(application_id, &release_dir)?;
        self.reload_all()?;
        let result = HealthClient::new(server_name).probe();
        if result.healthy {
            Ok(result)
        } else {
            Err(DeployError::Health(result))
        }
    }

    fn current_path(&self, application_id: &Uuid) -> PathBuf {
        self.app_root(application_id).join("current")
    }

    fn current_target(&self, application_id: &Uuid) -> Option<PathBuf> {
        std::fs::read_link(self.current_path(application_id)).ok()
    }

    fn switch_current(&self, application_id: &Uuid, release_dir: &Path) -> Result<(), DeployError> {
        let current = self.current_path(application_id);
        let temp = self.app_root(application_id).join(".current.tmp");
        let _ = std::fs::remove_file(&temp);
        std::os::unix::fs::symlink(release_dir, &temp).map_err(io)?;
        std::fs::rename(&temp, &current).map_err(io)?;
        Ok(())
    }

    fn restore_current(
        &self,
        application_id: &Uuid,
        previous: Option<&Path>,
    ) -> Result<(), DeployError> {
        match previous {
            Some(target) => self.switch_current(application_id, target),
            None => {
                let _ = std::fs::remove_file(self.current_path(application_id));
                Ok(())
            }
        }
    }

    fn reload_all(&self) -> Result<(), DeployError> {
        // Reload FPM (workers/OPcache) then Nginx. Reuse the managers' own
        // reload paths by re-applying the existing pool/site is unnecessary; a
        // direct reload is enough because the route and pool are unchanged.
        let fpm = std::process::Command::new("/usr/bin/systemctl")
            .args(["reload", "php8.3-fpm"])
            .status()
            .map_err(|error| DeployError::Provider(error.to_string()))?;
        if !fpm.success() {
            return Err(DeployError::Provider("fpm reload failed".to_string()));
        }
        let nginx = std::process::Command::new("/usr/sbin/nginx")
            .args(["-s", "reload"])
            .status()
            .map_err(|error| DeployError::Provider(error.to_string()))?;
        if !nginx.success() {
            return Err(DeployError::Provider("nginx reload failed".to_string()));
        }
        Ok(())
    }
}

fn io(error: std::io::Error) -> DeployError {
    DeployError::Io(error.to_string())
}
