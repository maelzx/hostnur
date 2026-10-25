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
    /// Request path the activation health gate probes.
    pub health_path: &'a str,
    pub allow_protocol: &'a str,
    pub run_migrations: bool,
    /// The operator explicitly acknowledged a recovery point for migrations.
    pub recovery_acknowledged: bool,
    /// Run the unprivileged Composer build when `composer.json` is present.
    pub build: bool,
    pub env: BTreeMap<String, String>,
    /// When set (execution-driven deploy), cancellation of that accepted
    /// execution is checked at safe boundaries before activation.
    pub execution_id: Option<Uuid>,
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
    Cancelled,
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
            DeployError::Cancelled => f.write_str("deployment was cancelled before activation"),
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
    /// Root-only store holding generated credentials (e.g. the app's database).
    pub secrets_dir: PathBuf,
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
        // Cancellation boundary: abort before any network/source work starts.
        if let Some(execution_id) = &input.execution_id
            && matches!(
                self.journal.execution_status(execution_id),
                Ok(Some(crate::contract::OperationStatus::Cancelled))
            )
        {
            return Err(DeployError::Cancelled);
        }
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
        // Cancellation boundary: a cancel request stops the run here, before
        // anything is published, and the journal status is left `cancelled`.
        if let Some(execution_id) = &input.execution_id
            && matches!(
                self.journal.execution_status(execution_id),
                Ok(Some(crate::contract::OperationStatus::Cancelled))
            )
        {
            return Err(DeployError::Cancelled);
        }
        let previous = self.current_target(&input.application_id);
        self.switch_current(&input.application_id, &release_dir)?;

        if let Err(error) = self.reload_all(input.runtime_version) {
            self.restore_current(&input.application_id, previous.as_deref())?;
            let _ = self.reload_all(input.runtime_version);
            return Err(error);
        }

        let mut health = HealthClient::new(input.server_name);
        health.path = input.health_path.to_string();
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
        health_path: &str,
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
        if let Err(error) = self.reload_all(&version) {
            self.restore_current(application_id, previous.as_deref())?;
            let _ = self.reload_all(&version);
            return Err(error);
        }
        let mut health = HealthClient::new(server_name);
        health.path = health_path.to_string();
        health.expect_body_contains = None;
        let mut result = health.probe();
        for _ in 0..20 {
            if result.healthy {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
            result = health.probe();
        }
        if !result.healthy {
            self.restore_current(application_id, previous.as_deref())?;
            let _ = self.reload_all(&version);
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
    /// Overlay the application's managed-database environment, reading the
    /// credential from the agent's root-only secret store.
    fn with_database_env(&self, application_id: &Uuid, mut env_text: String) -> String {
        let Ok(Some(database)) = self.journal.database_for_application(application_id) else {
            return env_text;
        };
        let Ok(password) =
            crate::secrets::read_secret(&self.secrets_dir, database.resource_id.as_str())
        else {
            return env_text;
        };
        let entries = [
            ("DB_CONNECTION", "mysql".to_string()),
            ("DB_HOST", database.host.clone()),
            ("DB_PORT", "3306".to_string()),
            ("DB_DATABASE", database.db_name.clone()),
            ("DB_USERNAME", database.username.clone()),
            ("DB_PASSWORD", password),
        ];
        for (key, value) in entries {
            env_text = upsert_env(&env_text, key, &value);
        }
        env_text
    }

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
        // Inject the app's managed database credentials (agent-owned, never sent
        // to the panel) so a Laravel app uses MariaDB instead of a local SQLite.
        env_text = self.with_database_env(&input.application_id, env_text);

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

            // Migrations mutate shared state before activation. They run only
            // when the caller explicitly acknowledged a recovery point.
            if input.run_migrations {
                if !input.recovery_acknowledged {
                    return Err(DeployError::MigrationPrerequisiteMissing);
                }

                let outcome = self.run_app(
                    &php,
                    &["artisan", "migrate", "--force"],
                    release_dir,
                    uid,
                    gid,
                    &[],
                )?;

                if !outcome.succeeded() {
                    return Err(DeployError::Provision(
                        "php artisan migrate failed".to_string(),
                    ));
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
        // The staging symlink name must be unique per switch: concurrent
        // deployments (and rollbacks) of the same application share only the
        // final `current` path, never the temp path.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let temp = self.app_root(application_id).join(format!(
            ".current.{}.{}.{}.tmp",
            release_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("switch"),
            std::process::id(),
            nonce
        ));
        std::os::unix::fs::symlink(release_dir, &temp).map_err(io)?;
        if let Err(error) = std::fs::rename(&temp, &current) {
            let _ = std::fs::remove_file(&temp);
            return Err(io(error));
        }
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
        Self::run_bounded(
            Path::new("/usr/bin/systemctl"),
            &["reload", &format!("php{version}-fpm")],
            "fpm reload failed",
        )?;
        Self::run_bounded(
            Path::new("/usr/sbin/nginx"),
            &["-s", "reload"],
            "nginx reload failed",
        )
    }

    /// Run a fixed reload command through the bounded process primitive
    /// (deadline, cleared environment, capped output) like every other spawn.
    fn run_bounded(
        program: &Path,
        args: &[&str],
        failure: &'static str,
    ) -> Result<(), DeployError> {
        let mut spec = CommandSpec::new(program, Duration::from_secs(30));
        spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
        let mut env = BTreeMap::new();
        env.insert(
            "PATH".to_string(),
            "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        );
        env.insert("LC_ALL".to_string(), "C".to_string());
        spec.env = env;
        spec.output_cap_bytes = 32_768;
        let outcome = run(&spec).map_err(|error| DeployError::Provider(error.to_string()))?;
        if outcome.succeeded() {
            Ok(())
        } else {
            Err(DeployError::Provider(failure.to_string()))
        }
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
    pub secrets_dir: PathBuf,
}

impl DeployRuntime {
    /// Run a deploy for an accepted execution, updating journal state and events.
    pub fn run_worker(
        &self,
        execution_id: Uuid,
        application_id: Uuid,
        git_ref: crate::git::GitRef,
        run_migrations: bool,
        recovery_acknowledged: bool,
    ) {
        let journal = match Journal::open(&self.db_path) {
            Ok(journal) => journal,
            Err(error) => {
                crate::logging::log(
                    crate::logging::Level::Error,
                    "deploy",
                    "worker.journal_unavailable",
                    &format!("execution {execution_id} could not be started: {error}"),
                    Some("journal"),
                    None,
                );
                return;
            }
        };
        let _ = journal.set_status(
            &execution_id,
            crate::contract::OperationStatus::Running,
            Some(5),
        );
        let _ = journal.append_event(&execution_id, "Deployment started.");

        let cancelled_midrun = matches!(
            journal.execution_status(&execution_id),
            Ok(Some(crate::contract::OperationStatus::Cancelled))
        );
        match self.execute(
            &journal,
            &application_id,
            git_ref,
            run_migrations,
            recovery_acknowledged,
            execution_id.clone(),
        ) {
            Ok((release_id, commit)) => {
                if cancelled_midrun {
                    let _ = journal.append_event(
                        &execution_id,
                        "A cancellation was requested during the run, but activation had already begun; the true outcome is recorded.",
                    );
                }
                // The release id is what a later `application.rollback` names.
                let result = serde_json::json!({
                    "release": release_id.as_str(),
                    "commit": commit,
                })
                .to_string();
                if let Err(error) = journal.finish_execution(
                    &execution_id,
                    crate::contract::OperationStatus::Succeeded,
                    Some(&result),
                    &format!("Activated release {release_id} ({commit})."),
                ) {
                    Self::log_terminal_write_failure(&execution_id, &error);
                }
            }
            Err(DeployError::Cancelled) => {
                let _ = journal.append_event(
                    &execution_id,
                    "Deployment cancelled at a safe boundary before activation; nothing was published.",
                );
            }
            Err(error) => {
                let _ = journal.append_event(&execution_id, &format!("Deployment failed: {error}"));
                if let Err(write_error) = journal.set_status(
                    &execution_id,
                    crate::contract::OperationStatus::Failed,
                    Some(100),
                ) {
                    Self::log_terminal_write_failure(&execution_id, &write_error);
                }
            }
        }
    }

    fn log_terminal_write_failure(execution_id: &Uuid, error: &crate::journal::JournalError) {
        crate::logging::log(
            crate::logging::Level::Error,
            "deploy",
            "worker.terminal_write_failed",
            &format!("execution {execution_id}: {error}"),
            Some("journal"),
            None,
        );
    }

    /// Roll an application back to a previously activated release for an
    /// accepted execution. Code only: database changes made by later releases
    /// are not reverted, and the journal says so.
    pub fn run_rollback_worker(&self, execution_id: Uuid, application_id: Uuid, release_id: Uuid) {
        let journal = match Journal::open(&self.db_path) {
            Ok(journal) => journal,
            Err(error) => {
                crate::logging::log(
                    crate::logging::Level::Error,
                    "deploy",
                    "worker.journal_unavailable",
                    &format!("execution {execution_id} could not be started: {error}"),
                    Some("journal"),
                    None,
                );
                return;
            }
        };
        let _ = journal.set_status(
            &execution_id,
            crate::contract::OperationStatus::Running,
            Some(5),
        );
        let _ = journal.append_event(
            &execution_id,
            &format!("Rollback to release {release_id} started."),
        );

        let (status, result, event) = match self.rollback(&journal, &application_id, &release_id) {
            Ok(health) => {
                let _ = journal.append_event(
                    &execution_id,
                    "Code only: database changes made by later releases were not reverted.",
                );
                (
                    crate::contract::OperationStatus::Succeeded,
                    serde_json::json!({
                        "release": release_id.as_str(),
                        "health": {"status": health.status, "healthy": health.healthy},
                    }),
                    format!("Switched to release {release_id}; the health check passed."),
                )
            }
            Err(error) => {
                let (code, message) = match &error {
                    DeployError::MissingLayout => (
                        "resource_not_found",
                        "the release is not present on the host".to_string(),
                    ),
                    DeployError::Health(_) => (
                        "operation_failed",
                        "the release failed its health check; the previous release was restored"
                            .to_string(),
                    ),
                    other => ("operation_failed", other.to_string()),
                };
                (
                    crate::contract::OperationStatus::Failed,
                    serde_json::json!({"error": {"code": code, "message": message, "retryable": false}}),
                    format!("Rollback failed: {error}"),
                )
            }
        };
        if let Err(error) =
            journal.finish_execution(&execution_id, status, Some(&result.to_string()), &event)
        {
            Self::log_terminal_write_failure(&execution_id, &error);
        }
    }

    fn rollback(
        &self,
        journal: &Journal,
        application_id: &Uuid,
        release_id: &Uuid,
    ) -> Result<HealthResult, DeployError> {
        let source = journal
            .application_source(application_id)
            .map_err(|error| DeployError::Io(error.to_string()))?
            .ok_or(DeployError::MissingLayout)?;
        let root = ManagedRoot::open(&self.apps_root)
            .map_err(|error| DeployError::Io(error.to_string()))?;
        self.deployer(journal, &root).rollback(
            application_id,
            release_id,
            &source.server_name,
            &source.health_path,
        )
    }

    fn deployer<'j>(&'j self, journal: &'j Journal, root: &'j ManagedRoot) -> Deployer<'j> {
        Deployer {
            journal,
            accounts: &self.accounts,
            root,
            git: self.git.clone(),
            nginx: self.nginx.clone(),
            fpm: self.fpm.clone(),
            apps_root: self.apps_root.clone(),
            secrets_dir: self.secrets_dir.clone(),
        }
    }

    fn execute(
        &self,
        journal: &Journal,
        application_id: &Uuid,
        git_ref: crate::git::GitRef,
        run_migrations: bool,
        recovery_acknowledged: bool,
        execution_id: Uuid,
    ) -> Result<(Uuid, String), DeployError> {
        let source = journal
            .application_source(application_id)
            .map_err(|error| DeployError::Io(error.to_string()))?
            .ok_or(DeployError::MissingLayout)?;
        let root = ManagedRoot::open(&self.apps_root)
            .map_err(|error| DeployError::Io(error.to_string()))?;
        let deployer = self.deployer(journal, &root);
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
            health_path: &source.health_path,
            allow_protocol: &self.allow_protocol,
            run_migrations,
            recovery_acknowledged,
            build: true,
            env: BTreeMap::new(),
            execution_id: Some(execution_id),
        };
        let outcome = deployer.deploy(&input)?;
        Ok((outcome.release_id, outcome.commit))
    }
}

/// Set or replace `KEY=value` in a dotenv-style text buffer. Values
/// containing newlines are dropped so a supplied environment can never inject
/// additional dotenv keys.
fn upsert_env(text: &str, key: &str, value: &str) -> String {
    if key.contains(['\n', '\r']) || value.contains(['\n', '\r']) {
        return text.to_string();
    }
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
