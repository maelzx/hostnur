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
pub const SCHEMA_VERSION: &str = "6";

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

CREATE TABLE IF NOT EXISTS deployments (
    release_id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL,
    commit_id TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS databases (
    resource_id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL,
    engine TEXT NOT NULL,
    db_name TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL UNIQUE,
    host TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schedules (
    schedule_id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL,
    program TEXT NOT NULL,
    arguments TEXT NOT NULL,
    on_calendar TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS application_sources (
    application_id TEXT PRIMARY KEY,
    repository_url TEXT NOT NULL,
    server_name TEXT NOT NULL,
    provider TEXT NOT NULL,
    runtime TEXT NOT NULL,
    runtime_version TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS managed_applications (
    application_id TEXT PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE,
    account_uid INTEGER,
    account_gid INTEGER,
    layout_root TEXT NOT NULL,
    account_group TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    provider TEXT,
    revision TEXT,
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

/// A managed application: the persistent UUID -> account/UID/GID/layout
/// mapping and lifecycle state owned by the agent (phase 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedApplication {
    pub application_id: Uuid,
    pub account_name: String,
    pub account_group: String,
    pub account_uid: Option<u32>,
    pub account_gid: Option<u32>,
    pub layout_root: String,
    pub lifecycle_state: String,
    pub provider: Option<String>,
    pub revision: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A recorded release deployment (phase 15), used for restart reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentRecord {
    pub release_id: Uuid,
    pub application_id: Uuid,
    pub commit_id: String,
    /// `staged` | `active` | `superseded` | `failed` | `rolled_back`.
    pub state: String,
}

/// Registered source/domain for an application (phase 15.7), supplied by the
/// panel through the `agent.register_application` control operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationSource {
    pub application_id: Uuid,
    pub repository_url: String,
    pub server_name: String,
    pub provider: String,
    pub runtime: String,
    pub runtime_version: String,
}

/// A registered application schedule (phase 17.3), supplied by the panel and
/// installed as an app-owned systemd timer by the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRecord {
    pub schedule_id: Uuid,
    pub application_id: Uuid,
    pub program: String,
    /// JSON array of argument strings.
    pub arguments: String,
    pub on_calendar: String,
}

/// A managed application database allocation (phase 18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseResource {
    pub resource_id: Uuid,
    pub application_id: Uuid,
    pub engine: String,
    pub db_name: String,
    pub username: String,
    pub host: String,
    pub state: String,
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
                "INSERT INTO schema_meta(key, value) VALUES ('schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
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

    /// Reserve a managed application identity. Returns `true` when a new row was
    /// inserted, `false` when the application id already exists.
    ///
    /// The unique `account_name` constraint also makes account-name collisions
    /// a hard error rather than silently sharing an identity.
    pub fn insert_managed_application(
        &self,
        application: &ManagedApplication,
    ) -> Result<bool, JournalError> {
        let now = clock::now_rfc3339();
        let changed = self
            .connection
            .execute(
                "INSERT OR IGNORE INTO managed_applications (
                    application_id, account_name, account_group, account_uid, account_gid,
                    layout_root, lifecycle_state, provider, revision, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    application.application_id.as_str(),
                    application.account_name,
                    application.account_group,
                    application.account_uid,
                    application.account_gid,
                    application.layout_root,
                    application.lifecycle_state,
                    application.provider,
                    application.revision,
                    now,
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(changed == 1)
    }

    /// Look up a managed application by its application UUID.
    pub fn managed_application(
        &self,
        application_id: &Uuid,
    ) -> Result<Option<ManagedApplication>, JournalError> {
        self.connection
            .query_row(
                "SELECT application_id, account_name, account_group, account_uid, account_gid,
                        layout_root, lifecycle_state, provider, revision, created_at, updated_at
                 FROM managed_applications WHERE application_id = ?1",
                params![application_id.as_str()],
                row_to_managed_application,
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    /// Look up a managed application by its derived account name.
    pub fn managed_application_by_account(
        &self,
        account_name: &str,
    ) -> Result<Option<ManagedApplication>, JournalError> {
        self.connection
            .query_row(
                "SELECT application_id, account_name, account_group, account_uid, account_gid,
                        layout_root, lifecycle_state, provider, revision, created_at, updated_at
                 FROM managed_applications WHERE account_name = ?1",
                params![account_name],
                row_to_managed_application,
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    /// Update the lifecycle state and resolved account ids.
    pub fn update_managed_application_state(
        &self,
        application_id: &Uuid,
        lifecycle_state: &str,
        account_uid: Option<u32>,
        account_gid: Option<u32>,
    ) -> Result<(), JournalError> {
        self.connection
            .execute(
                "UPDATE managed_applications
                 SET lifecycle_state = ?2, account_uid = ?3, account_gid = ?4, updated_at = ?5
                 WHERE application_id = ?1",
                params![
                    application_id.as_str(),
                    lifecycle_state,
                    account_uid,
                    account_gid,
                    clock::now_rfc3339(),
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    /// Remove a managed-application row (rollback of a reservation only).
    pub fn delete_managed_application(&self, application_id: &Uuid) -> Result<(), JournalError> {
        self.connection
            .execute(
                "DELETE FROM managed_applications WHERE application_id = ?1",
                params![application_id.as_str()],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }
}

impl Journal {
    /// Record (or update) a release deployment.
    pub fn record_deployment(&self, record: &DeploymentRecord) -> Result<(), JournalError> {
        let now = clock::now_rfc3339();
        self.connection
            .execute(
                "INSERT INTO deployments (release_id, application_id, commit_id, state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(release_id) DO UPDATE SET
                    state = excluded.state, commit_id = excluded.commit_id, updated_at = excluded.updated_at",
                params![
                    record.release_id.as_str(),
                    record.application_id.as_str(),
                    record.commit_id,
                    record.state,
                    now,
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    pub fn deployment(&self, release_id: &Uuid) -> Result<Option<DeploymentRecord>, JournalError> {
        self.connection
            .query_row(
                "SELECT release_id, application_id, commit_id, state FROM deployments WHERE release_id = ?1",
                params![release_id.as_str()],
                row_to_deployment,
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    pub fn deployments_for_application(
        &self,
        application_id: &Uuid,
    ) -> Result<Vec<DeploymentRecord>, JournalError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT release_id, application_id, commit_id, state FROM deployments
                 WHERE application_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        let rows = statement
            .query_map(params![application_id.as_str()], row_to_deployment)
            .map_err(|err| JournalError::Query(err.to_string()))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|err| JournalError::Query(err.to_string()))?);
        }
        Ok(records)
    }
}

impl Journal {
    /// Register (or update) an application's source/domain, validated by the
    /// caller before it reaches here.
    pub fn set_application_source(&self, source: &ApplicationSource) -> Result<(), JournalError> {
        self.connection
            .execute(
                "INSERT INTO application_sources (
                    application_id, repository_url, server_name, provider, runtime, runtime_version, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(application_id) DO UPDATE SET
                    repository_url = excluded.repository_url,
                    server_name = excluded.server_name,
                    provider = excluded.provider,
                    runtime = excluded.runtime,
                    runtime_version = excluded.runtime_version,
                    updated_at = excluded.updated_at",
                params![
                    source.application_id.as_str(),
                    source.repository_url,
                    source.server_name,
                    source.provider,
                    source.runtime,
                    source.runtime_version,
                    clock::now_rfc3339(),
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    pub fn application_source(
        &self,
        application_id: &Uuid,
    ) -> Result<Option<ApplicationSource>, JournalError> {
        self.connection
            .query_row(
                "SELECT application_id, repository_url, server_name, provider, runtime, runtime_version
                 FROM application_sources WHERE application_id = ?1",
                params![application_id.as_str()],
                |row| {
                    Ok(ApplicationSource {
                        application_id: Uuid::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        repository_url: row.get(1)?,
                        server_name: row.get(2)?,
                        provider: row.get(3)?,
                        runtime: row.get(4)?,
                        runtime_version: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }
}

impl Journal {
    /// Register (or update) an application schedule.
    pub fn set_schedule(&self, record: &ScheduleRecord) -> Result<(), JournalError> {
        self.connection
            .execute(
                "INSERT INTO schedules (schedule_id, application_id, program, arguments, on_calendar, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(schedule_id) DO UPDATE SET
                    application_id = excluded.application_id,
                    program = excluded.program,
                    arguments = excluded.arguments,
                    on_calendar = excluded.on_calendar,
                    updated_at = excluded.updated_at",
                params![
                    record.schedule_id.as_str(),
                    record.application_id.as_str(),
                    record.program,
                    record.arguments,
                    record.on_calendar,
                    clock::now_rfc3339(),
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    pub fn schedule(&self, schedule_id: &Uuid) -> Result<Option<ScheduleRecord>, JournalError> {
        self.connection
            .query_row(
                "SELECT schedule_id, application_id, program, arguments, on_calendar FROM schedules WHERE schedule_id = ?1",
                params![schedule_id.as_str()],
                |row| {
                    Ok(ScheduleRecord {
                        schedule_id: Uuid::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        application_id: Uuid::parse(&row.get::<_, String>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        program: row.get(2)?,
                        arguments: row.get(3)?,
                        on_calendar: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    pub fn remove_schedule(&self, schedule_id: &Uuid) -> Result<(), JournalError> {
        self.connection
            .execute(
                "DELETE FROM schedules WHERE schedule_id = ?1",
                params![schedule_id.as_str()],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }
}

impl Journal {
    pub fn set_database(&self, record: &DatabaseResource) -> Result<(), JournalError> {
        self.connection
            .execute(
                "INSERT INTO databases (resource_id, application_id, engine, db_name, username, host, state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(resource_id) DO UPDATE SET
                    application_id = excluded.application_id,
                    engine = excluded.engine,
                    db_name = excluded.db_name,
                    username = excluded.username,
                    host = excluded.host,
                    state = excluded.state,
                    updated_at = excluded.updated_at",
                params![
                    record.resource_id.as_str(),
                    record.application_id.as_str(),
                    record.engine,
                    record.db_name,
                    record.username,
                    record.host,
                    record.state,
                    clock::now_rfc3339(),
                ],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    pub fn database(&self, resource_id: &Uuid) -> Result<Option<DatabaseResource>, JournalError> {
        self.connection
            .query_row(
                "SELECT resource_id, application_id, engine, db_name, username, host, state FROM databases WHERE resource_id = ?1",
                params![resource_id.as_str()],
                |row| {
                    Ok(DatabaseResource {
                        resource_id: Uuid::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        application_id: Uuid::parse(&row.get::<_, String>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        engine: row.get(2)?,
                        db_name: row.get(3)?,
                        username: row.get(4)?,
                        host: row.get(5)?,
                        state: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    /// The registered (ready) database for an application, if any.
    pub fn database_for_application(
        &self,
        application_id: &Uuid,
    ) -> Result<Option<DatabaseResource>, JournalError> {
        self.connection
            .query_row(
                "SELECT resource_id, application_id, engine, db_name, username, host, state
                 FROM databases WHERE application_id = ?1 AND state = 'ready' LIMIT 1",
                params![application_id.as_str()],
                |row| {
                    Ok(DatabaseResource {
                        resource_id: Uuid::parse(&row.get::<_, String>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        application_id: Uuid::parse(&row.get::<_, String>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        engine: row.get(2)?,
                        db_name: row.get(3)?,
                        username: row.get(4)?,
                        host: row.get(5)?,
                        state: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|err| JournalError::Query(err.to_string()))
    }

    pub fn update_database_state(
        &self,
        resource_id: &Uuid,
        state: &str,
    ) -> Result<(), JournalError> {
        self.connection
            .execute(
                "UPDATE databases SET state = ?2, updated_at = ?3 WHERE resource_id = ?1",
                params![resource_id.as_str(), state, clock::now_rfc3339()],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }

    pub fn remove_database(&self, resource_id: &Uuid) -> Result<(), JournalError> {
        self.connection
            .execute(
                "DELETE FROM databases WHERE resource_id = ?1",
                params![resource_id.as_str()],
            )
            .map_err(|err| JournalError::Query(err.to_string()))?;
        Ok(())
    }
}

fn row_to_deployment(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeploymentRecord> {
    Ok(DeploymentRecord {
        release_id: Uuid::parse(&row.get::<_, String>(0)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        application_id: Uuid::parse(&row.get::<_, String>(1)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        commit_id: row.get(2)?,
        state: row.get(3)?,
    })
}

fn row_to_managed_application(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagedApplication> {
    Ok(ManagedApplication {
        application_id: Uuid::parse(&row.get::<_, String>(0)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        account_name: row.get(1)?,
        account_group: row.get(2)?,
        account_uid: row.get(3)?,
        account_gid: row.get(4)?,
        layout_root: row.get(5)?,
        lifecycle_state: row.get(6)?,
        provider: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
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
