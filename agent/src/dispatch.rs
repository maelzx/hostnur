//! Request dispatch: maps a validated operation to a response body.
//!
//! Only a narrow, tested set is supported in this phase. Control operations
//! (`agent.capabilities`, `agent.inspect`, `agent.cancel`) back the panel's
//! read-only agent-client methods; `server.snapshot` is read-only. Mutation
//! operations are cataloged but not yet implemented, so they return a safe
//! `unsupported_operation` error rather than pretending to run.

use serde_json::{Value, json};

use crate::clock;
use crate::contract::{
    AgentMode, CapabilitySnapshot, ContractErrorCode, OperationReceipt, OperationRequest,
    OperationSnapshot, OperationStatus, Uuid,
};
use crate::deploy::DeployRuntime;
use crate::git::policy::{NetworkPolicy, validate_repository_url};
use crate::git::{GitRef, validate_ref};
use crate::journal::{ApplicationSource, Journal};
use crate::observe;
use crate::provider::RuntimePolicy;
use crate::provider::policy::validate_hostname;
use crate::transport::RequestHandler;

/// Operations this build actually advertises and can serve.
pub const SUPPORTED_OPERATIONS: [&str; 5] = [
    "agent.capabilities",
    "agent.inspect",
    "agent.cancel",
    "agent.register_application",
    "server.snapshot",
];

/// Handler backed by the durable journal.
pub struct AgentHandler<'a> {
    journal: &'a Journal,
    deploy: Option<DeployRuntime>,
    policy: RuntimePolicy,
}

impl<'a> AgentHandler<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self {
            journal,
            deploy: None,
            policy: RuntimePolicy::default(),
        }
    }

    /// Handler with a privileged deployment runtime and runtime policy enabled.
    pub fn with_deploy(journal: &'a Journal, deploy: DeployRuntime, policy: RuntimePolicy) -> Self {
        Self {
            journal,
            deploy: Some(deploy),
            policy,
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
            let application_id = resource.id.clone();
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
        to_value(observe::server_snapshot())
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

fn error_body(code: ContractErrorCode, message: &str) -> Value {
    json!({
        "error": {
            "code": code.as_str(),
            "message": message,
            "retryable": false,
        }
    })
}
