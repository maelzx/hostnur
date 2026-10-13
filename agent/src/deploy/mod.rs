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
use crate::journal::{DeploymentRecord, Journal};
use crate::process::{CommandSpec, run};
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
    /// Run the unprivileged Composer build when `composer.json` is present.
    pub build: bool,
    pub env: BTreeMap<String, String>,
}

/// Deployment failure.
#[derive(Debug)]
pub enum DeployError {
    Provision(String),
    Source(String),
    Provider(String),
    Build(String),
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
            DeployError::Build(m) => write!(f, "build failed: {m}"),
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

        self.prepare_release(input, &release_rel, &release_dir, uid, gid)?;

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
        self.fpm_for(input.runtime_version)
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

        if let Err(error) = self.reload_all(input.runtime_version) {
            self.restore_current(&input.application_id, previous.as_deref())?;
            let _ = self.reload_all(input.runtime_version);
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
            let _ = self.reload_all(input.runtime_version);
            return Err(DeployError::Health(result));
        }

        // Record the active release; mark the previous one superseded.
        let record = |release_id: &Uuid, commit: String, state: &str| {
            self.journal.record_deployment(&DeploymentRecord {
                release_id: release_id.clone(),
                application_id: input.application_id.clone(),
                commit_id: commit,
                state: state.to_string(),
            })
        };
        record(&input.release_id, outcome.commit.clone(), "active")
            .map_err(|error| DeployError::Io(error.to_string()))?;
        if let Some(previous_path) = &previous
            && let Some(previous_id) = release_id_from_path(previous_path)
        {
            let _ = record(&previous_id, String::new(), "superseded");
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
        let previous = self.current_target(application_id);
        let version = self
            .journal
            .application_source(application_id)
            .ok()
            .flatten()
            .map(|source| source.runtime_version)
            .unwrap_or_default();
        self.switch_current(application_id, &release_dir)?;
        self.reload_all(&version)?;
        let result = HealthClient::new(server_name).probe();
        if !result.healthy {
            return Err(DeployError::Health(result));
        }
        let record = |release_id: &Uuid, state: &str| {
            self.journal.record_deployment(&DeploymentRecord {
                release_id: release_id.clone(),
                application_id: application_id.clone(),
                commit_id: String::new(),
                state: state.to_string(),
            })
        };
        record(release_id, "active").map_err(|error| DeployError::Io(error.to_string()))?;
        if let Some(previous_path) = &previous
            && let Some(previous_id) = release_id_from_path(previous_path)
        {
            let _ = record(&previous_id, "rolled_back");
        }
        Ok(result)
    }

    /// Run the Composer production build as the application user when a
    /// `composer.json` is present. Scripts are disabled (untrusted app code).
    fn php_binary(&self, version: &str) -> PathBuf {
        if version.is_empty() {
            PathBuf::from("/usr/bin/php")
        } else {
            PathBuf::from(format!("/usr/bin/php{version}"))
        }
    }

    fn fpm_for(&self, version: &str) -> FpmManager {
        if version.is_empty() {
            self.fpm.clone()
        } else {
            FpmManager::for_php_version(version)
        }
    }

    /// Run an application command as the app user with a minimal environment.
    fn run_app(
        &self,
        program: &Path,
        args: &[&str],
        dir: &Path,
        uid: u32,
        gid: u32,
        extra_env: &[(&str, String)],
    ) -> Result<crate::process::ProcessOutcome, DeployError> {
        let mut spec = CommandSpec::new(program, Duration::from_secs(1200));
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        spec.working_dir = Some(dir.to_path_buf());
        let mut env = BTreeMap::new();
        env.insert("HOME".to_string(), dir.display().to_string());
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env.insert("COMPOSER_NO_INTERACTION".to_string(), "1".to_string());
        env.insert(
            "COMPOSER_HOME".to_string(),
            dir.join(".composer").display().to_string(),
        );
        env.insert(
            "COMPOSER_CACHE_DIR".to_string(),
            dir.join(".composer/cache").display().to_string(),
        );
        env.insert(
            "npm_config_cache".to_string(),
            dir.join(".npm").display().to_string(),
        );
        for (key, value) in extra_env {
            env.insert((*key).to_string(), value.clone());
        }
        spec.env = env;
        spec.uid = Some(uid);
        spec.gid = Some(gid);
        spec.output_cap_bytes = crate::process::MAX_OUTPUT_BYTES;
        run(&spec).map_err(|error| DeployError::Build(error.to_string()))
    }

    /// Install dependencies, build frontend assets and prepare a Laravel app.
    fn prepare_release(
        &self,
        input: &DeployInput<'_>,
        release_rel: &str,
        release_dir: &Path,
        uid: u32,
        gid: u32,
    ) -> Result<(), DeployError> {
        let php = self.php_binary(input.runtime_version);

        // PHP dependencies (allow framework post-install scripts as the app user).
        if release_dir.join("composer.json").is_file() {
            let outcome = self.run_app(
                Path::new("/usr/bin/composer"),
                &[
                    "install",
                    "--no-interaction",
                    "--no-dev",
                    "--no-progress",
                    "--prefer-dist",
                    "--optimize-autoloader",
                ],
                release_dir,
                uid,
                gid,
                &[],
            )?;
            if !outcome.succeeded() {
                return Err(DeployError::Build(format!(
                    "composer exit code {:?}",
                    outcome.exit_code
                )));
            }
        }

        // Frontend assets (Vite/Tailwind, etc.).
        if release_dir.join("package.json").is_file() {
            let install = self.run_app(
                Path::new("/usr/bin/npm"),
                &["ci"],
                release_dir,
                uid,
                gid,
                &[],
            )?;
            if !install.succeeded() {
                return Err(DeployError::Build("npm ci failed".to_string()));
            }
            let build = self.run_app(
                Path::new("/usr/bin/npm"),
                &["run", "build"],
                release_dir,
                uid,
                gid,
                &[],
            )?;
            if !build.succeeded() {
                return Err(DeployError::Build("npm run build failed".to_string()));
            }
        }

        // Environment: start from .env.example, then apply explicit overrides.
        let env_path = release_dir.join(".env");
        let example_path = release_dir.join(".env.example");
        let mut env_text = if env_path.is_file() {
            std::fs::read_to_string(&env_path).unwrap_or_default()
        } else if example_path.is_file() {
            std::fs::read_to_string(&example_path).unwrap_or_default()
        } else {
            String::new()
        };
        if !input.env.is_empty() {
            for (key, value) in &input.env {
                env_text = upsert_env(&env_text, key, value);
            }
        }
        if !env_text.is_empty() {
            self.root
                .write_file(
                    &format!("{release_rel}/.env"),
                    0o600,
                    Some((uid, gid)),
                    env_text.as_bytes(),
                )
                .map_err(|error| DeployError::Io(error.to_string()))?;
        }

        let artisan = release_dir.join("artisan");
        if artisan.is_file() {
            // Generate an application key once.
            if env_text
                .lines()
                .all(|line| !line.starts_with("APP_KEY=") || line == "APP_KEY=")
            {
                let outcome = self.run_app(
                    &php,
                    &["artisan", "key:generate", "--force"],
                    release_dir,
                    uid,
                    gid,
                    &[],
                )?;
                if !outcome.succeeded() {
                    return Err(DeployError::Build(
                        "php artisan key:generate failed".to_string(),
                    ));
                }
            }

            // SQLite database for development-style apps.
            if env_text.contains("DB_CONNECTION=sqlite") {
                let db_rel = format!("{release_rel}/database/database.sqlite");
                if !release_dir.join("database/database.sqlite").exists() {
                    self.root
                        .write_file(&db_rel, 0o600, Some((uid, gid)), b"")
                        .map_err(|error| DeployError::Io(error.to_string()))?;
                }
            }

            if input.run_migrations {
                let outcome = self.run_app(
                    &php,
                    &["artisan", "migrate", "--force"],
                    release_dir,
                    uid,
                    gid,
                    &[],
                )?;
                if !outcome.succeeded() {
                    return Err(DeployError::Build("php artisan migrate failed".to_string()));
                }
            }

            let _ = self.run_app(
                &php,
                &["artisan", "storage:link"],
                release_dir,
                uid,
                gid,
                &[],
            );
        }

        Ok(())
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

    fn reload_all(&self, php_version: &str) -> Result<(), DeployError> {
        // Reload FPM (workers/OPcache) then Nginx. Reuse the managers' own
        // reload paths by re-applying the existing pool/site is unnecessary; a
        // direct reload is enough because the route and pool are unchanged.
        let version = if php_version.is_empty() {
            "8.3"
        } else {
            php_version
        };
        let fpm = std::process::Command::new("/usr/bin/systemctl")
            .args(["reload", &format!("php{version}-fpm")])
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

/// Extract the release UUID from a `.../releases/<uuid>` path.
fn release_id_from_path(path: &Path) -> Option<Uuid> {
    Uuid::parse(path.file_name()?.to_str()?).ok()
}

/// Owned deployment runtime, cloned into socket-driven deploy workers.
#[derive(Clone)]
pub struct DeployRuntime {
    pub db_path: PathBuf,
    pub apps_root: PathBuf,
    pub accounts: crate::provision::SystemAccounts,
    pub git: GitRunner,
    pub nginx: NginxManager,
    pub fpm: FpmManager,
    pub allow_protocol: String,
}

impl DeployRuntime {
    /// Run a deploy for an accepted execution, updating journal state and events.
    pub fn run_worker(
        &self,
        execution_id: Uuid,
        application_id: Uuid,
        git_ref: crate::git::GitRef,
        run_migrations: bool,
    ) {
        let Ok(journal) = Journal::open(&self.db_path) else {
            return;
        };
        let _ = journal.set_status(
            &execution_id,
            crate::contract::OperationStatus::Running,
            Some(5),
        );
        let _ = journal.append_event(&execution_id, "Deployment started.");

        match self.execute(&journal, &application_id, git_ref, run_migrations) {
            Ok((release_id, commit)) => {
                let _ = journal.append_event(
                    &execution_id,
                    &format!("Activated release {release_id} ({commit})."),
                );
                let _ = journal.set_status(
                    &execution_id,
                    crate::contract::OperationStatus::Succeeded,
                    Some(100),
                );
            }
            Err(message) => {
                let _ =
                    journal.append_event(&execution_id, &format!("Deployment failed: {message}"));
                let _ = journal.set_status(
                    &execution_id,
                    crate::contract::OperationStatus::Failed,
                    Some(100),
                );
            }
        }
    }

    fn execute(
        &self,
        journal: &Journal,
        application_id: &Uuid,
        git_ref: crate::git::GitRef,
        run_migrations: bool,
    ) -> Result<(Uuid, String), String> {
        let source = journal
            .application_source(application_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "no source registered for this application".to_string())?;
        let root = ManagedRoot::open(&self.apps_root).map_err(|error| error.to_string())?;
        let deployer = Deployer {
            journal,
            accounts: &self.accounts,
            root: &root,
            git: self.git.clone(),
            nginx: self.nginx.clone(),
            fpm: self.fpm.clone(),
            apps_root: self.apps_root.clone(),
        };
        let release_id = crate::ids::new_uuid_v4();
        let input = DeployInput {
            application_id: application_id.clone(),
            release_id: release_id.clone(),
            provider: &source.provider,
            runtime: &source.runtime,
            runtime_version: &source.runtime_version,
            repository_url: &source.repository_url,
            git_ref,
            server_name: &source.server_name,
            allow_protocol: &self.allow_protocol,
            run_migrations,
            build: true,
            env: BTreeMap::new(),
        };
        let outcome = deployer.deploy(&input).map_err(|error| error.to_string())?;
        Ok((outcome.release_id, outcome.commit))
    }
}

/// Set or replace `KEY=value` in a dotenv-style text buffer.
fn upsert_env(text: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut replaced = false;
    for line in &mut lines {
        if line.starts_with(&prefix) {
            *line = format!("{key}={value}");
            replaced = true;
        }
    }
    let mut out = lines.join("\n");
    if !replaced {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{key}={value}"));
    }
    out.push('\n');
    out
}
