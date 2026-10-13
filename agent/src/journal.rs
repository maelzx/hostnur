//! Agent-owned durable execution journal and managed-resource registry.
//!
//! This is separate from the panel's SQLite database. It records acceptance
//! before any mutation, deduplicates intents by idempotency key (same key +
//! same intent reuses one execution; same key + different intent is a
//! conflict), and keeps a bounded, ordered event history per execution so state
//! survives worker/client reconnects and an agent restart.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::clock;
use crate::contract::{OperationRequest, OperationStatus, SnapshotEvent, Uuid};
use crate::ids;

/// Schema version recorded in `schema_meta`.
pub const SCHEMA_VERSION: &str = "1";

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS executions (
    execution_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    operation TEXT NOT NULL,
    payload_canonical TEXT NOT NULL,
    resource_kind TEXT,
    resource_id TEXT,
    status TEXT NOT NULL,
    progress INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_events (
    execution_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    at TEXT NOT NULL,
    message TEXT NOT NULL,
    PRIMARY KEY (execution_id, sequence),
    FOREIGN KEY (execution_id) REFERENCES executions(execution_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS resources (
    resource_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    provider TEXT,
    owner_uid INTEGER,
    owner_gid INTEGER,
    revision TEXT,
    lifecycle_state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

/// Journal failures.
#[derive(Debug)]
pub enum JournalError {
    /// Opening or migrating the database failed.
    Open(String),
    /// A query or write failed.
    Query(String),
    /// The same idempotency key was reused with a different intent.
    Conflict,
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournalError::Open(message) => write!(f, "journal open: {message}"),
            JournalError::Query(message) => write!(f, "journal query: {message}"),
            JournalError::Conflict => f.write_str("idempotency key reused with a different intent"),
        }
    }
}

impl std::error::Error for JournalError {}

/// Result of accepting an intent.
#[derive(Debug, Clone, PartialEq)]
pub struct Acceptance {
    pub execution_id: Uuid,
    pub request_id: Uuid,
    pub operation: String,
    pub status: OperationStatus,
    pub accepted_at: String,
    /// True when an existing execution was reused for a duplicate intent.
    pub reused: bool,
}

/// Durable view of an execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionView {
    pub execution_id: Uuid,
    pub request_id: Uuid,
    pub operation: String,
    pub status: OperationStatus,
    pub progress: Option<i64>,
    pub events: Vec<SnapshotEvent>,
    pub created_at: String,
    pub updated_at: String,
}

/// A managed resource owned by the agent registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredResource {
    pub resource_id: Uuid,
    pub kind: String,
    pub provider: Option<String>,
    pub owner_uid: Option<u32>,
    pub owner_gid: Option<u32>,
    pub revision: Option<String>,
    pub lifecycle_state: String,
}

/// SQLite-backed journal. One connection; the transport is single-threaded.
pub struct Journal {
    connection: Connection,
}

impl Journal {
    /// Open (creating if needed) the journal at `path`.
    pub fn open(path: &Path) -> Result<Journal, JournalError> {
        let connection =
            Connection::open(path).map_err(|err| JournalError::Open(err.to_string()))?;
        Self::init(connection)
    }

    /// Open an in-memory journal (tests only).
    pub fn open_in_memory() -> Result<Journal, JournalError> {
        let connection =
            Connection::open_in_memory().map_err(|err| JournalError::Open(err.to_string()))?;
        Self::init(connection)
    }

    fn init(connection: Connection) -> Result<Journal, JournalError> {
        let _ = connection.pragma_update(None, "journal_mode", "WAL");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|err| JournalError::Open(err.to_string()))?;
        connection
            .execute_batch(SCHEMA)
            .map_err(|err| JournalError::Open(err.to_string()))?;
        connection
            .execute(
                "INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('schema_version', ?1)",
                params![SCHEMA_VERSION],
            )
            .map_err(|err| JournalError::Open(err.to_string()))?;
        Ok(Journal { connection })
    }

    /// Accept an intent, deduplicating on its idempotency key.
    ///
    /// Persists acceptance before returning; the caller may then start work.
    pub fn accept(&self, request: &OperationRequest) -> Result<Acceptance, JournalError> {
        let canonical = canonical_intent(request)?;
        let now = clock::now_rfc3339();
        let execution_id = ids::new_uuid_v4();

        // INSERT OR IGNORE means a concurrent/duplicate insertion loses cleanly;
        // the subsequent SELECT returns the single winning row either way.
        self.connection
            .execute(
                "INSERT OR IGNORE INTO executions (
                    execution_id, request_id, idempotency_key, operation,
                    payload_canonical, resource_kind, resource_id, status,
                    progress, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'accepted', NULL, ?8, ?8)",
                params![
                    execution_id.as_str(),
                    request.request_id.as_str(),
                    request.idempotency_key.as_str(),
                    request.operation,
                    canonical,
                    request
                        .resource
                        .as_ref()
                        .map(|resource| resource.kind.as_str()),
                    request
                        .resource
                        .as_ref()
                        .map(|resource| resource.id.as_str()),
                    now,
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        let row = self
            .connection
            .query_row(
                "SELECT execution_id, request_id, operation, payload_canonical, status, created_at
                 FROM executions WHERE idempotency_key = ?1",
                params![request.idempotency_key.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        if row.3 != canonical {
            return Err(JournalError::Conflict);
        }

        Ok(Acceptance {
            execution_id: Uuid::parse(&row.0).map_err(query_err)?,
            request_id: Uuid::parse(&row.1).map_err(query_err)?,
            operation: row.2,
            status: parse_status(&row.4)?,
            accepted_at: row.5,
            reused: row.0 != execution_id.as_str(),
        })
    }

    /// Fetch an execution and its ordered events.
    pub fn inspect(&self, execution_id: &Uuid) -> Result<Option<ExecutionView>, JournalError> {
        let base = self
            .connection
            .query_row(
                "SELECT execution_id, request_id, operation, status, progress, created_at, updated_at
                 FROM executions WHERE execution_id = ?1",
                params![execution_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))?;

        let Some(base) = base else {
            return Ok(None);
        };

        Ok(Some(ExecutionView {
            execution_id: Uuid::parse(&base.0).map_err(query_err)?,
            request_id: Uuid::parse(&base.1).map_err(query_err)?,
            operation: base.2,
            status: parse_status(&base.3)?,
            progress: base.4,
            events: self.events_after(execution_id, 0)?,
            created_at: base.5,
            updated_at: base.6,
        }))
    }

    /// Append an event, returning it with its assigned sequence.
    pub fn append_event(
        &self,
        execution_id: &Uuid,
        message: &str,
    ) -> Result<SnapshotEvent, JournalError> {
        let next: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM execution_events WHERE execution_id = ?1",
                params![execution_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        let at = clock::now_rfc3339();
        self.connection
            .execute(
                "INSERT INTO execution_events (execution_id, sequence, at, message)
                 VALUES (?1, ?2, ?3, ?4)",
                params![execution_id.as_str(), next, at, message],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        self.connection
            .execute(
                "UPDATE executions SET updated_at = ?2 WHERE execution_id = ?1",
                params![execution_id.as_str(), at],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        Ok(SnapshotEvent {
            sequence: next,
            at,
            message: message.to_string(),
        })
    }

    /// Events with a sequence greater than `cursor` (bounded by `limit`).
    pub fn events_after(
        &self,
        execution_id: &Uuid,
        cursor: i64,
    ) -> Result<Vec<SnapshotEvent>, JournalError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT sequence, at, message FROM execution_events
                 WHERE execution_id = ?1 AND sequence > ?2
                 ORDER BY sequence ASC LIMIT 100",
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;

        let rows = statement
            .query_map(params![execution_id.as_str(), cursor], |row| {
                Ok(SnapshotEvent {
                    sequence: row.get(0)?,
                    at: row.get(1)?,
                    message: row.get(2)?,
                })
            })
            .map_err(|err| JournalError::Query(err.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|err| JournalError::Query(err.to_string()))?);
        }
        Ok(events)
    }

    /// Update the status (and optional progress) of an execution.
    pub fn set_status(
        &self,
        execution_id: &Uuid,
        status: OperationStatus,
        progress: Option<i64>,
    ) -> Result<(), JournalError> {
        self.connection
            .execute(
                "UPDATE executions SET status = ?2, progress = ?3, updated_at = ?4
                 WHERE execution_id = ?1",
                params![
                    execution_id.as_str(),
                    status_str(status),
                    progress,
                    clock::now_rfc3339(),
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    /// Insert a managed resource if it is not already registered.
    ///
    /// Returns `true` when a new row was created, `false` when the id already
    /// existed (an existing identity is never silently adopted or overwritten).
    pub fn register_resource(&self, resource: &RegisteredResource) -> Result<bool, JournalError> {
        let now = clock::now_rfc3339();
        let changed = self
            .connection
            .execute(
                "INSERT OR IGNORE INTO resources (
                    resource_id, kind, provider, owner_uid, owner_gid, revision,
                    lifecycle_state, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    resource.resource_id.as_str(),
                    resource.kind,
                    resource.provider,
                    resource.owner_uid,
                    resource.owner_gid,
                    resource.revision,
                    resource.lifecycle_state,
                    now,
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(changed == 1)
    }

    /// Look up a registered resource by its agent-owned UUID.
    pub fn resource(&self, resource_id: &Uuid) -> Result<Option<RegisteredResource>, JournalError> {
        self.connection
            .query_row(
                "SELECT resource_id, kind, provider, owner_uid, owner_gid, revision, lifecycle_state
                 FROM resources WHERE resource_id = ?1",
                params![resource_id.as_str()],
                |row| {
                    Ok(RegisteredResource {
                        resource_id: Uuid::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        kind: row.get(1)?,
                        provider: row.get(2)?,
                        owner_uid: row.get(3)?,
                        owner_gid: row.get(4)?,
                        revision: row.get(5)?,
                        lifecycle_state: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }
}

/// Canonical, order-stable representation of an intent for dedupe comparison.
fn canonical_intent(request: &OperationRequest) -> Result<String, JournalError> {
    let intent = serde_json::json!({
        "operation": request.operation,
        "resource": request.resource,
        "parameters": request.parameters,
    });
    serde_json::to_string(&intent).map_err(|err| JournalError::Query(err.to_string()))
}

fn parse_status(raw: &str) -> Result<OperationStatus, JournalError> {
    match raw {
        "accepted" => Ok(OperationStatus::Accepted),
        "running" => Ok(OperationStatus::Running),
        "succeeded" => Ok(OperationStatus::Succeeded),
        "failed" => Ok(OperationStatus::Failed),
        "cancelled" => Ok(OperationStatus::Cancelled),
        "timed_out" => Ok(OperationStatus::TimedOut),
        other => Err(JournalError::Query(format!("unknown status `{other}`"))),
    }
}

const fn status_str(status: OperationStatus) -> &'static str {
    match status {
        OperationStatus::Accepted => "accepted",
        OperationStatus::Running => "running",
        OperationStatus::Succeeded => "succeeded",
        OperationStatus::Failed => "failed",
        OperationStatus::Cancelled => "cancelled",
        OperationStatus::TimedOut => "timed_out",
    }
}

fn query_err(error: crate::contract::ContractError) -> JournalError {
    JournalError::Query(error.to_string())
}
