//! Request dispatch: maps a validated operation to a response body.
//!
//! Control operations (`agent.capabilities`, `agent.inspect`, `agent.cancel`)
//! back the panel's agent-client methods; `server.snapshot` is read-only.
//! Mutation operations run through the durable journal's accept→execute→
//! receipt pipeline. An operation outside `SUPPORTED_OPERATIONS` and the
//! `handle` match returns a safe `unsupported_operation` error rather than
//! pretending to run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use crate::process::{CommandSpec, run};

use crate::clock;
use crate::contract::{
    AgentMode, CapabilitySnapshot, ContractErrorCode, OperationReceipt, OperationRequest,
    OperationSnapshot, OperationStatus, Uuid,
};
use crate::database::{
    DatabaseError, MariaDbAdmin, database_name, generate_password, is_managed_database,
    is_managed_username, username,
};
use crate::deploy::DeployRuntime;
use crate::firewall::{FirewallError, FirewallPolicy};
use crate::git::policy::{NetworkPolicy, validate_repository_url};
use crate::git::{GitRef, validate_ref};
use crate::journal::{ApplicationSource, CertificateRecord, DatabaseResource, Journal};
use crate::observe;
use crate::packages::{PackageError, PackagePolicy};
use crate::provider::policy::validate_hostname;
use crate::provider::{
    ManagedSite, NginxManager, RuntimePolicy, ScheduleUnit, SiteKind, WorkerManager,
    render_http_challenge_site, render_tls_site,
};
use crate::services::{ServiceAction, ServiceError, ServicePolicy};
use crate::tls::{CertbotClient, CertbotRequest, CertificateManager, from_live_dir};
use crate::transport::RequestHandler;

/// Operations this build actually advertises and can serve. Must stay in sync
/// with the match arms in `RequestHandler::handle` (minus the conditional
/// `application.deploy`, added in `capabilities`).
pub const SUPPORTED_OPERATIONS: [&str; 33] = [
    "agent.capabilities",
    "agent.inspect",
    "agent.cancel",
    "agent.register_application",
    "agent.register_package",
    "agent.remove_package",
    "server.snapshot",
    "server.updates",
    "service.control",
    "package.install",
    "package.upgrade",
    "firewall.rule.add",
    "firewall.rule.remove",
    "firewall.web",
    "application.logs",
    "application.metrics",
    "application.observe",
    "application.start",
    "application.stop",
    "application.restart",
    "schedule.apply",
    "agent.register_schedule",
    "agent.remove_schedule",
    "database.provision",
    "database.rotate",
    "database.export",
    "database.import",
    "agent.register_database",
    "agent.remove_database",
    "certificate.issue",
    "agent.register_certificate",
    "agent.register_backup",
    "backup.create",
];

/// Upper bound on concurrent deploy workers. Extra deploys are refused with
/// `agent_unavailable` (backpressure) rather than spawning unbounded root-
/// supervised OS threads from a misbehaving or compromised panel identity.
pub const MAX_CONCURRENT_DEPLOYS: usize = 4;

static DEPLOY_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// RAII deploy ticket: releases the slot even if the worker panics.
struct DeployTicket;

impl DeployTicket {
    fn try_acquire() -> Option<Self> {
        if DEPLOY_IN_FLIGHT.fetch_add(1, Ordering::AcqRel) >= MAX_CONCURRENT_DEPLOYS {
            DEPLOY_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Self)
    }
}

impl Drop for DeployTicket {
    fn drop(&mut self) {
        DEPLOY_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Handler backed by the durable journal.
pub struct AgentHandler<'a> {
    journal: &'a Journal,
    deploy: Option<DeployRuntime>,
    policy: RuntimePolicy,
    services: ServicePolicy,
    packages: PackagePolicy,
    firewall: FirewallPolicy,
    worker: WorkerManager,
    apps_root: PathBuf,
    db: MariaDbAdmin,
    secrets_dir: PathBuf,
    dumps_dir: PathBuf,
    certbot: CertbotClient,
    certs: CertificateManager,
    tls_nginx: NginxManager,
    acme_webroot: PathBuf,
    acme_email: Option<String>,
    acme_staging: bool,
    backup_recipient: Option<String>,
    backup_dir: PathBuf,
}

impl<'a> AgentHandler<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self {
            journal,
            deploy: None,
            policy: RuntimePolicy::default(),
            services: ServicePolicy::default(),
            packages: PackagePolicy::default(),
            firewall: FirewallPolicy::default(),
            worker: WorkerManager::default(),
            apps_root: PathBuf::from("/opt/hostnur/apps"),
            db: MariaDbAdmin::default(),
            secrets_dir: PathBuf::from("/var/lib/hostnur/agent/secrets"),
            dumps_dir: PathBuf::from("/var/lib/hostnur/agent/dumps"),
            certbot: CertbotClient::default(),
            certs: CertificateManager::default(),
            tls_nginx: NginxManager::default(),
            acme_webroot: PathBuf::from("/var/www/hostnur-acme"),
            acme_email: None,
            acme_staging: false,
            backup_recipient: None,
            backup_dir: PathBuf::from("/var/lib/hostnur/backups"),
        }
    }

    /// Handler with a privileged deployment runtime and runtime policy enabled.
    pub fn with_deploy(journal: &'a Journal, deploy: DeployRuntime, policy: RuntimePolicy) -> Self {
        Self {
            journal,
            deploy: Some(deploy),
            policy,
            services: ServicePolicy::default(),
            packages: PackagePolicy::default(),
            firewall: FirewallPolicy::default(),
            worker: WorkerManager::default(),
            apps_root: PathBuf::from("/opt/hostnur/apps"),
            db: MariaDbAdmin::default(),
            secrets_dir: PathBuf::from("/var/lib/hostnur/agent/secrets"),
            dumps_dir: PathBuf::from("/var/lib/hostnur/agent/dumps"),
            certbot: CertbotClient::default(),
            certs: CertificateManager::default(),
            tls_nginx: NginxManager::default(),
            acme_webroot: PathBuf::from("/var/www/hostnur-acme"),
            acme_email: None,
            acme_staging: false,
            backup_recipient: None,
            backup_dir: PathBuf::from("/var/lib/hostnur/backups"),
        }
    }

    fn register_database(&self, request: &OperationRequest) -> Value {
        let string = |key: &str| request.parameters.get(key).and_then(Value::as_str);
        let Some(resource_id) = string("database").and_then(|value| Uuid::parse(value).ok()) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "database must be a UUID",
            );
        };
        let Some(application_id) = string("application").and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "application must be a UUID",
            );
        };

        if let Ok(Some(existing)) = self.journal.database(&resource_id) {
            if existing.application_id != application_id {
                return error_body(
                    ContractErrorCode::Conflict,
                    "database identity conflicts with another application",
                );
            }
            return json!({"protocol_version": 1, "registered": true, "database": existing.db_name, "username": existing.username});
        }

        let record = DatabaseResource {
            resource_id: resource_id.clone(),
            application_id,
            engine: "mariadb".to_string(),
            db_name: database_name(&resource_id),
            username: username(&resource_id),
            host: self.db.host.clone(),
            state: "registered".to_string(),
        };
        match self.journal.set_database(&record) {
            Ok(()) => {
                json!({"protocol_version": 1, "registered": true, "database": record.db_name, "username": record.username})
            }
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn database_provision(&self, request: &OperationRequest) -> Value {
        let Some(resource_id) = request
            .parameters
            .get("request")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "request must be a UUID",
            );
        };

        let record = match self.journal.database(&resource_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "database is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        if record.state == "ready" {
            return json!({"protocol_version": 1, "provisioned": true, "state": "ready", "database": record.db_name, "username": record.username, "host": record.host});
        }

        let password = match generate_password() {
            Ok(password) => password,
            Err(error) => return error_body(ContractErrorCode::OperationFailed, &error),
        };

        if let Err(error) = self
            .db
            .provision(&record.db_name, &record.username, &password)
        {
            let _ = self.journal.update_database_state(&resource_id, "failed");
            return database_error(error);
        }
        if let Err(error) = self
            .db
            .check_connection(&record.db_name, &record.username, &password)
        {
            let _ = self.journal.update_database_state(&resource_id, "failed");
            return database_error(error);
        }
        if let Err(error) =
            crate::secrets::write_secret(&self.secrets_dir, resource_id.as_str(), &password)
        {
            let _ = self.journal.update_database_state(&resource_id, "failed");
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }
        let _ = self.journal.update_database_state(&resource_id, "ready");

        json!({"protocol_version": 1, "provisioned": true, "state": "ready", "database": record.db_name, "username": record.username, "host": record.host})
    }

    fn remove_database(&self, request: &OperationRequest) -> Value {
        let Some(resource_id) = request
            .parameters
            .get("database")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "database must be a UUID",
            );
        };
        let record = match self.journal.database(&resource_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "database is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        if !is_managed_database(&record.db_name) || !is_managed_username(&record.username) {
            return error_body(
                ContractErrorCode::PermissionDenied,
                "refusing to drop an unmanaged database",
            );
        }
        if let Err(error) = self.db.remove(&record.db_name, &record.username) {
            return database_error(error);
        }
        let _ = crate::secrets::remove_secret(&self.secrets_dir, resource_id.as_str());
        let _ = self.journal.remove_database(&resource_id);
        json!({"protocol_version": 1, "removed": true, "database": record.db_name})
    }

    /// Configure ACME (called from process startup with the agent config).
    pub fn configure_acme(&mut self, email: Option<String>, staging: bool) {
        self.acme_email = email;
        self.acme_staging = staging;
    }

    /// Configure the backup recipient (age public key) and local staging dir.
    pub fn configure_backup(&mut self, recipient: Option<String>, dir: PathBuf) {
        self.backup_recipient = recipient;
        self.backup_dir = dir;
    }

    fn site_name(&self, application_id: &Uuid) -> String {
        format!(
            "hn-{}",
            crate::provision::account_name(application_id).trim_start_matches("hn_")
        )
    }

    fn register_certificate(&self, request: &OperationRequest) -> Value {
        let string = |key: &str| request.parameters.get(key).and_then(Value::as_str);
        let Some(certificate_id) = string("certificate") else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "certificate is required",
            );
        };
        if certificate_id.is_empty()
            || certificate_id.len() > 64
            || !certificate_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "certificate id is invalid",
            );
        }
        let Some(hostname) = string("hostname") else {
            return error_body(ContractErrorCode::InvalidParameters, "hostname is required");
        };
        if let Err(error) = validate_hostname(hostname) {
            return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
        }
        let application_id = string("application").and_then(|value| Uuid::parse(value).ok());

        let record = CertificateRecord {
            certificate_id: certificate_id.to_string(),
            application_id,
            hostname: hostname.to_string(),
            state: "registered".to_string(),
        };
        match self.journal.set_certificate(&record) {
            Ok(()) => json!({"protocol_version": 1, "registered": true, "hostname": hostname}),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn certificate_issue(&self, request: &OperationRequest) -> Value {
        let Some(resource) = request.resource.as_ref() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "domain resource is required",
            );
        };
        if resource.kind != "domain" {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "resource must be a domain",
            );
        }
        let record = match self.journal.certificate(&resource.id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "certificate request is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        let Some(email) = self.acme_email.clone() else {
            return error_body(
                ContractErrorCode::OperationFailed,
                "ACME contact email is not configured on the agent",
            );
        };

        // Resolve the owning application by the domain's server_name.
        let application_id = match record.application_id.clone() {
            Some(id) => id,
            None => match self
                .journal
                .application_source_by_server_name(&record.hostname)
            {
                Ok(Some(source)) => source.application_id,
                Ok(None) => {
                    return error_body(
                        ContractErrorCode::ResourceNotFound,
                        "no application is registered for this domain",
                    );
                }
                Err(error) => {
                    return error_body(ContractErrorCode::OperationFailed, &error.to_string());
                }
            },
        };

        let hostname = record.hostname.clone();
        let name = self.site_name(&application_id);
        let socket = PathBuf::from(format!("/run/php/{name}.sock"));
        let docroot = self
            .apps_root
            .join(application_id.as_str())
            .join("current/public");
        let webroot = self.acme_webroot.join(&name);

        if let Err(error) = std::fs::create_dir_all(webroot.join(".well-known/acme-challenge")) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }

        let site = ManagedSite {
            name: name.clone(),
            server_name: hostname.clone(),
            root: docroot,
            fpm_socket: Some(socket),
            kind: SiteKind::Laravel,
        };

        // Serve the HTTP-01 challenge on :80 first.
        let challenge = match render_http_challenge_site(&site, &webroot) {
            Ok(contents) => contents,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        if let Err(error) = self.tls_nginx.apply(&name, &challenge) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }

        let outcome = match self.certbot.obtain(&CertbotRequest {
            domains: vec![hostname.clone()],
            email,
            webroot: webroot.clone(),
            staging: self.acme_staging,
        }) {
            Ok(outcome) => outcome,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        let live = from_live_dir(&outcome.live_dir);
        let info = match self.certs.validate(&live.fullchain, &live.key, &hostname) {
            Ok(info) => info,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let dest = PathBuf::from(format!("/etc/hostnur/tls/{name}"));
        let installed = match self.certs.install(&live, &dest) {
            Ok(installed) => installed,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        let tls_site = match render_tls_site(&site, &webroot, &installed.fullchain, &installed.key)
        {
            Ok(contents) => contents,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        if let Err(error) = self.tls_nginx.apply(&name, &tls_site) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }

        let mut updated = record.clone();
        updated.state = "issued".to_string();
        let _ = self.journal.set_certificate(&updated);

        json!({"protocol_version": 1, "issued": true, "hostname": hostname, "not_after": info.not_after})
    }

    fn rotate_database(&self, request: &OperationRequest) -> Value {
        let Some(resource_id) = request
            .parameters
            .get("database")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "database must be a UUID",
            );
        };
        let record = match self.journal.database(&resource_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "database is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let password = match generate_password() {
            Ok(password) => password,
            Err(error) => return error_body(ContractErrorCode::OperationFailed, &error),
        };
        if let Err(error) = self.db.rotate(&record.username, &password) {
            return database_error(error);
        }
        if let Err(error) = self
            .db
            .check_connection(&record.db_name, &record.username, &password)
        {
            return database_error(error);
        }
        if let Err(error) =
            crate::secrets::write_secret(&self.secrets_dir, resource_id.as_str(), &password)
        {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }
        json!({"protocol_version": 1, "rotated": true, "database": record.db_name})
    }

    fn export_database(&self, request: &OperationRequest) -> Value {
        let Some(resource_id) = request
            .parameters
            .get("database")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "database must be a UUID",
            );
        };
        let record = match self.journal.database(&resource_id) {
            Ok(Some(record)) if record.state == "ready" => record,
            Ok(Some(_)) => return error_body(ContractErrorCode::Conflict, "database is not ready"),
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "database is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let password = match crate::secrets::read_secret(&self.secrets_dir, resource_id.as_str()) {
            Ok(password) => password,
            Err(_) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "credential is not available",
                );
            }
        };
        let dump = match self.db.export(&record.db_name, &record.username, &password) {
            Ok(dump) => dump,
            Err(error) => return database_error(error),
        };
        let artifact = format!("{}.sql", resource_id.as_str());
        if let Err(error) = crate::secrets::write_bytes(&self.dumps_dir, &artifact, &dump) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }
        json!({"protocol_version": 1, "exported": true, "bytes": dump.len(), "artifact": artifact})
    }

    fn import_database(&self, request: &OperationRequest) -> Value {
        let Some(resource_id) = request
            .parameters
            .get("database")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "database must be a UUID",
            );
        };
        let Some(dump) = request.parameters.get("dump").and_then(Value::as_str) else {
            return error_body(ContractErrorCode::InvalidParameters, "dump is required");
        };
        let record = match self.journal.database(&resource_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "database is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let password = match crate::secrets::read_secret(&self.secrets_dir, resource_id.as_str()) {
            Ok(password) => password,
            Err(_) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "credential is not available",
                );
            }
        };
        if let Err(error) = self.db.import(
            &record.db_name,
            &record.username,
            &password,
            dump.as_bytes(),
        ) {
            return database_error(error);
        }
        json!({"protocol_version": 1, "imported": true, "database": record.db_name})
    }

    fn schedule_name(&self, application_id: &Uuid, schedule_id: &Uuid) -> String {
        let app = crate::provision::account_name(application_id);
        let fragment = schedule_id.as_str().replace('-', "");
        format!(
            "hn-{}-sched-{}",
            app.trim_start_matches("hn_"),
            &fragment[..8]
        )
    }

    fn register_schedule(&self, request: &OperationRequest) -> Value {
        let string = |key: &str| request.parameters.get(key).and_then(Value::as_str);
        let Some(schedule_id) = string("schedule").and_then(|value| Uuid::parse(value).ok()) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "schedule must be a UUID",
            );
        };
        let Some(application_id) = string("application").and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "application must be a UUID",
            );
        };
        let Some(program) = string("program") else {
            return error_body(ContractErrorCode::InvalidParameters, "program is required");
        };
        if !crate::provider::worker::is_safe_path_token(program) {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "program must be an absolute path of safe printable characters",
            );
        }
        let Some(expression) = string("expression") else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "expression is required",
            );
        };
        let Some(timezone) = string("timezone") else {
            return error_body(ContractErrorCode::InvalidParameters, "timezone is required");
        };
        let on_calendar = match crate::cron::to_on_calendar(expression, timezone) {
            Ok(lines) => lines.join("\n"),
            Err(error) => {
                return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
            }
        };

        let arguments = string("arguments").unwrap_or("[]").to_string();
        match serde_json::from_str::<Vec<String>>(&arguments) {
            Ok(args) if args.iter().all(|arg| !arg.contains(['\n', '\r', '\0'])) => {}
            _ => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "arguments must be a JSON array of strings",
                );
            }
        }

        let record = crate::journal::ScheduleRecord {
            schedule_id: schedule_id.clone(),
            application_id,
            program: program.to_string(),
            arguments,
            on_calendar: on_calendar.to_string(),
        };
        match self.journal.set_schedule(&record) {
            Ok(()) => {
                json!({"protocol_version": 1, "registered": true, "schedule": schedule_id.as_str()})
            }
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn schedule_apply(&self, request: &OperationRequest) -> Value {
        let Some(resource) = request.resource.as_ref() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "schedule resource is required",
            );
        };
        if resource.kind != "schedule" {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "resource must be a schedule",
            );
        }
        let Ok(schedule_id) = Uuid::parse(&resource.id) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "schedule id must be a UUID",
            );
        };
        let record = match self.journal.schedule(&schedule_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "schedule is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let app = match self.journal.managed_application(&record.application_id) {
            Ok(Some(app)) => app,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::ResourceNotFound,
                    "application is not provisioned",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        let args: Vec<String> = serde_json::from_str(&record.arguments).unwrap_or_default();
        let unit = ScheduleUnit {
            name: self.schedule_name(&record.application_id, &schedule_id),
            user: app.account_name.clone(),
            group: app.account_group.clone(),
            working_dir: self
                .apps_root
                .join(record.application_id.as_str())
                .join("current"),
            program: PathBuf::from(record.program),
            args,
            on_calendar: record.on_calendar,
        };
        match self.worker.apply_schedule(&unit) {
            Ok(timer) => json!({"protocol_version": 1, "applied": true, "timer": timer}),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn remove_schedule(&self, request: &OperationRequest) -> Value {
        let Some(schedule_id) = request
            .parameters
            .get("schedule")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse(value).ok())
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "schedule must be a UUID",
            );
        };
        let record = match self.journal.schedule(&schedule_id) {
            Ok(record) => record,
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };
        if let Some(record) = &record {
            let name = self.schedule_name(&record.application_id, &schedule_id);
            if let Err(error) = self.worker.remove(&name) {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        }
        if let Err(error) = self.journal.remove_schedule(&schedule_id) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }
        json!({"protocol_version": 1, "removed": true, "schedule": schedule_id.as_str()})
    }

    /// Run a task-driven mutation as accept→execute→receipt, persisting the
    /// terminal status and result so the panel's `inspect` poll sees the outcome.
    /// Read-only/control methods (capabilities/snapshot/register_*) stay immediate.
    fn complete_operation(
        &self,
        request: &OperationRequest,
        work: impl FnOnce() -> Value,
    ) -> Value {
        let acceptance = match self.journal.accept(request) {
            Ok(acceptance) => acceptance,
            Err(error) => return error_body(ContractErrorCode::Conflict, &error.to_string()),
        };

        if !acceptance.reused {
            let outcome = work();
            let failed = outcome.get("error").is_some();
            let status = if failed {
                OperationStatus::Failed
            } else {
                OperationStatus::Succeeded
            };
            let _ = self
                .journal
                .set_status(&acceptance.execution_id, status, Some(100));
            let _ = self
                .journal
                .set_execution_result(&acceptance.execution_id, Some(&outcome.to_string()));
            let _ = self.journal.append_event(
                &acceptance.execution_id,
                if failed { "Failed." } else { "Completed." },
            );
        }

        to_value(OperationReceipt {
            protocol_version: 1,
            request_id: acceptance.request_id.clone(),
            execution_id: acceptance.execution_id.clone(),
            operation: request.operation.clone(),
            status: acceptance.status,
            accepted_at: acceptance.accepted_at.clone(),
        })
    }

    fn service_control(&self, request: &OperationRequest) -> Value {
        let Some(resource) = request.resource.as_ref() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "service resource is required",
            );
        };
        if resource.kind != "service" {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "resource must be a service",
            );
        }
        let Some(action) = request
            .parameters
            .get("action")
            .and_then(Value::as_str)
            .and_then(ServiceAction::parse)
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "action must be start, stop or restart",
            );
        };

        match self.services.control(&resource.id, action) {
            Ok(observation) => json!({
                "protocol_version": 1,
                "service": observation.name,
                "status": observation.status,
                "action": action.as_str(),
            }),
            Err(ServiceError::UnknownService(id)) => error_body(
                ContractErrorCode::PermissionDenied,
                &format!("service `{id}` is not managed by the agent"),
            ),
            Err(ServiceError::Tool(message)) => {
                error_body(ContractErrorCode::OperationFailed, &message)
            }
        }
    }

    fn package_install(&self, request: &OperationRequest) -> Value {
        let Some(id) = request.parameters.get("package").and_then(Value::as_str) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "package id is required",
            );
        };
        let outcome = if self.packages.is_approved(id) {
            self.packages.install(id)
        } else if self.package_is_registered(id) {
            self.packages.install_name(id)
        } else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "package is not in the approved catalog",
            );
        };

        match outcome {
            Ok(version) => json!({
                "protocol_version": 1,
                "installed": true,
                "package": id,
                "version": version,
            }),
            Err(PackageError::UnknownPackage(_)) => error_body(
                ContractErrorCode::InvalidParameters,
                "package is not in the approved catalog",
            ),
            Err(PackageError::ToolFailed(message)) => {
                error_body(ContractErrorCode::OperationFailed, &message)
            }
        }
    }

    fn package_is_registered(&self, id: &str) -> bool {
        self.journal
            .packages()
            .map(|packages| packages.iter().any(|package| package == id))
            .unwrap_or(false)
    }

    fn register_package(&self, request: &OperationRequest) -> Value {
        let Some(package) = request.parameters.get("package").and_then(Value::as_str) else {
            return error_body(ContractErrorCode::InvalidParameters, "package is required");
        };
        if !crate::packages::is_valid_package_name(package) {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "package name is invalid",
            );
        }
        match self.journal.set_package(package) {
            Ok(()) => json!({"protocol_version": 1, "registered": true, "package": package}),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn remove_package(&self, request: &OperationRequest) -> Value {
        let Some(package) = request.parameters.get("package").and_then(Value::as_str) else {
            return error_body(ContractErrorCode::InvalidParameters, "package is required");
        };
        match self.journal.remove_package(package) {
            Ok(()) => json!({"protocol_version": 1, "removed": true, "package": package}),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn register_backup(&self, request: &OperationRequest) -> Value {
        let string = |key: &str| request.parameters.get(key).and_then(Value::as_str);
        let Some(backup_id) = string("backup").and_then(|value| Uuid::parse(value).ok()) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "backup must be a UUID",
            );
        };
        let Some(scope) = string("scope") else {
            return error_body(ContractErrorCode::InvalidParameters, "scope is required");
        };
        if !["application_data", "control_state", "server"].contains(&scope) {
            return error_body(ContractErrorCode::InvalidParameters, "unknown backup scope");
        }
        let application_id = string("application").and_then(|value| Uuid::parse(value).ok());

        if scope == "application_data" && application_id.is_none() {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "application is required for application_data scope",
            );
        }

        let record = crate::journal::BackupRecord {
            backup_id: backup_id.clone(),
            scope: scope.to_string(),
            application_id,
        };

        match self.journal.set_backup(&record) {
            Ok(()) => {
                json!({"protocol_version": 1, "registered": true, "backup": backup_id.as_str()})
            }
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn create_backup(&self, request: &OperationRequest) -> Value {
        let Some(resource) = request.resource.as_ref() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "backup resource is required",
            );
        };
        if resource.kind != "backup" {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "resource must be a backup",
            );
        }
        let Ok(backup_id) = Uuid::parse(&resource.id) else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "backup id must be a UUID",
            );
        };

        let record = match self.journal.backup(&backup_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "backup is not registered",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        let Some(recipient) = self.backup_recipient.clone() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "no backup recipient (age public key) is configured",
            );
        };

        if let Err(error) = std::fs::create_dir_all(&self.backup_dir) {
            return error_body(ContractErrorCode::OperationFailed, &error.to_string());
        }

        match record.scope.as_str() {
            "application_data" => {
                let Some(application_id) = record.application_id else {
                    return error_body(
                        ContractErrorCode::InvalidParameters,
                        "application is required for application_data scope",
                    );
                };

                let shared = self.apps_root.join(application_id.as_str()).join("shared");
                let archive = self.backup_dir.join(format!("{backup_id}.tar.gz"));
                if let Err(message) = crate::backup::archive_directory(&shared, &archive) {
                    return error_body(ContractErrorCode::OperationFailed, &message);
                }

                let sealed_path = self.backup_dir.join(format!("{backup_id}.tar.gz.age"));
                let (sealed_bytes, hash) =
                    match crate::backup::seal_file(&archive, &sealed_path, &recipient) {
                        Ok(value) => value,
                        Err(message) => {
                            return error_body(ContractErrorCode::OperationFailed, &message);
                        }
                    };
                let archive_len = std::fs::metadata(&archive)
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                let _ = std::fs::remove_file(&archive);

                let manifest = crate::backup::manifest(
                    backup_id.as_str(),
                    "application_data",
                    &recipient,
                    "quiesced",
                    &clock::now_rfc3339(),
                    archive_len,
                    sealed_bytes,
                    &hash,
                );

                match crate::backup::write_manifest(&self.backup_dir, &manifest) {
                    Ok(path) => json!({
                        "protocol_version": 1,
                        "artifact_id": backup_id.as_str(),
                        "scope": "application_data",
                        "sealed_bytes": sealed_bytes,
                        "sha256": hash,
                        "manifest": path.display().to_string(),
                    }),
                    Err(message) => error_body(ContractErrorCode::OperationFailed, &message),
                }
            }
            other => error_body(
                ContractErrorCode::UnsupportedOperation,
                &format!("backup scope `{other}` is not supported yet"),
            ),
        }
    }

    fn package_upgrade(&self, request: &OperationRequest) -> Value {
        let security_only = request
            .parameters
            .get("security")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        match self.packages.upgrade(security_only) {
            Ok(count) => json!({
                "protocol_version": 1,
                "upgraded": count,
                "security_only": security_only,
            }),
            Err(PackageError::ToolFailed(message)) => {
                error_body(ContractErrorCode::OperationFailed, &message)
            }
            Err(PackageError::UnknownPackage(message)) => {
                error_body(ContractErrorCode::InvalidParameters, &message)
            }
        }
    }

    fn firewall_rule_add(&self, request: &OperationRequest) -> Value {
        let Some(port) = request.parameters.get("port").and_then(Value::as_i64) else {
            return error_body(ContractErrorCode::InvalidParameters, "port is required");
        };
        let protocol = request
            .parameters
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source = request.parameters.get("source").and_then(Value::as_str);
        let Some(action) = request.parameters.get("action").and_then(Value::as_str) else {
            return error_body(ContractErrorCode::InvalidParameters, "action is required");
        };

        match self.firewall.add_rule(port, protocol, source, action) {
            Ok(()) => json!({
                "protocol_version": 1,
                "applied": true,
                "port": port,
                "protocol": protocol,
                "action": action,
            }),
            Err(error) => firewall_error(error),
        }
    }

    fn firewall_rule_remove(&self, request: &OperationRequest) -> Value {
        let Some(port) = request.parameters.get("port").and_then(Value::as_i64) else {
            return error_body(ContractErrorCode::InvalidParameters, "port is required");
        };
        let protocol = request
            .parameters
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source = request.parameters.get("source").and_then(Value::as_str);
        let Some(action) = request.parameters.get("action").and_then(Value::as_str) else {
            return error_body(ContractErrorCode::InvalidParameters, "action is required");
        };

        match self.firewall.remove_rule(port, protocol, source, action) {
            Ok(()) => json!({
                "protocol_version": 1,
                "applied": true,
                "removed": true,
                "port": port,
                "protocol": protocol,
            }),
            Err(error) => firewall_error(error),
        }
    }

    fn firewall_web(&self, request: &OperationRequest) -> Value {
        let Some(https_only) = request
            .parameters
            .get("https_only")
            .and_then(Value::as_bool)
        else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "https_only is required",
            );
        };

        match self.firewall.web_https_only(https_only) {
            Ok(()) => json!({
                "protocol_version": 1,
                "applied": true,
                "https_only": https_only,
            }),
            Err(error) => firewall_error(error),
        }
    }

    fn application_logs(&self, request: &OperationRequest) -> Value {
        let application_id = match application_resource(request) {
            Ok(id) => id,
            Err(message) => return error_body(ContractErrorCode::InvalidParameters, message),
        };

        let (lines, truncated) =
            crate::appstate::read_logs(&self.apps_root, &application_id).unwrap_or_default();

        json!({
            "protocol_version": 1,
            "application_id": application_id.as_str(),
            "lines": lines,
            "truncated": truncated,
            "sampled_at": clock::now_rfc3339(),
        })
    }

    fn application_metrics(&self, request: &OperationRequest) -> Value {
        let application_id = match application_resource(request) {
            Ok(id) => id,
            Err(message) => return error_body(ContractErrorCode::InvalidParameters, message),
        };

        let storage = crate::appstate::storage(&self.apps_root, &application_id);

        json!({
            "protocol_version": 1,
            "application_id": application_id.as_str(),
            "storage_used_bytes": storage.map(|(used, _)| used),
            "storage_total_bytes": storage.map(|(_, total)| total),
            "sampled_at": clock::now_rfc3339(),
        })
    }

    fn application_observe(&self, request: &OperationRequest) -> Value {
        let application_id = match application_resource(request) {
            Ok(id) => id,
            Err(message) => return error_body(ContractErrorCode::InvalidParameters, message),
        };

        let root = self.apps_root.join(application_id.as_str());
        let current = root.join("current");

        let (lifecycle, active_release) = match std::fs::read_link(&current) {
            Ok(target) => {
                let release = target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| Uuid::parse(name).ok())
                    .map(|uuid| uuid.as_str().to_string());
                ("ready", release)
            }
            Err(_) if root.join("releases").is_dir() => ("layout_prepared", None),
            Err(_) if root.is_dir() => ("account_created", None),
            Err(_) => ("reserved", None),
        };

        json!({
            "protocol_version": 1,
            "application_id": application_id.as_str(),
            "lifecycle": lifecycle,
            "active_release": active_release,
            "mode": "real",
            "sampled_at": clock::now_rfc3339(),
        })
    }

    fn application_control(&self, request: &OperationRequest, action: &str) -> Value {
        let application_id = match application_resource(request) {
            Ok(id) => id,
            Err(message) => return error_body(ContractErrorCode::InvalidParameters, message),
        };

        let fragment = crate::provision::account_name(&application_id);
        let fragment = fragment.trim_start_matches("hn_").to_string();

        match crate::appstate::control_units(&fragment, action) {
            Ok(units) => json!({
                "protocol_version": 1,
                "applied": true,
                "action": action,
                "units": units,
            }),
            Err(message) => error_body(ContractErrorCode::OperationFailed, &message),
        }
    }

    fn application_restart(&self, request: &OperationRequest) -> Value {
        let application_id = match application_resource(request) {
            Ok(id) => id,
            Err(message) => return error_body(ContractErrorCode::InvalidParameters, message),
        };

        let source = match self.journal.application_source(&application_id) {
            Ok(Some(source)) => source,
            Ok(None) => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "application is not registered; deploy it first",
                );
            }
            Err(error) => {
                return error_body(ContractErrorCode::OperationFailed, &error.to_string());
            }
        };

        let version = if source.runtime_version.is_empty() {
            "8.3".to_string()
        } else {
            source.runtime_version.clone()
        };

        if let Err(message) = reload_runtime(&version) {
            return error_body(ContractErrorCode::OperationFailed, &message);
        }

        json!({
            "protocol_version": 1,
            "restarted": true,
            "php": version,
        })
    }

    fn application_start(&self, request: &OperationRequest) -> Value {
        self.application_control(request, "start")
    }

    fn application_stop(&self, request: &OperationRequest) -> Value {
        self.application_control(request, "stop")
    }

    fn deploy_release(&self, request: &OperationRequest) -> Value {
        let Some(runtime) = self.deploy.as_ref() else {
            return error_body(
                ContractErrorCode::UnsupportedOperation,
                "deployment is not enabled in this agent build",
            );
        };
        let Some(resource) = request.resource.as_ref() else {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "application resource is required",
            );
        };
        if resource.kind != "application" {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "resource must be an application",
            );
        }

        let application_id = match Uuid::parse(&resource.id) {
            Ok(id) => id,
            Err(_) => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "application resource id must be a UUID",
                );
            }
        };

        let git_ref = match request.parameters.get("ref").and_then(Value::as_str) {
            Some(value) => match validate_ref(value) {
                Ok(reference) => reference,
                Err(error) => {
                    return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
                }
            },
            None => GitRef::Name("main".to_string()),
        };
        let run_migrations = request
            .parameters
            .get("run_migrations")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let Some(ticket) = DeployTicket::try_acquire() else {
            return error_body(
                ContractErrorCode::AgentUnavailable,
                "the deployment concurrency limit is reached; retry shortly",
            );
        };

        let acceptance = match self.journal.accept(request) {
            Ok(acceptance) => acceptance,
            Err(error) => return error_body(ContractErrorCode::Conflict, &error.to_string()),
        };

        if !acceptance.reused {
            let runtime = runtime.clone();
            let execution_id = acceptance.execution_id.clone();
            std::thread::spawn(move || {
                // The ticket is returned when the worker finishes or panics.
                let _ticket = ticket;
                runtime.run_worker(execution_id, application_id, git_ref, run_migrations);
            });
        }

        to_value(OperationReceipt {
            protocol_version: 1,
            request_id: acceptance.request_id.clone(),
            execution_id: acceptance.execution_id.clone(),
            operation: request.operation.clone(),
            status: acceptance.status,
            accepted_at: acceptance.accepted_at.clone(),
        })
    }

    fn capabilities(&self) -> Value {
        let mut operations: Vec<String> = SUPPORTED_OPERATIONS
            .iter()
            .map(|op| (*op).to_string())
            .collect();
        if self.deploy.is_some() {
            operations.push("application.deploy".to_string());
        }
        to_value(CapabilitySnapshot {
            protocol_version: 1,
            mode: AgentMode::Real,
            available: true,
            operations,
            sampled_at: clock::now_rfc3339(),
            reason: None,
        })
    }

    fn server_snapshot(&self) -> Value {
        let mut snapshot = observe::server_snapshot();
        snapshot.services = self.services.observations();
        snapshot.firewall = self.firewall.snapshot();
        to_value(snapshot)
    }

    fn server_updates(&self) -> Value {
        let (updates, truncated) = self.packages.list_updates();

        let entries: Vec<Value> = updates
            .into_iter()
            .map(|(package, current, candidate, security)| {
                json!({
                    "package": package,
                    "current": current,
                    "candidate": candidate,
                    "security": security,
                })
            })
            .collect();

        json!({
            "protocol_version": 1,
            "updates": entries,
            "truncated": truncated,
            "sampled_at": clock::now_rfc3339(),
        })
    }

    fn register_application(&self, request: &OperationRequest) -> Value {
        let parameter = |key: &str| request.parameters.get(key).and_then(Value::as_str);
        let application = match parameter("application").and_then(|value| Uuid::parse(value).ok()) {
            Some(id) => id,
            None => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "application must be a UUID",
                );
            }
        };
        let repository_url = match parameter("repository_url") {
            Some(value) => value,
            None => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "repository_url is required",
                );
            }
        };
        // Validate then persist the normalized, credential-free, trimmed URL so
        // the stored value cannot differ from the validated one by whitespace,
        // case or a legacy numeric host form.
        let repository_url =
            match validate_repository_url(repository_url, &NetworkPolicy::default()) {
                Ok(normalized) => normalized.url,
                Err(error) => {
                    return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
                }
            };
        let server_name = match parameter("server_name") {
            Some(value) => value,
            None => {
                return error_body(
                    ContractErrorCode::InvalidParameters,
                    "server_name is required",
                );
            }
        };
        if let Err(error) = validate_hostname(server_name) {
            return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
        }
        let provider = match parameter("provider") {
            Some(value) => value,
            None => {
                return error_body(ContractErrorCode::InvalidParameters, "provider is required");
            }
        };
        let runtime = parameter("runtime").unwrap_or("php");
        let runtime_version = parameter("runtime_version").unwrap_or("");
        if let Err(error) = self.policy.validate(runtime, runtime_version) {
            return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
        }

        let source = ApplicationSource {
            application_id: application.clone(),
            repository_url,
            server_name: server_name.to_string(),
            provider: provider.to_string(),
            runtime: runtime.to_string(),
            runtime_version: runtime_version.to_string(),
        };
        match self.journal.set_application_source(&source) {
            Ok(()) => json!({
                "protocol_version": 1,
                "registered": true,
                "application": application.as_str(),
            }),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn inspect(&self, request: &OperationRequest) -> Value {
        let execution_id = match execution_id(request) {
            Ok(id) => id,
            Err(error) => return error_body(error.0, error.1),
        };

        match self.journal.inspect(&execution_id) {
            Ok(Some(view)) => {
                let result = view
                    .result
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
                to_value(OperationSnapshot {
                    protocol_version: 1,
                    execution_id: view.execution_id,
                    request_id: view.request_id,
                    operation: view.operation,
                    status: view.status,
                    progress: view.progress,
                    events: view.events,
                    result,
                    error: None,
                    updated_at: view.updated_at,
                })
            }
            Ok(None) => error_body(
                ContractErrorCode::ExecutionUnknown,
                "the execution is not known to the agent",
            ),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }

    fn cancel(&self, request: &OperationRequest) -> Value {
        let execution_id = match execution_id(request) {
            Ok(id) => id,
            Err(error) => return error_body(error.0, error.1),
        };

        match self.journal.inspect(&execution_id) {
            Ok(Some(view)) => {
                if view.status.is_terminal() {
                    return json!({
                        "protocol_version": 1,
                        "execution_id": execution_id.as_str(),
                        "status": "already_finished",
                        "requested_at": clock::now_rfc3339(),
                    });
                }
                if let Err(error) = self.journal.set_status(
                    &execution_id,
                    OperationStatus::Cancelled,
                    view.progress,
                ) {
                    return error_body(ContractErrorCode::OperationFailed, &error.to_string());
                }
                let _ = self
                    .journal
                    .append_event(&execution_id, "Cancellation requested by the panel.");
                json!({
                    "protocol_version": 1,
                    "execution_id": execution_id.as_str(),
                    "status": "requested",
                    "requested_at": clock::now_rfc3339(),
                })
            }
            Ok(None) => error_body(
                ContractErrorCode::ExecutionUnknown,
                "the execution is not known to the agent",
            ),
            Err(error) => error_body(ContractErrorCode::OperationFailed, &error.to_string()),
        }
    }
}

impl RequestHandler for AgentHandler<'_> {
    fn handle(&self, request: &OperationRequest) -> Option<Value> {
        Some(match request.operation.as_str() {
            "agent.capabilities" => self.capabilities(),
            "server.snapshot" => self.server_snapshot(),
            "server.updates" => self.server_updates(),
            "agent.inspect" => self.inspect(request),
            "agent.cancel" => self.cancel(request),
            "agent.register_application" => self.register_application(request),
            "application.deploy" => self.deploy_release(request),
            "service.control" => self.complete_operation(request, || self.service_control(request)),
            "package.install" => self.complete_operation(request, || self.package_install(request)),
            "package.upgrade" => self.complete_operation(request, || self.package_upgrade(request)),
            "agent.register_package" => self.register_package(request),
            "agent.remove_package" => self.remove_package(request),
            "agent.register_backup" => self.register_backup(request),
            "backup.create" => self.complete_operation(request, || self.create_backup(request)),
            "firewall.rule.add" => {
                self.complete_operation(request, || self.firewall_rule_add(request))
            }
            "firewall.rule.remove" => {
                self.complete_operation(request, || self.firewall_rule_remove(request))
            }
            "firewall.web" => self.complete_operation(request, || self.firewall_web(request)),
            "application.logs" => self.application_logs(request),
            "application.metrics" => self.application_metrics(request),
            "application.observe" => self.application_observe(request),
            "application.start" => {
                self.complete_operation(request, || self.application_start(request))
            }
            "application.stop" => {
                self.complete_operation(request, || self.application_stop(request))
            }
            "application.restart" => {
                self.complete_operation(request, || self.application_restart(request))
            }
            "agent.register_database" => self.register_database(request),
            "database.provision" => {
                self.complete_operation(request, || self.database_provision(request))
            }
            "database.rotate" => self.complete_operation(request, || self.rotate_database(request)),
            "database.export" => self.export_database(request),
            "database.import" => self.complete_operation(request, || self.import_database(request)),
            "agent.remove_database" => self.remove_database(request),
            "agent.register_schedule" => self.register_schedule(request),
            "agent.remove_schedule" => self.remove_schedule(request),
            "schedule.apply" => self.complete_operation(request, || self.schedule_apply(request)),
            "agent.register_certificate" => self.register_certificate(request),
            "certificate.issue" => {
                self.complete_operation(request, || self.certificate_issue(request))
            }
            _ => error_body(
                ContractErrorCode::UnsupportedOperation,
                "the operation is not implemented by this agent build",
            ),
        })
    }
}

/// Build a receipt from a journal acceptance (used by future submit wiring).
pub fn receipt_from_acceptance(acceptance: &crate::journal::Acceptance) -> OperationReceipt {
    OperationReceipt {
        protocol_version: 1,
        request_id: acceptance.request_id.clone(),
        execution_id: acceptance.execution_id.clone(),
        operation: acceptance.operation.clone(),
        status: acceptance.status,
        accepted_at: acceptance.accepted_at.clone(),
    }
}

fn execution_id(request: &OperationRequest) -> Result<Uuid, (ContractErrorCode, &'static str)> {
    match request
        .parameters
        .get("execution_id")
        .and_then(Value::as_str)
    {
        Some(value) => Uuid::parse(value).map_err(|_| {
            (
                ContractErrorCode::InvalidParameters,
                "execution_id must be a UUID",
            )
        }),
        None => Err((
            ContractErrorCode::InvalidParameters,
            "execution_id is required",
        )),
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| {
        error_body(
            ContractErrorCode::OperationFailed,
            "the agent could not encode the response",
        )
    })
}

fn database_error(error: DatabaseError) -> Value {
    let code = match error {
        DatabaseError::Policy(_) => ContractErrorCode::InvalidParameters,
        DatabaseError::Tool(_) | DatabaseError::Conflict(_) => ContractErrorCode::OperationFailed,
    };
    error_body(code, &error.to_string())
}

fn application_resource(request: &OperationRequest) -> Result<Uuid, &'static str> {
    let Some(resource) = request.resource.as_ref() else {
        return Err("application resource is required");
    };
    if resource.kind != "application" {
        return Err("resource must be an application");
    }
    Uuid::parse(&resource.id).map_err(|_| "application id must be a UUID")
}

/// Reload the application's PHP-FPM service and Nginx after a restart request.
fn reload_runtime(php_version: &str) -> Result<(), String> {
    let fpm = format!("php{php_version}-fpm");
    run_bounded(Path::new("/usr/bin/systemctl"), &["reload", &fpm])
        .map_err(|error| format!("php-fpm reload failed: {error}"))?;
    run_bounded(Path::new("/usr/sbin/nginx"), &["-s", "reload"])
        .map_err(|error| format!("nginx reload failed: {error}"))?;
    Ok(())
}

/// Run a fixed command through the bounded process primitive.
fn run_bounded(program: &Path, args: &[&str]) -> Result<(), String> {
    let mut spec = CommandSpec::new(program, Duration::from_secs(30));
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.output_cap_bytes = 32_768;
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );
    env.insert("LC_ALL".to_string(), "C".to_string());
    spec.env = env;
    let outcome = run(&spec).map_err(|error| error.to_string())?;
    if outcome.succeeded() {
        Ok(())
    } else {
        Err(format!("exit code {:?}", outcome.exit_code))
    }
}

fn firewall_error(error: FirewallError) -> Value {
    let code = match error {
        FirewallError::ProtectedPort => ContractErrorCode::PermissionDenied,
        FirewallError::HttpsRequired => ContractErrorCode::Conflict,
        FirewallError::InvalidInput(_) => ContractErrorCode::InvalidParameters,
        FirewallError::Tool(_) => ContractErrorCode::OperationFailed,
    };
    error_body(code, &error.to_string())
}

fn error_body(code: ContractErrorCode, message: &str) -> Value {
    json!({
        "error": {
            "code": code.as_str(),
            "message": message,
            "retryable": false,
        }
    })
}
