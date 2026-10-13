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
    OperationSnapshot, OperationStatus, ServerSnapshot, Uuid,
};
use crate::journal::Journal;
use crate::transport::RequestHandler;

/// Operations this build actually advertises and can serve.
pub const SUPPORTED_OPERATIONS: [&str; 4] = [
    "agent.capabilities",
    "agent.inspect",
    "agent.cancel",
    "server.snapshot",
];

/// Handler backed by the durable journal.
pub struct AgentHandler<'a> {
    journal: &'a Journal,
}

impl<'a> AgentHandler<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self { journal }
    }

    fn capabilities(&self) -> Value {
        to_value(CapabilitySnapshot {
            protocol_version: 1,
            mode: AgentMode::Real,
            available: true,
            operations: SUPPORTED_OPERATIONS
                .iter()
                .map(|op| (*op).to_string())
                .collect(),
            sampled_at: clock::now_rfc3339(),
            reason: None,
        })
    }

    fn server_snapshot(&self) -> Value {
        to_value(ServerSnapshot {
            protocol_version: 1,
            mode: AgentMode::Real,
            available: true,
            sampled_at: clock::now_rfc3339(),
            metrics: Vec::new(),
            services: Vec::new(),
            reason: None,
            hostname: None,
            os: None,
            firewall: None,
        })
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
