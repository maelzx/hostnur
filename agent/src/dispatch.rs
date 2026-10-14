//! Request dispatch: maps a validated operation to a response body.
//!
//! Only a narrow, tested set is supported in this phase. Control operations
//! (`agent.capabilities`, `agent.inspect`, `agent.cancel`) back the panel's
//! read-only agent-client methods; `server.snapshot` is read-only. Mutation
//! operations are cataloged but not yet implemented, so they return a safe
//! `unsupported_operation` error rather than pretending to run.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

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
use crate::firewall::FirewallPolicy;
use crate::git::policy::{NetworkPolicy, validate_repository_url};
use crate::git::{GitRef, validate_ref};
use crate::journal::{ApplicationSource, DatabaseResource, Journal};
use crate::observe;
use crate::packages::{PackageError, PackagePolicy};
use crate::provider::policy::validate_hostname;
use crate::provider::{RuntimePolicy, ScheduleUnit, WorkerManager};
use crate::services::{ServiceAction, ServiceError, ServicePolicy};
use crate::transport::RequestHandler;

/// Operations this build actually advertises and can serve.
pub const SUPPORTED_OPERATIONS: [&str; 9] = [
    "agent.capabilities",
    "agent.inspect",
    "agent.cancel",
    "agent.register_application",
    "server.snapshot",
    "service.control",
    "package.install",
    "schedule.apply",
    "database.provision",
];

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
        if !Path::new(program).is_absolute() || program.len() > 255 {
            return error_body(
                ContractErrorCode::InvalidParameters,
                "program must be an absolute path",
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
        match self.packages.install(id) {
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

        let acceptance = match self.journal.accept(request) {
            Ok(acceptance) => acceptance,
            Err(error) => return error_body(ContractErrorCode::Conflict, &error.to_string()),
        };

        if !acceptance.reused {
            let runtime = runtime.clone();
            let execution_id = acceptance.execution_id.clone();
            std::thread::spawn(move || {
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
        if let Err(error) = validate_repository_url(repository_url, &NetworkPolicy::default()) {
            return error_body(ContractErrorCode::InvalidParameters, &error.to_string());
        }
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
            repository_url: repository_url.to_string(),
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
            Ok(Some(view)) => to_value(OperationSnapshot {
                protocol_version: 1,
                execution_id: view.execution_id,
                request_id: view.request_id,
                operation: view.operation,
                status: view.status,
                progress: view.progress,
                events: view.events,
                result: None,
                error: None,
                updated_at: view.updated_at,
            }),
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
            "agent.inspect" => self.inspect(request),
            "agent.cancel" => self.cancel(request),
            "agent.register_application" => self.register_application(request),
            "application.deploy" => self.deploy_release(request),
            "service.control" => self.service_control(request),
            "package.install" => self.package_install(request),
            "agent.register_database" => self.register_database(request),
            "database.provision" => self.database_provision(request),
            "agent.remove_database" => self.remove_database(request),
            "agent.register_schedule" => self.register_schedule(request),
            "agent.remove_schedule" => self.remove_schedule(request),
            "schedule.apply" => self.schedule_apply(request),
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

fn error_body(code: ContractErrorCode, message: &str) -> Value {
    json!({
        "error": {
            "code": code.as_str(),
            "message": message,
            "retryable": false,
        }
    })
}
