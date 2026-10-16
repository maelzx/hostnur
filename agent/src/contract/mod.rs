//! Cross-language contract (`contracts/v1`) implemented in Rust.
//!
//! This module mirrors the PHP panel's request envelope and response DTOs, the
//! framing used on the local transport, and the embedded operation catalog. It
//! is the agent's own trust boundary: every field is re-validated here rather
//! than trusting a caller-supplied value.

pub mod catalog;
pub mod dto;
pub mod error;
pub mod framing;
pub mod request;

pub use catalog::{Catalog, OperationDefinition, ParameterRule};
pub use dto::{
    AgentError, AgentMode, CancellationReceipt, CancellationStatus, CapabilitySnapshot,
    FirewallRule, FirewallSummary, Metric, OperationReceipt, OperationSnapshot, OperationStatus,
    ServerSnapshot, ServiceObservation, SnapshotEvent, Uuid, is_uuid,
};
pub use error::{ContractError, ContractErrorCode};
pub use framing::{FrameError, MAX_MESSAGE_BYTES, encode_frame, read_frame};
pub use request::{OperationRequest, ResourceRef, parse_request, parse_request_str};
