use crate::indexing::{IndexChange, IndexedFileSnapshot};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STORAGE_SCHEMA_VERSION: i64 = 10;
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageHealth {
    pub ok: bool,
    pub check: String,
    pub schema_version: Option<i64>,
    pub sqlite_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl StorageHealth {
    pub fn unavailable(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            check: "unavailable".to_string(),
            schema_version: None,
            sqlite_version: None,
            error: Some(error.into()),
        }
    }
}
#[derive(Debug, Clone)]
pub struct StorageError {
    pub code: &'static str,
    pub message: String,
}

impl StorageError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn sqlite(context: &str, error: rusqlite::Error) -> Self {
        Self::new("STORAGE_ERROR", format!("{context}: {error}"))
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub root_uri: String,
    pub created_at: String,
    pub updated_at: String,
    pub lifecycle_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRemovalApprovalRecord {
    pub id: String,
    pub project_id: String,
    pub state: String,
    pub project_state_revision: i64,
    pub project_lifecycle_state: String,
    pub requester_actor: String,
    pub requester_client: String,
    pub requester_delegator: Option<String>,
    pub approver_actor: Option<String>,
    pub approver_client: Option<String>,
    pub executor_actor: Option<String>,
    pub executor_client: Option<String>,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub decided_at_ms: Option<i64>,
    pub executed_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TaskContextSnapshot {
    pub project: ProjectRecord,
    pub project_state_revision: i64,
    pub index: Option<ProjectIndexState>,
    pub configuration: Option<ProjectConfiguration>,
    pub results: Vec<ResultRecord>,
    pub removal_approvals: Vec<ProjectRemovalApprovalRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfiguration {
    pub project_id: String,
    pub revision: i64,
    pub format_version: i64,
    pub project_type: String,
    pub adapter_id: Option<String>,
    pub adapter_version: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndexState {
    pub project_id: String,
    pub generation: i64,
    pub baseline_generation: i64,
    pub status: String,
    pub content_verification_required: bool,
    pub file_count: u64,
    pub total_bytes: u64,
    pub last_reconciled_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChangeRecord {
    pub id: String,
    pub project_id: String,
    pub generation: i64,
    pub change_kind: String,
    pub relative_path: String,
    pub previous_path: Option<String>,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyEdgeRecord {
    pub project_id: String,
    pub source_path: String,
    pub target_path: String,
    pub source_sha256: String,
    pub producer_id: String,
    pub producer_version: String,
    pub indexed_generation: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyCoverageRecord {
    pub project_id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub producer_id: String,
    pub producer_version: String,
    pub indexed_generation: i64,
    pub configuration_revision: Option<i64>,
    pub target_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidatedDependencyEdgeRecord {
    pub project_id: String,
    pub generation: i64,
    pub source_path: String,
    pub target_path: String,
    pub source_sha256: String,
    pub producer_id: String,
    pub producer_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectCheckCatalogRecord {
    pub project_id: String,
    pub revision: i64,
    pub index_generation: i64,
    pub configuration_revision: Option<i64>,
    pub catalog: Value,
    pub catalog_sha256: String,
}

#[derive(Clone)]
pub struct ProjectPlanSnapshot {
    pub state: ProjectIndexState,
    pub configuration: Option<ProjectConfiguration>,
    pub catalog: ProjectCheckCatalogRecord,
    pub files: Vec<IndexedFileSnapshot>,
    pub changes: Vec<ProjectChangeRecord>,
    pub edges: Vec<DependencyEdgeRecord>,
    pub invalidations: Vec<InvalidatedDependencyEdgeRecord>,
    pub coverage: Vec<DependencyCoverageRecord>,
    pub bounds_exceeded: bool,
}

pub struct PreparedCheckExecution {
    pub check_id: String,
    pub planned_check_id: String,
    pub status: &'static str,
    pub reason_code: &'static str,
    pub result_payload: Option<Value>,
}

pub struct DependencyReplacement<'a> {
    pub project_id: &'a str,
    pub expected_generation: i64,
    pub source_path: &'a str,
    pub expected_source_sha256: &'a str,
    pub producer_id: &'a str,
    pub producer_version: &'a str,
    pub configuration_guard: Option<(i64, &'a str, &'a str)>,
    pub targets: &'a [String],
}

pub enum IndexCommitMode {
    Authoritative,
    HintsOnly,
}

pub struct ProjectIndexCommit<'a> {
    pub project_id: &'a str,
    pub expected_generation: i64,
    pub touched_files: &'a [IndexedFileSnapshot],
    pub changes: &'a [IndexChange],
    pub file_count: usize,
    pub total_bytes: u64,
    pub mode: IndexCommitMode,
    pub content_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub schema_version: i64,
    pub producer_version: String,
    pub payload: Value,
    pub payload_sha256: String,
    pub provenance: Value,
    pub trust: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultDescription {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub schema_version: i64,
    pub producer_version: String,
    pub payload_sha256: String,
    pub payload_bytes: i64,
    pub trust: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub command: String,
    pub state: String,
    pub checkpoint: Value,
    pub result_id: Option<String>,
    pub provenance: Value,
    pub trust: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDescription {
    pub id: String,
    pub project_id: Option<String>,
    pub command: String,
    pub state: String,
    pub result_id: Option<String>,
    pub trust: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub key: String,
    pub command: String,
    pub request_sha256: String,
    pub response_json: String,
}

#[derive(Debug, Clone)]
pub struct NewTransaction<'a> {
    pub project_id: Option<&'a str>,
    pub request_id: &'a str,
    pub command: &'a str,
    pub effect_class: &'a str,
    pub permission: &'a str,
    pub actor_id: &'a str,
    pub client_id: &'a str,
    pub delegator_id: Option<&'a str>,
    pub request_sha256: &'a str,
    pub intended_summary: &'a str,
    pub rollback_status: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub request_id: String,
    pub command: String,
    pub effect_class: String,
    pub permission: String,
    pub state: String,
    pub actor_id: String,
    pub client_id: String,
    pub delegator_id: Option<String>,
    pub request_sha256: String,
    pub intended_summary: String,
    pub before_ref: Option<String>,
    pub after_ref: Option<String>,
    pub verification: String,
    pub result_id: Option<String>,
    pub job_id: Option<String>,
    pub rollback_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UsageMetricInput<'a> {
    pub request_id: &'a str,
    pub command: &'a str,
    pub command_version: u32,
    pub actor_id: &'a str,
    pub client_id: &'a str,
    pub delegator_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub effect_class: &'a str,
    pub permission: &'a str,
    pub ok: bool,
    pub replayed: bool,
    pub elapsed_ms: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub remote_calls: u64,
    pub model_tokens_in: u64,
    pub model_tokens_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub command_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub replay_count: u64,
    pub total_elapsed_ms: u64,
    pub total_request_bytes: u64,
    pub total_response_bytes: u64,
    pub remote_calls: u64,
    pub model_tokens_in: u64,
    pub model_tokens_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialHandleRecord {
    pub id: String,
    pub integration: String,
    pub scopes: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct EgressLedgerInput<'a> {
    pub project_id: Option<&'a str>,
    pub request_id: &'a str,
    pub actor_id: &'a str,
    pub client_id: &'a str,
    pub delegator_id: Option<&'a str>,
    pub destination: &'a str,
    pub data_classes: &'a [String],
    pub modalities: &'a [String],
    pub source_refs: &'a [String],
    pub purpose: &'a str,
    pub approx_bytes: u64,
    pub approx_tokens: u64,
    pub credential_handle: Option<&'a str>,
    pub decision: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressLedgerRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub request_id: String,
    pub actor_id: String,
    pub client_id: String,
    pub delegator_id: Option<String>,
    pub destination: String,
    pub data_classes: Vec<String>,
    pub modalities: Vec<String>,
    pub source_refs: Vec<String>,
    pub purpose: String,
    pub approx_bytes: u64,
    pub approx_tokens: u64,
    pub credential_handle: Option<String>,
    pub decision: String,
    pub reason: String,
    pub created_at: String,
}

pub struct RelayStorage {
    db_path: PathBuf,
    conn: Connection,
}

fn opaque_id(prefix: &str) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let material = format!("{}:{}:{}:{}", prefix, std::process::id(), now, counter);
    let digest = Sha256::digest(material.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("{prefix}-{hex}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
fn sql_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn json_text(value: &Value) -> Result<String, StorageError> {
    serde_json::to_string(value)
        .map_err(|error| StorageError::new("STORAGE_ERROR", format!("serialize JSON: {error}")))
}

fn parse_json(text: &str, column: usize) -> Result<Value, rusqlite::Error> {
    serde_json::from_str(text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn sqlite_now(conn: &Connection) -> Result<String, StorageError> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
        row.get(0)
    })
    .map_err(|error| StorageError::sqlite("read SQLite clock", error))
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn decode_removal_approval(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProjectRemovalApprovalRecord> {
    Ok(ProjectRemovalApprovalRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        state: row.get(2)?,
        project_state_revision: row.get(3)?,
        project_lifecycle_state: row.get(4)?,
        requester_actor: row.get(5)?,
        requester_client: row.get(6)?,
        requester_delegator: row.get(7)?,
        approver_actor: row.get(8)?,
        approver_client: row.get(9)?,
        executor_actor: row.get(10)?,
        executor_client: row.get(11)?,
        created_at_ms: row.get(12)?,
        expires_at_ms: row.get(13)?,
        decided_at_ms: row.get(14)?,
        executed_at_ms: row.get(15)?,
    })
}

const REMOVAL_APPROVAL_COLUMNS: &str =
    "id, project_id, state, project_state_revision, project_lifecycle_state,
     requester_actor, requester_client, requester_delegator, approver_actor,
     approver_client, executor_actor, executor_client, created_at_ms,
     expires_at_ms, decided_at_ms, executed_at_ms";

fn read_removal_approval(
    conn: &Connection,
    project_id: &str,
    approval_id: &str,
) -> Result<Option<ProjectRemovalApprovalRecord>, StorageError> {
    conn.query_row(
        &format!(
            "SELECT {REMOVAL_APPROVAL_COLUMNS} FROM project_removal_approvals
                  WHERE project_id = ?1 AND id = ?2"
        ),
        params![project_id, approval_id],
        decode_removal_approval,
    )
    .optional()
    .map_err(|error| StorageError::sqlite("read removal approval", error))
}

fn effective_removal_state(
    approval: &ProjectRemovalApprovalRecord,
    project_revision: i64,
    project_lifecycle: &str,
    now_ms: i64,
) -> &'static str {
    match approval.state.as_str() {
        "pending" | "approved_pending_execution" => {
            if approval.project_state_revision != project_revision
                || approval.project_lifecycle_state != project_lifecycle
                || project_lifecycle == "removed"
            {
                "stale"
            } else if now_ms >= approval.expires_at_ms {
                "expired"
            } else if approval.state == "pending" {
                "pending"
            } else {
                "approved_pending_execution"
            }
        }
        "rejected" => "rejected",
        "expired" => "expired",
        "stale" => "stale",
        "executed" => "executed",
        _ => "stale",
    }
}

impl RelayStorage {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StorageError::new(
                    "STORAGE_ERROR",
                    format!("create database directory: {error}"),
                )
            })?;
        }
        let conn = Connection::open(&db_path)
            .map_err(|error| StorageError::sqlite("open database", error))?;
        let mut storage = Self { db_path, conn };
        storage.configure()?;
        storage.migrate()?;
        Ok(storage)
    }

    fn configure(&self) -> Result<(), StorageError> {
        self.conn
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=FULL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA busy_timeout=3000;",
            )
            .map_err(|error| StorageError::sqlite("configure database", error))
    }

    fn migrate(&mut self) -> Result<(), StorageError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );",
            )
            .map_err(|error| StorageError::sqlite("create migration table", error))?;

        let current = self.schema_version()?;
        if current > STORAGE_SCHEMA_VERSION {
            return Err(StorageError::new(
                "STORAGE_SCHEMA_FUTURE",
                format!(
                    "storage schema {current} is newer than supported {}",
                    STORAGE_SCHEMA_VERSION
                ),
            ));
        }
        if current < 1 {
            self.apply_schema_one()?;
        }
        if self.schema_version()? < 2 {
            self.apply_schema_two()?;
        }
        if self.schema_version()? < 3 {
            self.apply_schema_three()?;
        }
        if self.schema_version()? < 4 {
            self.apply_schema_four()?;
        }
        if self.schema_version()? < 5 {
            self.apply_schema_five()?;
        }
        if self.schema_version()? < 6 {
            self.apply_schema_six()?;
        }
        if self.schema_version()? < 7 {
            self.apply_schema_seven()?;
        }
        if self.schema_version()? < 8 {
            self.apply_schema_eight()?;
        }
        if self.schema_version()? < 9 {
            self.apply_schema_nine()?;
        }
        if self.schema_version()? < 10 {
            self.apply_schema_ten()?;
        }
        Ok(())
    }

    fn apply_schema_one(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-1 migration", error))?;

        tx.execute_batch(
            "CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_uri TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE results (
                id TEXT PRIMARY KEY,
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                kind TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                producer_version TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_sha256 TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .map_err(|error| StorageError::sqlite("create schema-1 tables", error))?;
        tx.execute_batch(
            "CREATE TABLE jobs (
                id TEXT PRIMARY KEY,
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                command TEXT NOT NULL,
                state TEXT NOT NULL,
                checkpoint_json TEXT NOT NULL,
                result_id TEXT REFERENCES results(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX results_project_created
              ON results(project_id, created_at);
            CREATE INDEX jobs_project_updated
              ON jobs(project_id, updated_at);",
        )
        .map_err(|error| StorageError::sqlite("create schema-1 job tables", error))?;

        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![1i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 1", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 1", error))
    }
    fn apply_schema_two(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-2 migration", error))?;

        tx.execute_batch(
            "ALTER TABLE results
               ADD COLUMN provenance_json TEXT NOT NULL DEFAULT '{}';
             ALTER TABLE results
               ADD COLUMN trust TEXT NOT NULL DEFAULT 'local';
             ALTER TABLE jobs
               ADD COLUMN provenance_json TEXT NOT NULL DEFAULT '{}';
             ALTER TABLE jobs
               ADD COLUMN trust TEXT NOT NULL DEFAULT 'local';
             CREATE TABLE idempotency_records (
               key TEXT PRIMARY KEY,
               command TEXT NOT NULL,
               request_sha256 TEXT NOT NULL,
               response_json TEXT NOT NULL,
               created_at TEXT NOT NULL
             );",
        )
        .map_err(|error| StorageError::sqlite("apply schema migration 2", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![2i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 2", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 2", error))
    }

    fn apply_schema_three(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-3 migration", error))?;

        tx.execute_batch(
            "CREATE TABLE transactions (
               id TEXT PRIMARY KEY,
               project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
               request_id TEXT NOT NULL,
               command TEXT NOT NULL,
               effect_class TEXT NOT NULL,
               permission TEXT NOT NULL,
               state TEXT NOT NULL,
               actor_id TEXT NOT NULL,
               client_id TEXT NOT NULL,
               delegator_id TEXT,
               request_sha256 TEXT NOT NULL,
               intended_summary TEXT NOT NULL,
               before_ref TEXT,
               after_ref TEXT,
               verification TEXT NOT NULL,
               result_id TEXT REFERENCES results(id) ON DELETE SET NULL,
               job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL,
               rollback_status TEXT NOT NULL,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );
             CREATE INDEX transactions_project_updated
               ON transactions(project_id, updated_at);
             CREATE INDEX transactions_request
               ON transactions(request_id);

             CREATE TABLE usage_metrics (
               id TEXT PRIMARY KEY,
               request_id TEXT NOT NULL,
               command TEXT NOT NULL,
               command_version INTEGER NOT NULL,
               actor_id TEXT NOT NULL,
               client_id TEXT NOT NULL,
               delegator_id TEXT,
               project_id TEXT,
               effect_class TEXT NOT NULL,
               permission TEXT NOT NULL,
               ok INTEGER NOT NULL,
               replayed INTEGER NOT NULL,
               elapsed_ms INTEGER NOT NULL,
               request_bytes INTEGER NOT NULL,
               response_bytes INTEGER NOT NULL,
               remote_calls INTEGER NOT NULL DEFAULT 0,
               model_tokens_in INTEGER NOT NULL DEFAULT 0,
               model_tokens_out INTEGER NOT NULL DEFAULT 0,
               created_at TEXT NOT NULL
             );
             CREATE INDEX usage_metrics_created
               ON usage_metrics(created_at);
             CREATE INDEX usage_metrics_command
               ON usage_metrics(command, created_at);",
        )
        .map_err(|error| StorageError::sqlite("create schema-3 transaction/usage tables", error))?;

        tx.execute_batch(
            "CREATE TABLE credential_handles (
               id TEXT PRIMARY KEY,
               integration TEXT NOT NULL,
               scopes_json TEXT NOT NULL,
               status TEXT NOT NULL,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );

             CREATE TABLE egress_ledger (
               id TEXT PRIMARY KEY,
               project_id TEXT,
               request_id TEXT NOT NULL,
               actor_id TEXT NOT NULL,
               client_id TEXT NOT NULL,
               delegator_id TEXT,
               destination TEXT NOT NULL,
               data_classes_json TEXT NOT NULL,
               modalities_json TEXT NOT NULL,
               source_refs_json TEXT NOT NULL,
               purpose TEXT NOT NULL,
               approx_bytes INTEGER NOT NULL,
               approx_tokens INTEGER NOT NULL,
               credential_handle TEXT,
               decision TEXT NOT NULL,
               reason TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE INDEX egress_project_created
               ON egress_ledger(project_id, created_at);
             CREATE INDEX egress_destination_created
               ON egress_ledger(destination, created_at);",
        )
        .map_err(|error| StorageError::sqlite("create schema-3 credential/egress tables", error))?;

        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![3i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 3", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 3", error))
    }

    fn apply_schema_four(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-4 migration", error))?;

        tx.execute_batch(
            "CREATE TABLE project_index_state (
               project_id TEXT PRIMARY KEY
                 REFERENCES projects(id) ON DELETE CASCADE,
               generation INTEGER NOT NULL,
               status TEXT NOT NULL,
               file_count INTEGER NOT NULL,
               total_bytes INTEGER NOT NULL,
               last_reconciled_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );

             CREATE TABLE project_files (
               project_id TEXT NOT NULL
                 REFERENCES projects(id) ON DELETE CASCADE,
               relative_path TEXT NOT NULL,
               size_bytes INTEGER NOT NULL,
               modified_unix_ns INTEGER NOT NULL,
               content_sha256 TEXT NOT NULL,
               indexed_at TEXT NOT NULL,
               PRIMARY KEY(project_id, relative_path)
             );
             CREATE TABLE project_changes (
               id TEXT PRIMARY KEY,
               project_id TEXT NOT NULL
                 REFERENCES projects(id) ON DELETE CASCADE,
               generation INTEGER NOT NULL,
               change_kind TEXT NOT NULL,
               relative_path TEXT NOT NULL,
               previous_path TEXT,
               before_sha256 TEXT,
               after_sha256 TEXT,
               detected_at TEXT NOT NULL
             );
             CREATE INDEX project_changes_project_generation
               ON project_changes(project_id, generation, detected_at);",
        )
        .map_err(|error| StorageError::sqlite("create schema-4 project index tables", error))?;

        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![4i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 4", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 4", error))
    }

    fn apply_schema_five(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-5 migration", error))?;
        tx.execute_batch(
            "ALTER TABLE project_index_state
               ADD COLUMN baseline_generation INTEGER NOT NULL DEFAULT 1;
             UPDATE project_index_state
               SET baseline_generation = generation;
             CREATE TABLE project_dependency_edges (
               project_id TEXT NOT NULL
                 REFERENCES projects(id) ON DELETE CASCADE,
               source_path TEXT NOT NULL,
               target_path TEXT NOT NULL,
               source_sha256 TEXT NOT NULL,
               producer_id TEXT NOT NULL,
               producer_version TEXT NOT NULL,
               indexed_generation INTEGER NOT NULL,
               updated_at TEXT NOT NULL,
               PRIMARY KEY(project_id, source_path, target_path, producer_id)
             );
             CREATE INDEX project_dependency_edges_target
               ON project_dependency_edges(project_id, target_path);",
        )
        .map_err(|error| StorageError::sqlite("create schema-5 dependency tables", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![5i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 5", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 5", error))
    }

    fn apply_schema_six(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-6 migration", error))?;
        tx.execute_batch(
            "ALTER TABLE project_index_state
               ADD COLUMN content_verification_required INTEGER NOT NULL DEFAULT 0;
             UPDATE project_index_state
               SET status = 'stale', content_verification_required = 1;",
        )
        .map_err(|error| StorageError::sqlite("update schema-6 project index state", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![6i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 6", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 6", error))
    }

    fn apply_schema_seven(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-7 migration", error))?;
        tx.execute_batch(
            "CREATE TABLE project_configuration (
               project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
               revision INTEGER NOT NULL CHECK(revision >= 1),
               format_version INTEGER NOT NULL CHECK(format_version = 1),
               project_type TEXT NOT NULL,
               adapter_id TEXT,
               adapter_version TEXT,
               updated_at TEXT NOT NULL,
               CHECK((adapter_id IS NULL) = (adapter_version IS NULL))
             );",
        )
        .map_err(|error| StorageError::sqlite("create schema-7 project configuration", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![7i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 7", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 7", error))
    }

    fn apply_schema_eight(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-8 migration", error))?;
        tx.execute_batch(
            "ALTER TABLE projects ADD COLUMN lifecycle_state TEXT NOT NULL DEFAULT 'active'
               CHECK(lifecycle_state IN ('active', 'archived', 'removed'));
             CREATE INDEX projects_lifecycle_state ON projects(lifecycle_state, created_at, id);",
        )
        .map_err(|error| StorageError::sqlite("add project lifecycle state", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![8i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 8", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 8", error))
    }

    fn apply_schema_nine(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-9 migration", error))?;
        tx.execute_batch(
            "CREATE TABLE project_dependency_coverage (
               project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
               source_path TEXT NOT NULL,
               source_sha256 TEXT NOT NULL,
               producer_id TEXT NOT NULL,
               producer_version TEXT NOT NULL,
               indexed_generation INTEGER NOT NULL,
               configuration_revision INTEGER,
               target_count INTEGER NOT NULL CHECK(target_count >= 0),
               updated_at TEXT NOT NULL,
               PRIMARY KEY(project_id, source_path, producer_id)
             );
             CREATE TABLE project_dependency_edge_invalidations (
               project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
               generation INTEGER NOT NULL,
               source_path TEXT NOT NULL,
               target_path TEXT NOT NULL,
               source_sha256 TEXT NOT NULL,
               producer_id TEXT NOT NULL,
               producer_version TEXT NOT NULL,
               PRIMARY KEY(project_id, generation, source_path, target_path, producer_id)
             );
             CREATE INDEX project_dependency_edge_invalidations_generation
               ON project_dependency_edge_invalidations(project_id, generation);
             CREATE TABLE project_check_catalog (
               project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
               revision INTEGER NOT NULL CHECK(revision >= 1),
               index_generation INTEGER NOT NULL,
               configuration_revision INTEGER,
               catalog_json TEXT NOT NULL,
               catalog_sha256 TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );",
        )
        .map_err(|error| StorageError::sqlite("create dependency coverage", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![9i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 9", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 9", error))
    }

    fn apply_schema_ten(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StorageError::sqlite("begin schema-10 migration", error))?;
        tx.execute_batch(
            "ALTER TABLE projects ADD COLUMN state_revision INTEGER NOT NULL DEFAULT 1
               CHECK(state_revision >= 1);
             CREATE TABLE project_removal_approvals (
               id TEXT PRIMARY KEY,
               project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
               state TEXT NOT NULL CHECK(state IN (
                 'pending', 'approved_pending_execution', 'rejected',
                 'expired', 'stale', 'executed'
               )),
               project_state_revision INTEGER NOT NULL,
               project_lifecycle_state TEXT NOT NULL,
               requester_actor TEXT NOT NULL,
               requester_client TEXT NOT NULL,
               requester_delegator TEXT,
               approver_actor TEXT,
               approver_client TEXT,
               executor_actor TEXT,
               executor_client TEXT,
               created_at_ms INTEGER NOT NULL,
               expires_at_ms INTEGER NOT NULL,
               decided_at_ms INTEGER,
               executed_at_ms INTEGER
             );
             CREATE INDEX project_removal_approvals_project
               ON project_removal_approvals(project_id, created_at_ms DESC, id DESC);",
        )
        .map_err(|error| StorageError::sqlite("create project removal approvals", error))?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![10i64, applied_at],
        )
        .map_err(|error| StorageError::sqlite("record schema migration 10", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit schema migration 10", error))
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0)
                 FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| StorageError::sqlite("read schema version", error))
    }

    pub fn sqlite_version(&self) -> Result<String, StorageError> {
        self.conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(|error| StorageError::sqlite("read SQLite version", error))
    }

    pub fn integrity(&self) -> Result<StorageHealth, StorageError> {
        let check: String = self
            .conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| StorageError::sqlite("run quick_check", error))?;
        Ok(StorageHealth {
            ok: check == "ok",
            check,
            schema_version: Some(self.schema_version()?),
            sqlite_version: Some(self.sqlite_version()?),
            error: None,
        })
    }

    pub fn register_project(
        &self,
        id: Option<&str>,
        name: &str,
        root_uri: &str,
    ) -> Result<ProjectRecord, StorageError> {
        let id = id
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| opaque_id("PRJ"));
        let now = sqlite_now(&self.conn)?;
        if let Some(existing) = self.get_project(&id)? {
            if existing.lifecycle_state != "active" {
                return Err(StorageError::new(
                    "PROJECT_INACTIVE",
                    "archived or removed project IDs cannot be registered again",
                ));
            }
        }
        self.conn
            .execute(
                "INSERT INTO projects(
                    id, name, root_uri, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                   name=excluded.name,
                   root_uri=excluded.root_uri,
                   state_revision=projects.state_revision + 1,
                   updated_at=excluded.updated_at",
                params![id, name, root_uri, now, now],
            )
            .map_err(|error| StorageError::sqlite("register project", error))?;
        self.get_project(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "project disappeared after write"))
    }
    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, name, root_uri, created_at, updated_at, lifecycle_state
                 FROM projects WHERE id = ?1",
                [id],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_uri: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        lifecycle_state: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read project", error))
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>, StorageError> {
        self.list_projects_with_inactive(false)
    }

    pub fn list_projects_with_inactive(
        &self,
        include_inactive: bool,
    ) -> Result<Vec<ProjectRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, name, root_uri, created_at, updated_at, lifecycle_state
                 FROM projects WHERE lifecycle_state = 'active' OR ?1
                 ORDER BY created_at, id",
            )
            .map_err(|error| StorageError::sqlite("prepare project list", error))?;
        let rows = statement
            .query_map([include_inactive], |row| {
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_uri: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    lifecycle_state: row.get(5)?,
                })
            })
            .map_err(|error| StorageError::sqlite("query project list", error))?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row.map_err(|error| StorageError::sqlite("decode project row", error))?);
        }
        Ok(projects)
    }

    pub fn get_active_project(&self, id: &str) -> Result<ProjectRecord, StorageError> {
        let project = self
            .get_project(id)?
            .ok_or_else(|| StorageError::new("PROJECT_NOT_FOUND", "project not found"))?;
        if project.lifecycle_state != "active" {
            return Err(StorageError::new(
                "PROJECT_NOT_FOUND",
                "project is not active",
            ));
        }
        Ok(project)
    }

    pub fn set_project_lifecycle(
        &self,
        id: &str,
        target: &str,
    ) -> Result<(ProjectRecord, bool), StorageError> {
        if !matches!(target, "active" | "archived") {
            return Err(StorageError::new(
                "VALIDATION_FAILED",
                "unsupported lifecycle state",
            ));
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin project lifecycle change", error))?;
        let current: Option<String> = tx
            .query_row(
                "SELECT lifecycle_state FROM projects WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read project lifecycle state", error))?;
        let current =
            current.ok_or_else(|| StorageError::new("PROJECT_NOT_FOUND", "project not found"))?;
        if current == "removed" {
            return Err(StorageError::new(
                "PROJECT_REMOVED",
                "removed project cannot be restored or archived",
            ));
        }
        if target == "active" && !matches!(current.as_str(), "active" | "archived") {
            return Err(StorageError::new(
                "PROJECT_NOT_ARCHIVED",
                "only archived projects can be restored",
            ));
        }
        let changed = current != target;
        if changed {
            let now = sqlite_now(&tx)?;
            if target == "active" {
                tx.execute(
                    "UPDATE project_index_state
                     SET status = 'stale', content_verification_required = 1, updated_at = ?2
                     WHERE project_id = ?1",
                    params![id, now],
                )
                .map_err(|error| {
                    StorageError::sqlite("invalidate restored project index", error)
                })?;
            }
            tx.execute(
                "UPDATE projects SET lifecycle_state = ?2, updated_at = ?3,
                   state_revision = state_revision + 1 WHERE id = ?1",
                params![id, target, now],
            )
            .map_err(|error| StorageError::sqlite("change project lifecycle state", error))?;
        }
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit project lifecycle change", error))?;
        let project = self.get_project(id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "project disappeared after lifecycle change",
            )
        })?;
        Ok((project, changed))
    }

    pub fn plan_project_removal(
        &self,
        project_id: &str,
        requester_actor: &str,
        requester_client: &str,
        requester_delegator: Option<&str>,
    ) -> Result<ProjectRemovalApprovalRecord, StorageError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| StorageError::sqlite("begin removal plan", e))?;
        let project: Option<(i64, String)> = tx
            .query_row(
                "SELECT state_revision, lifecycle_state FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| StorageError::sqlite("read removal target", e))?;
        let (revision, lifecycle) =
            project.ok_or_else(|| StorageError::new("PROJECT_NOT_FOUND", "project not found"))?;
        if lifecycle == "removed" {
            return Err(StorageError::new(
                "PROJECT_REMOVED",
                "project is already removed",
            ));
        }
        let now = current_unix_ms();
        let outstanding: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM project_removal_approvals WHERE project_id = ?1
             AND state IN ('pending', 'approved_pending_execution') AND expires_at_ms > ?2
             AND project_state_revision = ?3 AND project_lifecycle_state = ?4",
                params![project_id, now, revision, lifecycle],
                |row| row.get(0),
            )
            .map_err(|e| StorageError::sqlite("count outstanding removal plans", e))?;
        if outstanding >= 4 {
            return Err(StorageError::new(
                "APPROVAL_LIMIT",
                "too many outstanding removal plans for project",
            ));
        }
        let id = opaque_id("APR");
        let expires = now.saturating_add(15 * 60 * 1000);
        tx.execute(
            "INSERT INTO project_removal_approvals
            (id, project_id, state, project_state_revision, project_lifecycle_state,
             requester_actor, requester_client, requester_delegator, created_at_ms, expires_at_ms)
            VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                project_id,
                revision,
                lifecycle,
                requester_actor,
                requester_client,
                requester_delegator,
                now,
                expires
            ],
        )
        .map_err(|e| StorageError::sqlite("write removal plan", e))?;
        let approval =
            read_removal_approval(&tx, project_id, &id)?.expect("inserted approval exists");
        tx.commit()
            .map_err(|e| StorageError::sqlite("commit removal plan", e))?;
        Ok(approval)
    }

    pub fn get_project_removal_approval(
        &self,
        project_id: &str,
        approval_id: &str,
    ) -> Result<ProjectRemovalApprovalRecord, StorageError> {
        let mut approval = read_removal_approval(&self.conn, project_id, approval_id)?
            .ok_or_else(|| StorageError::new("APPROVAL_NOT_FOUND", "removal approval not found"))?;
        let project: (i64, String) = self
            .conn
            .query_row(
                "SELECT state_revision, lifecycle_state FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| StorageError::sqlite("read removal target", e))?;
        approval.state =
            effective_removal_state(&approval, project.0, &project.1, current_unix_ms())
                .to_string();
        Ok(approval)
    }

    pub fn list_project_removal_approvals(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<ProjectRemovalApprovalRecord>, StorageError> {
        let project: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT state_revision, lifecycle_state FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| StorageError::sqlite("read removal target", e))?;
        let (revision, lifecycle) =
            project.ok_or_else(|| StorageError::new("PROJECT_NOT_FOUND", "project not found"))?;
        let mut stmt = self.conn.prepare(&format!("SELECT {REMOVAL_APPROVAL_COLUMNS} FROM project_removal_approvals WHERE project_id = ?1 ORDER BY created_at_ms DESC, id DESC LIMIT ?2"))
            .map_err(|e| StorageError::sqlite("prepare removal approval list", e))?;
        let rows = stmt
            .query_map(
                params![project_id, limit.min(100) as i64],
                decode_removal_approval,
            )
            .map_err(|e| StorageError::sqlite("list removal approvals", e))?;
        let mut out = Vec::new();
        let now = current_unix_ms();
        for row in rows {
            let mut approval =
                row.map_err(|e| StorageError::sqlite("decode removal approval", e))?;
            approval.state =
                effective_removal_state(&approval, revision, &lifecycle, now).to_string();
            out.push(approval);
        }
        Ok(out)
    }

    pub fn task_context_snapshot(
        &self,
        project_id: &str,
        result_ids: &[&str],
        approval_ids: &[&str],
    ) -> Result<TaskContextSnapshot, StorageError> {
        if result_ids.len() > 8 || approval_ids.len() > 4 {
            return Err(StorageError::new(
                "VALIDATION_FAILED",
                "task context source limit exceeded",
            ));
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| StorageError::sqlite("begin task context snapshot", e))?;
        let project = self
            .get_project(project_id)?
            .ok_or_else(|| StorageError::new("PROJECT_NOT_FOUND", "project not found"))?;
        let project_state_revision: i64 = tx
            .query_row(
                "SELECT state_revision FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(|e| StorageError::sqlite("read project revision", e))?;
        let index = self.get_project_index_state(project_id)?;
        let configuration = self.get_project_configuration(project_id)?;
        let mut results = Vec::with_capacity(result_ids.len());
        for id in result_ids {
            let record = self.get_result(id)?.ok_or_else(|| {
                StorageError::new("RESULT_NOT_FOUND", "result not found in project")
            })?;
            if record.project_id.as_deref() != Some(project_id) {
                return Err(StorageError::new(
                    "RESULT_NOT_FOUND",
                    "result not found in project",
                ));
            }
            results.push(record);
        }
        let mut removal_approvals = Vec::with_capacity(approval_ids.len());
        for id in approval_ids {
            removal_approvals.push(self.get_project_removal_approval(project_id, id)?);
        }
        tx.commit()
            .map_err(|e| StorageError::sqlite("commit task context snapshot", e))?;
        Ok(TaskContextSnapshot {
            project,
            project_state_revision,
            index,
            configuration,
            results,
            removal_approvals,
        })
    }

    pub fn decide_project_removal(
        &self,
        project_id: &str,
        approval_id: &str,
        approve: bool,
        approver_actor: &str,
        approver_client: &str,
    ) -> Result<ProjectRemovalApprovalRecord, StorageError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| StorageError::sqlite("begin removal decision", e))?;
        let mut approval = read_removal_approval(&tx, project_id, approval_id)?
            .ok_or_else(|| StorageError::new("APPROVAL_NOT_FOUND", "removal approval not found"))?;
        let project: (i64, String) = tx
            .query_row(
                "SELECT state_revision, lifecycle_state FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| StorageError::sqlite("read removal target", e))?;
        let state = effective_removal_state(&approval, project.0, &project.1, current_unix_ms());
        if matches!(state, "stale" | "expired") {
            tx.execute(
                "UPDATE project_removal_approvals SET state = ?3 WHERE project_id = ?1 AND id = ?2",
                params![project_id, approval_id, state],
            )
            .map_err(|e| StorageError::sqlite("record invalid removal approval", e))?;
            approval =
                read_removal_approval(&tx, project_id, approval_id)?.expect("approval exists");
            tx.commit()
                .map_err(|e| StorageError::sqlite("commit invalid removal approval", e))?;
            return Ok(approval);
        }
        let next = match (state, approve) {
            ("pending", true) => "approved_pending_execution",
            ("pending" | "approved_pending_execution", false) => "rejected",
            _ => {
                return Err(StorageError::new(
                    "APPROVAL_STATE_CONFLICT",
                    "approval cannot be decided in its current state",
                ));
            }
        };
        let now = current_unix_ms();
        tx.execute("UPDATE project_removal_approvals SET state = ?3, approver_actor = ?4, approver_client = ?5, decided_at_ms = ?6 WHERE project_id = ?1 AND id = ?2",
            params![project_id, approval_id, next, approver_actor, approver_client, now])
            .map_err(|e| StorageError::sqlite("write removal decision", e))?;
        approval = read_removal_approval(&tx, project_id, approval_id)?.expect("approval exists");
        tx.commit()
            .map_err(|e| StorageError::sqlite("commit removal decision", e))?;
        Ok(approval)
    }

    pub fn execute_project_removal(
        &self,
        project_id: &str,
        approval_id: &str,
        executor_actor: &str,
        executor_client: &str,
    ) -> Result<(ProjectRemovalApprovalRecord, bool), StorageError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| StorageError::sqlite("begin approved removal", e))?;
        let mut approval = read_removal_approval(&tx, project_id, approval_id)?
            .ok_or_else(|| StorageError::new("APPROVAL_NOT_FOUND", "removal approval not found"))?;
        if approval.state == "executed" {
            return Ok((approval, false));
        }
        let project: (i64, String) = tx
            .query_row(
                "SELECT state_revision, lifecycle_state FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| StorageError::sqlite("read removal target", e))?;
        let state = effective_removal_state(&approval, project.0, &project.1, current_unix_ms());
        if state != "approved_pending_execution" {
            if state == "stale" || state == "expired" {
                tx.execute("UPDATE project_removal_approvals SET state = ?3 WHERE project_id = ?1 AND id = ?2", params![project_id, approval_id, state])
                    .map_err(|e| StorageError::sqlite("record invalid removal approval", e))?;
                tx.commit()
                    .map_err(|e| StorageError::sqlite("commit invalid removal approval", e))?;
            }
            return Err(StorageError::new(
                "APPROVAL_STATE_CONFLICT",
                format!("removal approval is {state}"),
            ));
        }
        let now_text = sqlite_now(&tx)?;
        let now = current_unix_ms();
        tx.execute("UPDATE projects SET lifecycle_state = 'removed', state_revision = state_revision + 1, updated_at = ?2 WHERE id = ?1", params![project_id, now_text])
            .map_err(|e| StorageError::sqlite("remove approved project", e))?;
        tx.execute("UPDATE project_removal_approvals SET state = 'executed', executor_actor = ?3, executor_client = ?4, executed_at_ms = ?5 WHERE project_id = ?1 AND id = ?2",
            params![project_id, approval_id, executor_actor, executor_client, now])
            .map_err(|e| StorageError::sqlite("record approved removal", e))?;
        approval = read_removal_approval(&tx, project_id, approval_id)?.expect("approval exists");
        tx.commit()
            .map_err(|e| StorageError::sqlite("commit approved removal", e))?;
        Ok((approval, true))
    }

    pub fn get_project_configuration(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectConfiguration>, StorageError> {
        self.conn
            .query_row(
                "SELECT project_id, revision, format_version, project_type,
                    adapter_id, adapter_version, updated_at
             FROM project_configuration WHERE project_id = ?1",
                [project_id],
                |row| {
                    Ok(ProjectConfiguration {
                        project_id: row.get(0)?,
                        revision: row.get(1)?,
                        format_version: row.get(2)?,
                        project_type: row.get(3)?,
                        adapter_id: row.get(4)?,
                        adapter_version: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read project configuration", error))
    }

    pub fn put_project_configuration(
        &self,
        project_id: &str,
        expected_revision: i64,
        project_type: &str,
        adapter_id: Option<&str>,
        adapter_version: Option<&str>,
    ) -> Result<ProjectConfiguration, StorageError> {
        self.get_active_project(project_id)?;
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin project configuration update", error))?;
        let project_exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(|error| StorageError::sqlite("check configured project", error))?;
        if project_exists == 0 {
            return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
        }
        let current_revision: Option<i64> = tx
            .query_row(
                "SELECT revision FROM project_configuration WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read configuration revision", error))?;
        if current_revision.unwrap_or(0) != expected_revision {
            return Err(StorageError::new(
                "PROJECT_CONFIG_CONFLICT",
                format!(
                    "expected configuration revision {expected_revision}, found {}",
                    current_revision.unwrap_or(0)
                ),
            ));
        }
        let next_revision = expected_revision.checked_add(1).ok_or_else(|| {
            StorageError::new(
                "PROJECT_CONFIG_CONFLICT",
                "configuration revision exhausted",
            )
        })?;
        tx.execute(
            "INSERT INTO project_configuration(
               project_id, revision, format_version, project_type,
               adapter_id, adapter_version, updated_at
             ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)
             ON CONFLICT(project_id) DO UPDATE SET
               revision=excluded.revision,
               project_type=excluded.project_type,
               adapter_id=excluded.adapter_id,
               adapter_version=excluded.adapter_version,
               updated_at=excluded.updated_at",
            params![
                project_id,
                next_revision,
                project_type,
                adapter_id,
                adapter_version,
                now
            ],
        )
        .map_err(|error| StorageError::sqlite("write project configuration", error))?;
        tx.execute(
            "DELETE FROM project_dependency_edges WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| {
            StorageError::sqlite("invalidate edges after configuration change", error)
        })?;
        tx.execute(
            "DELETE FROM project_dependency_coverage WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| {
            StorageError::sqlite(
                "invalidate parser coverage after configuration change",
                error,
            )
        })?;
        tx.execute(
            "DELETE FROM project_dependency_edge_invalidations WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| {
            StorageError::sqlite("invalidate edge history after configuration change", error)
        })?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit project configuration", error))?;
        self.get_project_configuration(project_id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "project configuration disappeared after update",
            )
        })
    }

    pub fn get_project_index_state(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectIndexState>, StorageError> {
        self.conn
            .query_row(
                "SELECT project_id, generation, baseline_generation, status,
                        file_count, total_bytes, last_reconciled_at, updated_at,
                        content_verification_required
                 FROM project_index_state WHERE project_id = ?1",
                [project_id],
                |row| {
                    Ok(ProjectIndexState {
                        project_id: row.get(0)?,
                        generation: row.get(1)?,
                        baseline_generation: row.get(2)?,
                        status: row.get(3)?,
                        content_verification_required: row.get::<_, i64>(8)? != 0,
                        file_count: row.get::<_, i64>(4)?.max(0) as u64,
                        total_bytes: row.get::<_, i64>(5)?.max(0) as u64,
                        last_reconciled_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read project index state", error))
    }

    pub fn mark_project_index_stale(&self, project_id: &str) -> Result<(), StorageError> {
        self.get_active_project(project_id)?;
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin index continuity loss", error))?;
        tx.execute(
            "UPDATE project_index_state
             SET status = 'stale', content_verification_required = 1, updated_at = ?2
             WHERE project_id = ?1",
            params![project_id, now],
        )
        .map_err(|error| StorageError::sqlite("mark project index stale", error))?;
        tx.execute(
            "DELETE FROM project_dependency_coverage WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| {
            StorageError::sqlite("invalidate coverage after continuity loss", error)
        })?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit index continuity loss", error))?;
        Ok(())
    }

    pub fn list_project_files(
        &self,
        project_id: &str,
    ) -> Result<Vec<IndexedFileSnapshot>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT relative_path, size_bytes, modified_unix_ns,
                        content_sha256
                 FROM project_files
                 WHERE project_id = ?1
                 ORDER BY relative_path",
            )
            .map_err(|error| StorageError::sqlite("prepare project file list", error))?;
        let rows = statement
            .query_map([project_id], |row| {
                Ok(IndexedFileSnapshot {
                    relative_path: row.get(0)?,
                    size_bytes: row.get::<_, i64>(1)?.max(0) as u64,
                    modified_unix_ns: row.get::<_, i64>(2)?.max(0) as u64,
                    content_sha256: row.get(3)?,
                })
            })
            .map_err(|error| StorageError::sqlite("query project file list", error))?;

        let mut files = Vec::new();
        for row in rows {
            files
                .push(row.map_err(|error| StorageError::sqlite("decode project file row", error))?);
        }
        Ok(files)
    }

    fn list_project_files_bounded(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<IndexedFileSnapshot>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT relative_path, size_bytes, modified_unix_ns, content_sha256
             FROM project_files WHERE project_id = ?1
             ORDER BY relative_path LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare bounded project files", error))?;
        let rows = statement
            .query_map(
                params![project_id, i64::try_from(limit).unwrap_or(i64::MAX)],
                |row| {
                    Ok(IndexedFileSnapshot {
                        relative_path: row.get(0)?,
                        size_bytes: row.get::<_, i64>(1)?.max(0) as u64,
                        modified_unix_ns: row.get::<_, i64>(2)?.max(0) as u64,
                        content_sha256: row.get(3)?,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query bounded project files", error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| StorageError::sqlite("decode bounded project files", error))
    }

    pub fn project_file_sha256(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<Option<String>, StorageError> {
        self.conn.query_row(
            "SELECT content_sha256 FROM project_files WHERE project_id = ?1 AND relative_path = ?2",
            params![project_id, relative_path],
            |row| row.get(0),
        ).optional().map_err(|error| StorageError::sqlite("read indexed parser source", error))
    }

    pub fn project_files_for_paths(
        &self,
        project_id: &str,
        relative_paths: &[String],
    ) -> Result<Vec<IndexedFileSnapshot>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT relative_path, size_bytes, modified_unix_ns, content_sha256
             FROM project_files
             WHERE project_id = ?1 AND relative_path = ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare hinted file lookup", error))?;
        let mut files = Vec::new();
        for relative_path in relative_paths {
            let file = statement
                .query_row(params![project_id, relative_path], |row| {
                    Ok(IndexedFileSnapshot {
                        relative_path: row.get(0)?,
                        size_bytes: row.get::<_, i64>(1)?.max(0) as u64,
                        modified_unix_ns: row.get::<_, i64>(2)?.max(0) as u64,
                        content_sha256: row.get(3)?,
                    })
                })
                .optional()
                .map_err(|error| StorageError::sqlite("read hinted file", error))?;
            if let Some(file) = file {
                files.push(file);
            }
        }
        Ok(files)
    }

    pub fn replace_project_baseline(
        &self,
        project_id: &str,
        files: &[IndexedFileSnapshot],
        total_bytes: u64,
    ) -> Result<ProjectIndexState, StorageError> {
        self.get_active_project(project_id)?;
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin project baseline transaction", error))?;

        let project_exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(|error| StorageError::sqlite("verify project for baseline", error))?;
        if project_exists == 0 {
            return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
        }

        let current_generation: Option<i64> = tx
            .query_row(
                "SELECT generation FROM project_index_state
                 WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read baseline generation", error))?;
        let generation = current_generation.unwrap_or(0).saturating_add(1);

        tx.execute(
            "DELETE FROM project_files WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior project files", error))?;
        tx.execute(
            "DELETE FROM project_changes WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior project changes", error))?;
        tx.execute(
            "DELETE FROM project_dependency_edges WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior project dependencies", error))?;
        tx.execute(
            "DELETE FROM project_dependency_coverage WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior parser coverage", error))?;
        tx.execute(
            "DELETE FROM project_dependency_edge_invalidations WHERE project_id = ?1",
            [project_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior edge history", error))?;

        for file in files {
            tx.execute(
                "INSERT INTO project_files(
                    project_id, relative_path, size_bytes,
                    modified_unix_ns, content_sha256, indexed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    project_id,
                    file.relative_path,
                    sql_i64(file.size_bytes),
                    sql_i64(file.modified_unix_ns),
                    file.content_sha256,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("insert baseline file", error))?;
        }

        tx.execute(
            "INSERT INTO project_index_state(
                project_id, generation, baseline_generation, status, file_count,
                total_bytes, last_reconciled_at, updated_at, content_verification_required
             ) VALUES (?1, ?2, ?2, 'ready', ?3, ?4, ?5, ?5, 0)
             ON CONFLICT(project_id) DO UPDATE SET
               generation=excluded.generation,
               baseline_generation=excluded.baseline_generation,
               status=excluded.status,
               content_verification_required=0,
               file_count=excluded.file_count,
               total_bytes=excluded.total_bytes,
               last_reconciled_at=excluded.last_reconciled_at,
               updated_at=excluded.updated_at",
            params![
                project_id,
                generation,
                i64::try_from(files.len()).unwrap_or(i64::MAX),
                sql_i64(total_bytes),
                now,
            ],
        )
        .map_err(|error| StorageError::sqlite("write project baseline state", error))?;

        tx.commit()
            .map_err(|error| StorageError::sqlite("commit project baseline", error))?;
        self.get_project_index_state(project_id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "project index state disappeared after baseline write",
            )
        })
    }

    pub fn apply_project_reconciliation(
        &self,
        commit: ProjectIndexCommit<'_>,
    ) -> Result<ProjectIndexState, StorageError> {
        let ProjectIndexCommit {
            project_id,
            expected_generation,
            touched_files,
            changes,
            file_count,
            total_bytes,
            mode,
            content_verified,
        } = commit;
        self.get_active_project(project_id)?;
        let status = match mode {
            IndexCommitMode::Authoritative => "ready",
            IndexCommitMode::HintsOnly => "stale",
        };
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin project reconciliation", error))?;

        let current_state: Option<(i64, bool)> = tx
            .query_row(
                "SELECT generation, content_verification_required FROM project_index_state
                 WHERE project_id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read reconciliation generation", error))?;

        let Some((current_generation, verification_required)) = current_state else {
            return Err(StorageError::new(
                "INDEX_BASELINE_MISSING",
                "project baseline is missing",
            ));
        };
        if current_generation != expected_generation {
            return Err(StorageError::new(
                "INDEX_GENERATION_CONFLICT",
                format!(
                    "expected project index generation {expected_generation}, found {current_generation}"
                ),
            ));
        }
        if matches!(mode, IndexCommitMode::Authoritative)
            && verification_required
            && !content_verified
        {
            return Err(StorageError::new(
                "INDEX_CONTENT_VERIFICATION_REQUIRED",
                "full content verification is required after index continuity loss",
            ));
        }
        let next_generation = current_generation.saturating_add(1);

        for change in changes {
            for path in [
                Some(change.relative_path.as_str()),
                change.previous_path.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                tx.execute(
                    "INSERT OR IGNORE INTO project_dependency_edge_invalidations(
                       project_id, generation, source_path, target_path,
                       source_sha256, producer_id, producer_version
                     ) SELECT project_id, ?3, source_path, target_path,
                              source_sha256, producer_id, producer_version
                       FROM project_dependency_edges
                       WHERE project_id = ?1 AND (source_path = ?2 OR target_path = ?2)",
                    params![project_id, path, next_generation],
                )
                .map_err(|error| {
                    StorageError::sqlite("record invalidated dependency edges", error)
                })?;
                tx.execute(
                    "DELETE FROM project_dependency_coverage
                     WHERE project_id = ?1 AND source_path IN (
                       SELECT source_path FROM project_dependency_edges
                       WHERE project_id = ?1 AND (source_path = ?2 OR target_path = ?2)
                     )",
                    params![project_id, path],
                )
                .map_err(|error| {
                    StorageError::sqlite("invalidate parser coverage for changed edge", error)
                })?;
                tx.execute(
                    "DELETE FROM project_dependency_coverage
                     WHERE project_id = ?1 AND source_path = ?2",
                    params![project_id, path],
                )
                .map_err(|error| {
                    StorageError::sqlite("invalidate changed source parser coverage", error)
                })?;
                tx.execute(
                    "DELETE FROM project_dependency_edges
                     WHERE project_id = ?1
                       AND (source_path = ?2 OR target_path = ?2)",
                    params![project_id, path],
                )
                .map_err(|error| {
                    StorageError::sqlite("invalidate changed dependency edges", error)
                })?;
            }
            match change.change_kind.as_str() {
                "deleted" => {
                    tx.execute(
                        "DELETE FROM project_files
                         WHERE project_id = ?1 AND relative_path = ?2",
                        params![project_id, change.relative_path],
                    )
                    .map_err(|error| StorageError::sqlite("delete indexed file", error))?;
                }
                "renamed" => {
                    if let Some(previous_path) = &change.previous_path {
                        tx.execute(
                            "DELETE FROM project_files
                             WHERE project_id = ?1 AND relative_path = ?2",
                            params![project_id, previous_path],
                        )
                        .map_err(|error| {
                            StorageError::sqlite("remove renamed indexed path", error)
                        })?;
                    }
                }
                _ => {}
            }
        }

        for file in touched_files {
            tx.execute(
                "INSERT INTO project_files(
                    project_id, relative_path, size_bytes,
                    modified_unix_ns, content_sha256, indexed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(project_id, relative_path) DO UPDATE SET
                   size_bytes=excluded.size_bytes,
                   modified_unix_ns=excluded.modified_unix_ns,
                   content_sha256=excluded.content_sha256,
                   indexed_at=excluded.indexed_at",
                params![
                    project_id,
                    file.relative_path,
                    sql_i64(file.size_bytes),
                    sql_i64(file.modified_unix_ns),
                    file.content_sha256,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("upsert reconciled file", error))?;
        }

        for change in changes {
            tx.execute(
                "INSERT INTO project_changes(
                    id, project_id, generation, change_kind,
                    relative_path, previous_path,
                    before_sha256, after_sha256, detected_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    opaque_id("CHG"),
                    project_id,
                    next_generation,
                    change.change_kind,
                    change.relative_path,
                    change.previous_path,
                    change.before_sha256,
                    change.after_sha256,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("record project change", error))?;
        }

        tx.execute(
            "UPDATE project_index_state
             SET generation = ?2,
                 status = ?5,
                 content_verification_required = CASE
                   WHEN ?5 = 'ready' THEN 0 ELSE content_verification_required END,
                 file_count = ?3,
                 total_bytes = ?4,
                 last_reconciled_at = CASE
                   WHEN ?5 = 'ready' THEN ?6 ELSE last_reconciled_at END,
                 updated_at = ?6
             WHERE project_id = ?1",
            params![
                project_id,
                next_generation,
                i64::try_from(file_count).unwrap_or(i64::MAX),
                sql_i64(total_bytes),
                status,
                now,
            ],
        )
        .map_err(|error| StorageError::sqlite("update project index state", error))?;

        tx.commit()
            .map_err(|error| StorageError::sqlite("commit project reconciliation", error))?;
        self.get_project_index_state(project_id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "project index state disappeared after reconciliation",
            )
        })
    }

    pub fn list_project_changes(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<ProjectChangeRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, project_id, generation, change_kind,
                    relative_path, previous_path,
                    before_sha256, after_sha256, detected_at
             FROM project_changes
             WHERE project_id = ?1
             ORDER BY generation DESC, detected_at DESC, id DESC
             LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare project change list", error))?;
        let rows = statement
            .query_map(
                params![project_id, i64::try_from(limit).unwrap_or(i64::MAX)],
                |row| {
                    Ok(ProjectChangeRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        generation: row.get(2)?,
                        change_kind: row.get(3)?,
                        relative_path: row.get(4)?,
                        previous_path: row.get(5)?,
                        before_sha256: row.get(6)?,
                        after_sha256: row.get(7)?,
                        detected_at: row.get(8)?,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query project change list", error))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(
                row.map_err(|error| StorageError::sqlite("decode project change row", error))?,
            );
        }
        Ok(records)
    }

    pub fn list_project_changes_since(
        &self,
        project_id: &str,
        after_generation: i64,
        limit: usize,
    ) -> Result<Vec<ProjectChangeRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, project_id, generation, change_kind,
                    relative_path, previous_path,
                    before_sha256, after_sha256, detected_at
             FROM project_changes
             WHERE project_id = ?1 AND generation > ?2
             ORDER BY generation, relative_path, id
             LIMIT ?3",
            )
            .map_err(|error| StorageError::sqlite("prepare project changes since", error))?;
        let rows = statement
            .query_map(
                params![
                    project_id,
                    after_generation,
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok(ProjectChangeRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        generation: row.get(2)?,
                        change_kind: row.get(3)?,
                        relative_path: row.get(4)?,
                        previous_path: row.get(5)?,
                        before_sha256: row.get(6)?,
                        after_sha256: row.get(7)?,
                        detected_at: row.get(8)?,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query project changes since", error))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(
                row.map_err(|error| StorageError::sqlite("decode project change since", error))?,
            );
        }
        Ok(records)
    }

    pub fn replace_project_dependencies(
        &self,
        replacement: DependencyReplacement<'_>,
    ) -> Result<usize, StorageError> {
        self.get_active_project(replacement.project_id)?;
        let DependencyReplacement {
            project_id,
            expected_generation,
            source_path,
            expected_source_sha256,
            producer_id,
            producer_version,
            configuration_guard,
            targets,
        } = replacement;
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin dependency replacement", error))?;
        let index_state: Option<(i64, String)> = tx
            .query_row(
                "SELECT generation, status FROM project_index_state WHERE project_id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read dependency index generation", error))?;
        let Some((generation, status)) = index_state else {
            return Err(StorageError::new(
                "INDEX_BASELINE_MISSING",
                "project baseline is missing",
            ));
        };
        if status != "ready" {
            return Err(StorageError::new(
                "INDEX_RECONCILIATION_REQUIRED",
                "dependency edges require authoritative reconciliation",
            ));
        }
        if generation != expected_generation {
            return Err(StorageError::new(
                "INDEX_GENERATION_CONFLICT",
                format!("expected generation {expected_generation}, found {generation}"),
            ));
        }
        if let Some((expected_revision, expected_adapter_id, expected_adapter_version)) =
            configuration_guard
        {
            let selected: Option<(i64, Option<String>, Option<String>)> = tx.query_row(
                "SELECT revision, adapter_id, adapter_version FROM project_configuration WHERE project_id = ?1",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional().map_err(|error| StorageError::sqlite("read parser selection", error))?;
            if !selected.is_some_and(|(revision, id, version)| {
                revision == expected_revision
                    && id.as_deref() == Some(expected_adapter_id)
                    && version.as_deref() == Some(expected_adapter_version)
                    && producer_id == expected_adapter_id
                    && producer_version == expected_adapter_version
            }) {
                return Err(StorageError::new(
                    "PROJECT_CONFIG_CONFLICT",
                    "project parser selection changed before dependency replacement",
                ));
            }
        }
        let source_sha256: Option<String> = tx
            .query_row(
                "SELECT content_sha256 FROM project_files
             WHERE project_id = ?1 AND relative_path = ?2",
                params![project_id, source_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read dependency source", error))?;
        let Some(source_sha256) = source_sha256 else {
            return Err(StorageError::new(
                "INDEX_FILE_NOT_FOUND",
                "dependency source is not indexed",
            ));
        };
        if source_sha256 != expected_source_sha256 {
            return Err(StorageError::new(
                "INDEX_SOURCE_CHANGED",
                "dependency source hash does not match the indexed file",
            ));
        }
        for target in targets {
            let found: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM project_files
                 WHERE project_id = ?1 AND relative_path = ?2",
                    params![project_id, target],
                    |row| row.get(0),
                )
                .map_err(|error| StorageError::sqlite("verify dependency target", error))?;
            if found == 0 {
                return Err(StorageError::new(
                    "INDEX_FILE_NOT_FOUND",
                    "dependency target is not indexed",
                ));
            }
        }

        tx.execute(
            "INSERT OR IGNORE INTO project_dependency_edge_invalidations(
               project_id, generation, source_path, target_path,
               source_sha256, producer_id, producer_version
             ) SELECT project_id, ?4, source_path, target_path,
                      source_sha256, producer_id, producer_version
               FROM project_dependency_edges
               WHERE project_id = ?1 AND source_path = ?2 AND producer_id = ?3",
            params![project_id, source_path, producer_id, generation],
        )
        .map_err(|error| StorageError::sqlite("record replaced dependency edges", error))?;
        tx.execute(
            "DELETE FROM project_dependency_edges
             WHERE project_id = ?1 AND source_path = ?2 AND producer_id = ?3",
            params![project_id, source_path, producer_id],
        )
        .map_err(|error| StorageError::sqlite("clear prior source dependency edges", error))?;
        for target in targets {
            tx.execute(
                "INSERT INTO project_dependency_edges(
                    project_id, source_path, target_path, source_sha256,
                    producer_id, producer_version, indexed_generation, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    project_id,
                    source_path,
                    target,
                    source_sha256,
                    producer_id,
                    producer_version,
                    generation,
                    now
                ],
            )
            .map_err(|error| StorageError::sqlite("insert dependency edge", error))?;
        }
        tx.execute(
            "INSERT INTO project_dependency_coverage(
               project_id, source_path, source_sha256, producer_id,
               producer_version, indexed_generation, configuration_revision,
               target_count, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(project_id, source_path, producer_id) DO UPDATE SET
               source_sha256=excluded.source_sha256,
               producer_version=excluded.producer_version,
               indexed_generation=excluded.indexed_generation,
               configuration_revision=excluded.configuration_revision,
               target_count=excluded.target_count,
               updated_at=excluded.updated_at",
            params![
                project_id,
                source_path,
                source_sha256,
                producer_id,
                producer_version,
                generation,
                configuration_guard.map(|(revision, _, _)| revision),
                i64::try_from(targets.len()).unwrap_or(i64::MAX),
                now,
            ],
        )
        .map_err(|error| StorageError::sqlite("record parser coverage", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit dependency replacement", error))?;
        Ok(targets.len())
    }

    pub fn list_project_dependency_edges(
        &self,
        project_id: &str,
        source_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DependencyEdgeRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT project_id, source_path, target_path, source_sha256,
                    producer_id, producer_version, indexed_generation
             FROM project_dependency_edges
             WHERE project_id = ?1 AND (?2 IS NULL OR source_path = ?2)
             ORDER BY source_path, target_path, producer_id
             LIMIT ?3",
            )
            .map_err(|error| StorageError::sqlite("prepare dependency edge list", error))?;
        let rows = statement
            .query_map(
                params![
                    project_id,
                    source_path,
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok(DependencyEdgeRecord {
                        project_id: row.get(0)?,
                        source_path: row.get(1)?,
                        target_path: row.get(2)?,
                        source_sha256: row.get(3)?,
                        producer_id: row.get(4)?,
                        producer_version: row.get(5)?,
                        indexed_generation: row.get(6)?,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query dependency edges", error))?;
        let mut edges = Vec::new();
        for row in rows {
            edges.push(row.map_err(|error| StorageError::sqlite("decode dependency edge", error))?);
        }
        Ok(edges)
    }

    pub fn list_project_dependency_coverage(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<DependencyCoverageRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT project_id, source_path, source_sha256, producer_id,
                    producer_version, indexed_generation, configuration_revision, target_count
             FROM project_dependency_coverage WHERE project_id = ?1
             ORDER BY source_path, producer_id LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare parser coverage list", error))?;
        let rows = statement
            .query_map(
                params![project_id, i64::try_from(limit).unwrap_or(i64::MAX)],
                |row| {
                    Ok(DependencyCoverageRecord {
                        project_id: row.get(0)?,
                        source_path: row.get(1)?,
                        source_sha256: row.get(2)?,
                        producer_id: row.get(3)?,
                        producer_version: row.get(4)?,
                        indexed_generation: row.get(5)?,
                        configuration_revision: row.get(6)?,
                        target_count: row.get::<_, i64>(7)?.max(0) as u32,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query parser coverage", error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| StorageError::sqlite("decode parser coverage", error))
    }

    pub fn list_project_dependency_invalidations_since(
        &self,
        project_id: &str,
        after_generation: i64,
        limit: usize,
    ) -> Result<Vec<InvalidatedDependencyEdgeRecord>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT project_id, generation, source_path, target_path, source_sha256,
                    producer_id, producer_version
             FROM project_dependency_edge_invalidations
             WHERE project_id = ?1 AND generation > ?2
             ORDER BY generation, source_path, target_path, producer_id LIMIT ?3",
            )
            .map_err(|error| StorageError::sqlite("prepare dependency invalidation list", error))?;
        let rows = statement
            .query_map(
                params![
                    project_id,
                    after_generation,
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok(InvalidatedDependencyEdgeRecord {
                        project_id: row.get(0)?,
                        generation: row.get(1)?,
                        source_path: row.get(2)?,
                        target_path: row.get(3)?,
                        source_sha256: row.get(4)?,
                        producer_id: row.get(5)?,
                        producer_version: row.get(6)?,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("query dependency invalidations", error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| StorageError::sqlite("decode dependency invalidations", error))
    }

    pub fn put_project_check_catalog(
        &self,
        project_id: &str,
        expected_revision: i64,
        catalog: &Value,
    ) -> Result<ProjectCheckCatalogRecord, StorageError> {
        self.get_active_project(project_id)?;
        let catalog_json = json_text(catalog)?;
        let catalog_sha256 = sha256_hex(catalog_json.as_bytes());
        let now = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin check catalog update", error))?;
        let index_generation: i64 = tx
            .query_row(
                "SELECT generation FROM project_index_state WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read check catalog index generation", error))?
            .ok_or_else(|| {
                StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
            })?;
        let configuration_revision: Option<i64> = tx
            .query_row(
                "SELECT revision FROM project_configuration WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                StorageError::sqlite("read check catalog configuration revision", error)
            })?;
        let current_revision: Option<i64> = tx
            .query_row(
                "SELECT revision FROM project_check_catalog WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read check catalog revision", error))?;
        if current_revision.unwrap_or(0) != expected_revision {
            return Err(StorageError::new(
                "CHECK_CATALOG_CONFLICT",
                "check catalog revision changed",
            ));
        }
        let revision = expected_revision.checked_add(1).ok_or_else(|| {
            StorageError::new("CHECK_CATALOG_CONFLICT", "check catalog revision exhausted")
        })?;
        tx.execute(
            "INSERT INTO project_check_catalog(project_id, revision, index_generation,
               configuration_revision, catalog_json, catalog_sha256, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(project_id) DO UPDATE SET
               revision=excluded.revision,
               index_generation=excluded.index_generation,
               configuration_revision=excluded.configuration_revision,
               catalog_json=excluded.catalog_json,
               catalog_sha256=excluded.catalog_sha256,
               updated_at=excluded.updated_at",
            params![
                project_id,
                revision,
                index_generation,
                configuration_revision,
                catalog_json,
                catalog_sha256,
                now
            ],
        )
        .map_err(|error| StorageError::sqlite("write check catalog", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit check catalog update", error))?;
        self.get_project_check_catalog(project_id)?.ok_or_else(|| {
            StorageError::new("STORAGE_ERROR", "check catalog disappeared after update")
        })
    }

    pub fn get_project_check_catalog(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectCheckCatalogRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT project_id, revision, index_generation, configuration_revision,
                    catalog_json, catalog_sha256
             FROM project_check_catalog WHERE project_id = ?1",
                [project_id],
                |row| {
                    let catalog_json: String = row.get(4)?;
                    let catalog_sha256: String = row.get(5)?;
                    if sha256_hex(catalog_json.as_bytes()) != catalog_sha256 {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "check catalog digest mismatch",
                            )),
                        ));
                    }
                    Ok(ProjectCheckCatalogRecord {
                        project_id: row.get(0)?,
                        revision: row.get(1)?,
                        index_generation: row.get(2)?,
                        configuration_revision: row.get(3)?,
                        catalog: parse_json(&catalog_json, 4)?,
                        catalog_sha256,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read check catalog", error))
    }

    pub fn project_plan_snapshot(
        &self,
        project_id: &str,
        after_generation: i64,
        max_items: usize,
    ) -> Result<ProjectPlanSnapshot, StorageError> {
        self.get_active_project(project_id)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin check plan snapshot", error))?;
        let state = self.get_project_index_state(project_id)?.ok_or_else(|| {
            StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
        })?;
        let configuration = self.get_project_configuration(project_id)?;
        let catalog = self.get_project_check_catalog(project_id)?.ok_or_else(|| {
            StorageError::new("CHECK_CATALOG_MISSING", "project check catalog is missing")
        })?;
        let changes =
            self.list_project_changes_since(project_id, after_generation, max_items + 1)?;
        let edges = self.list_project_dependency_edges(project_id, None, max_items + 1)?;
        let invalidations = self.list_project_dependency_invalidations_since(
            project_id,
            after_generation,
            max_items + 1,
        )?;
        let coverage = self.list_project_dependency_coverage(project_id, max_items + 1)?;
        let files = if state.file_count <= max_items as u64 {
            self.list_project_files_bounded(project_id, max_items + 1)?
        } else {
            Vec::new()
        };
        let bounds_exceeded = state.file_count > max_items as u64
            || files.len() > max_items
            || changes.len() > max_items
            || edges.len() > max_items
            || invalidations.len() > max_items
            || coverage.len() > max_items;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit check plan snapshot", error))?;
        Ok(ProjectPlanSnapshot {
            state,
            configuration,
            catalog,
            files,
            changes,
            edges,
            invalidations,
            coverage,
            bounds_exceeded,
        })
    }

    /// Commit every selected deterministic check result and the replay marker
    /// in one SQLite transaction after rechecking the observed plan boundary.
    pub fn commit_check_execution(
        &self,
        snapshot: &ProjectPlanSnapshot,
        planned: &Value,
        after_generation: i64,
        prepared: &[PreparedCheckExecution],
        producer_version: &str,
        provenance: &Value,
        trust: &str,
    ) -> Result<Value, StorageError> {
        let project_id = snapshot.state.project_id.as_str();
        let plan_id = planned["plan_id"]
            .as_str()
            .ok_or_else(|| StorageError::new("CHECK_PLAN_CONFLICT", "plan identity is missing"))?;
        let suffix = plan_id
            .strip_prefix("PLAN-")
            .filter(|value| value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| StorageError::new("CHECK_PLAN_CONFLICT", "plan identity is invalid"))?;
        let selected = planned["checks"]
            .as_array()
            .ok_or_else(|| StorageError::new("CHECK_PLAN_CONFLICT", "plan checks are missing"))?;
        if selected.len() != prepared.len()
            || prepared.len() > 128
            || selected.iter().zip(prepared).any(|(entry, check)| {
                entry["check_id"] != check.check_id
                    || entry["planned_check_id"] != check.planned_check_id
                    || (check.status == "untested") != check.result_payload.is_none()
            })
        {
            return Err(StorageError::new(
                "CHECK_PLAN_CONFLICT",
                "prepared checks differ from plan",
            ));
        }
        let job_id = format!("JOB-{suffix}");
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| StorageError::sqlite("begin check execution", error))?;
        let existing: Option<(String, String, String)> = tx
            .query_row(
                "SELECT project_id, command, checkpoint_json FROM jobs WHERE id = ?1",
                [&job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read check execution replay", error))?;
        if let Some((owner, command, checkpoint_json)) = existing {
            let mut response: Value = serde_json::from_str(&checkpoint_json).map_err(|_| {
                StorageError::new("STORAGE_ERROR", "check execution replay is damaged")
            })?;
            if owner != project_id
                || command != "automation.checks.execute"
                || response["plan_id"] != plan_id
            {
                return Err(StorageError::new(
                    "CHECK_PLAN_CONFLICT",
                    "check execution identity conflicts",
                ));
            }
            response["replayed"] = json!(true);
            tx.commit()
                .map_err(|error| StorageError::sqlite("finish check execution replay", error))?;
            return Ok(response);
        }
        let guard: Option<(
            String,
            i64,
            i64,
            String,
            bool,
            i64,
            i64,
            String,
            Option<i64>,
            Option<i64>,
        )> = tx
            .query_row(
                "SELECT p.lifecycle_state, i.generation, i.baseline_generation, i.status,
                        i.content_verification_required, c.revision, c.index_generation,
                        c.catalog_sha256, c.configuration_revision,
                        (SELECT revision FROM project_configuration WHERE project_id = p.id)
                 FROM projects p
                 JOIN project_index_state i ON i.project_id = p.id
                 JOIN project_check_catalog c ON c.project_id = p.id
                 WHERE p.id = ?1",
                [project_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read check execution guard", error))?;
        let Some((
            lifecycle,
            generation,
            baseline,
            index_status,
            verification_required,
            catalog_revision,
            catalog_index_generation,
            catalog_sha256,
            catalog_configuration_revision,
            live_configuration_revision,
        )) = guard
        else {
            return Err(StorageError::new(
                "CHECK_PLAN_CONFLICT",
                "check execution state is missing",
            ));
        };
        if lifecycle != "active"
            || index_status != "ready"
            || verification_required
            || generation != snapshot.state.generation
            || after_generation < baseline
            || catalog_revision != snapshot.catalog.revision
            || catalog_index_generation != snapshot.catalog.index_generation
            || catalog_sha256 != snapshot.catalog.catalog_sha256
            || catalog_configuration_revision != snapshot.catalog.configuration_revision
            || live_configuration_revision
                != snapshot.configuration.as_ref().map(|row| row.revision)
            || prepared.len() > 128
        {
            return Err(StorageError::new(
                "CHECK_PLAN_CONFLICT",
                "check execution plan changed",
            ));
        }
        let now = sqlite_now(&self.conn)?;
        let provenance_json = json_text(provenance)?;
        let mut checks = Vec::with_capacity(prepared.len());
        let mut passed_count = 0usize;
        let mut failed_count = 0usize;
        let mut untested_count = 0usize;
        for check in prepared {
            let result_id = if let Some(payload) = &check.result_payload {
                let id = opaque_id("RES");
                let payload_json = json_text(payload)?;
                if payload_json.len() > 4096 {
                    return Err(StorageError::new(
                        "CHECK_RESULT_TOO_LARGE",
                        "check result exceeds limit",
                    ));
                }
                let payload_sha256 = sha256_hex(payload_json.as_bytes());
                tx.execute(
                    "INSERT INTO results(id, project_id, kind, schema_version,
                        producer_version, payload_json, payload_sha256, created_at,
                        provenance_json, trust)
                     VALUES (?1, ?2, 'DECLARED_INDEX_CHECK', 1, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id,
                        project_id,
                        producer_version,
                        payload_json,
                        payload_sha256,
                        now,
                        provenance_json,
                        trust
                    ],
                )
                .map_err(|error| StorageError::sqlite("write check result", error))?;
                Some(id)
            } else {
                None
            };
            match check.status {
                "passed" => passed_count += 1,
                "failed" => failed_count += 1,
                "untested" => untested_count += 1,
                _ => {
                    return Err(StorageError::new(
                        "STORAGE_ERROR",
                        "check status is invalid",
                    ));
                }
            }
            checks.push(json!({
                "check_id": check.check_id,
                "planned_check_id": check.planned_check_id,
                "status": check.status,
                "reason_code": check.reason_code,
                "result_id": result_id,
            }));
        }
        let response = json!({
            "project_id": project_id,
            "plan_id": plan_id,
            "job_id": job_id,
            "index_generation": generation,
            "evidence_scope": "indexed_snapshot_at_generation",
            "mode": planned["mode"],
            "fallback_reason": planned["fallback_reason"],
            "selected_check_count": prepared.len(),
            "passed_count": passed_count,
            "failed_count": failed_count,
            "untested_count": untested_count,
            "replayed": false,
            "checks": checks,
        });
        let checkpoint_json = json_text(&response)?;
        tx.execute(
            "INSERT INTO jobs(id, project_id, command, state, checkpoint_json,
                result_id, created_at, updated_at, provenance_json, trust)
             VALUES (?1, ?2, 'automation.checks.execute', 'completed', ?3,
                NULL, ?4, ?4, ?5, ?6)",
            params![
                job_id,
                project_id,
                checkpoint_json,
                now,
                provenance_json,
                trust
            ],
        )
        .map_err(|error| StorageError::sqlite("write check execution job", error))?;
        tx.commit()
            .map_err(|error| StorageError::sqlite("commit check execution", error))?;
        Ok(response)
    }

    pub fn put_result(
        &self,
        project_id: Option<&str>,
        kind: &str,
        payload: &Value,
        producer_version: &str,
        provenance: &Value,
        trust: &str,
    ) -> Result<ResultRecord, StorageError> {
        if let Some(project_id) = project_id {
            self.get_active_project(project_id)?;
        }
        let id = opaque_id("RES");
        let payload_json = json_text(payload)?;
        let payload_sha256 = sha256_hex(payload_json.as_bytes());
        let provenance_json = json_text(provenance)?;
        let created_at = sqlite_now(&self.conn)?;
        self.conn
            .execute(
                "INSERT INTO results(
                    id, project_id, kind, schema_version,
                    producer_version, payload_json, payload_sha256,
                    created_at, provenance_json, trust
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 )",
                params![
                    id,
                    project_id,
                    kind,
                    STORAGE_SCHEMA_VERSION,
                    producer_version,
                    payload_json,
                    payload_sha256,
                    created_at,
                    provenance_json,
                    trust
                ],
            )
            .map_err(|error| StorageError::sqlite("store result", error))?;

        self.get_result(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "result disappeared after write"))
    }
    pub fn get_result(&self, id: &str) -> Result<Option<ResultRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, kind, schema_version,
                        producer_version, payload_json, payload_sha256,
                        provenance_json, trust, created_at
                 FROM results WHERE id = ?1",
                [id],
                |row| {
                    let payload_json: String = row.get(5)?;
                    let payload_sha256: String = row.get(6)?;
                    if sha256_hex(payload_json.as_bytes()) != payload_sha256 {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "result payload digest mismatch",
                            )),
                        ));
                    }
                    let provenance_json: String = row.get(7)?;
                    Ok(ResultRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        schema_version: row.get(3)?,
                        producer_version: row.get(4)?,
                        payload: parse_json(&payload_json, 5)?,
                        payload_sha256,
                        provenance: parse_json(&provenance_json, 7)?,
                        trust: row.get(8)?,
                        created_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read result", error))
    }

    pub fn describe_result(&self, id: &str) -> Result<Option<ResultDescription>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, kind, schema_version,
                        producer_version, payload_sha256,
                        length(CAST(payload_json AS BLOB)), trust, created_at
                 FROM results WHERE id = ?1",
                [id],
                |row| {
                    Ok(ResultDescription {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        schema_version: row.get(3)?,
                        producer_version: row.get(4)?,
                        payload_sha256: row.get(5)?,
                        payload_bytes: row.get(6)?,
                        trust: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("describe result", error))
    }

    /// Return metadata only; payload and provenance remain in result.get.
    pub fn list_result_descriptions(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ResultDescription>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, project_id, kind, schema_version,
                    producer_version, payload_sha256,
                    length(CAST(payload_json AS BLOB)), trust, created_at
             FROM results
             WHERE (?1 IS NULL OR project_id = ?1)
             ORDER BY created_at DESC, id DESC LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare result list", error))?;
        let rows = statement
            .query_map(params![project_id, limit as i64], |row| {
                Ok(ResultDescription {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    kind: row.get(2)?,
                    schema_version: row.get(3)?,
                    producer_version: row.get(4)?,
                    payload_sha256: row.get(5)?,
                    payload_bytes: row.get(6)?,
                    trust: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(|error| StorageError::sqlite("query result list", error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| StorageError::sqlite("decode result list", error))
    }

    pub fn checkpoint_job(
        &self,
        id: Option<&str>,
        project_id: Option<&str>,
        command: &str,
        state: &str,
        checkpoint: &Value,
        result_id: Option<&str>,
        provenance: &Value,
        trust: &str,
    ) -> Result<JobRecord, StorageError> {
        if let Some(project_id) = project_id {
            self.get_active_project(project_id)?;
        }
        if let Some(id) = id
            && let Some(existing) = self.get_job(id)?
            && let Some(project_id) = existing.project_id.as_deref()
        {
            self.get_active_project(project_id)?;
        }
        let id = id
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| opaque_id("JOB"));
        let checkpoint_json = json_text(checkpoint)?;
        let provenance_json = json_text(provenance)?;
        let now = sqlite_now(&self.conn)?;

        self.conn
            .execute(
                "INSERT INTO jobs(
                    id, project_id, command, state, checkpoint_json,
                    result_id, created_at, updated_at,
                    provenance_json, trust
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 )
                 ON CONFLICT(id) DO UPDATE SET
                   state=excluded.state,
                   checkpoint_json=excluded.checkpoint_json,
                   result_id=excluded.result_id,
                   updated_at=excluded.updated_at,
                   provenance_json=excluded.provenance_json,
                   trust=excluded.trust",
                params![
                    id,
                    project_id,
                    command,
                    state,
                    checkpoint_json,
                    result_id,
                    now,
                    now,
                    provenance_json,
                    trust
                ],
            )
            .map_err(|error| StorageError::sqlite("checkpoint job", error))?;

        self.get_job(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "job disappeared after write"))
    }

    pub fn get_job(&self, id: &str) -> Result<Option<JobRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, command, state,
                        checkpoint_json, result_id,
                        provenance_json, trust,
                        created_at, updated_at
                 FROM jobs WHERE id = ?1",
                [id],
                |row| {
                    let checkpoint_json: String = row.get(4)?;
                    let provenance_json: String = row.get(6)?;
                    Ok(JobRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        command: row.get(2)?,
                        state: row.get(3)?,
                        checkpoint: parse_json(&checkpoint_json, 4)?,
                        result_id: row.get(5)?,
                        provenance: parse_json(&provenance_json, 6)?,
                        trust: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read job", error))
    }

    pub fn list_job_descriptions(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<JobDescription>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, project_id, command, state, result_id, trust, created_at, updated_at
             FROM jobs WHERE (?1 IS NULL OR project_id = ?1)
             ORDER BY updated_at DESC, id DESC LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare job list", error))?;
        let rows = statement
            .query_map(params![project_id, limit as i64], |row| {
                Ok(JobDescription {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    command: row.get(2)?,
                    state: row.get(3)?,
                    result_id: row.get(4)?,
                    trust: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|error| StorageError::sqlite("query job list", error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| StorageError::sqlite("decode job list", error))
    }

    pub fn get_idempotency(&self, key: &str) -> Result<Option<IdempotencyRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT key, command, request_sha256, response_json
                 FROM idempotency_records WHERE key = ?1",
                [key],
                |row| {
                    Ok(IdempotencyRecord {
                        key: row.get(0)?,
                        command: row.get(1)?,
                        request_sha256: row.get(2)?,
                        response_json: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read idempotency record", error))
    }
    pub fn put_idempotency(
        &self,
        key: &str,
        command: &str,
        request_sha256: &str,
        response_json: &str,
    ) -> Result<(), StorageError> {
        let created_at = sqlite_now(&self.conn)?;
        self.conn
            .execute(
                "INSERT INTO idempotency_records(
                    key, command, request_sha256,
                    response_json, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![key, command, request_sha256, response_json, created_at],
            )
            .map_err(|error| {
                if matches!(
                    error.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::ConstraintViolation)
                ) {
                    StorageError::new("IDEMPOTENCY_CONFLICT", "idempotency key already exists")
                } else {
                    StorageError::sqlite("store idempotency record", error)
                }
            })?;
        Ok(())
    }

    pub fn begin_transaction(
        &self,
        input: NewTransaction<'_>,
    ) -> Result<TransactionRecord, StorageError> {
        let id = opaque_id("TXN");
        let now = sqlite_now(&self.conn)?;
        self.conn
            .execute(
                "INSERT INTO transactions(
                    id, project_id, request_id, command,
                    effect_class, permission, state,
                    actor_id, client_id, delegator_id,
                    request_sha256, intended_summary,
                    verification, rollback_status,
                    created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, 'REQUESTED',
                    ?7, ?8, ?9, ?10, ?11,
                    'pending', ?12, ?13, ?13
                 )",
                params![
                    id,
                    input.project_id,
                    input.request_id,
                    input.command,
                    input.effect_class,
                    input.permission,
                    input.actor_id,
                    input.client_id,
                    input.delegator_id,
                    input.request_sha256,
                    input.intended_summary,
                    input.rollback_status,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("begin transaction record", error))?;
        self.get_transaction(&id)?.ok_or_else(|| {
            StorageError::new("STORAGE_ERROR", "transaction disappeared after write")
        })
    }

    pub fn finish_transaction(
        &self,
        id: &str,
        state: &str,
        after_ref: Option<&str>,
        verification: &str,
        result_id: Option<&str>,
        job_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<TransactionRecord, StorageError> {
        let now = sqlite_now(&self.conn)?;
        self.conn
            .execute(
                "UPDATE transactions
                 SET state=?2, after_ref=?3, verification=?4,
                     result_id=?5, job_id=?6, updated_at=?7,
                     project_id=COALESCE(?8, project_id)
                 WHERE id=?1",
                params![
                    id,
                    state,
                    after_ref,
                    verification,
                    result_id,
                    job_id,
                    now,
                    project_id
                ],
            )
            .map_err(|error| StorageError::sqlite("finish transaction record", error))?;
        self.get_transaction(id)?
            .ok_or_else(|| StorageError::new("TRANSACTION_NOT_FOUND", "transaction not found"))
    }
    pub fn get_transaction(&self, id: &str) -> Result<Option<TransactionRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, request_id, command,
                        effect_class, permission, state,
                        actor_id, client_id, delegator_id,
                        request_sha256, intended_summary,
                        before_ref, after_ref, verification,
                        result_id, job_id, rollback_status,
                        created_at, updated_at
                 FROM transactions WHERE id=?1",
                [id],
                |row| {
                    Ok(TransactionRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        request_id: row.get(2)?,
                        command: row.get(3)?,
                        effect_class: row.get(4)?,
                        permission: row.get(5)?,
                        state: row.get(6)?,
                        actor_id: row.get(7)?,
                        client_id: row.get(8)?,
                        delegator_id: row.get(9)?,
                        request_sha256: row.get(10)?,
                        intended_summary: row.get(11)?,
                        before_ref: row.get(12)?,
                        after_ref: row.get(13)?,
                        verification: row.get(14)?,
                        result_id: row.get(15)?,
                        job_id: row.get(16)?,
                        rollback_status: row.get(17)?,
                        created_at: row.get(18)?,
                        updated_at: row.get(19)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read transaction record", error))
    }

    pub fn list_transactions(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TransactionRecord>, StorageError> {
        let limit = limit.clamp(1, 200) as i64;
        let mut statement = self
            .conn
            .prepare(
                "SELECT id FROM transactions
                 WHERE (?1 IS NULL OR project_id=?1)
                 ORDER BY created_at DESC, id DESC LIMIT ?2",
            )
            .map_err(|error| StorageError::sqlite("prepare transaction list", error))?;
        let ids = statement
            .query_map(params![project_id, limit], |row| row.get::<_, String>(0))
            .map_err(|error| StorageError::sqlite("query transaction list", error))?;
        let mut output = Vec::new();
        for id in ids {
            if let Some(record) = self.get_transaction(
                &id.map_err(|error| StorageError::sqlite("decode transaction ID", error))?,
            )? {
                output.push(record);
            }
        }
        Ok(output)
    }

    pub fn record_usage(&self, input: UsageMetricInput<'_>) -> Result<(), StorageError> {
        let id = opaque_id("USG");
        let now = sqlite_now(&self.conn)?;
        self.conn
            .execute(
                "INSERT INTO usage_metrics(
                    id, request_id, command, command_version,
                    actor_id, client_id, delegator_id, project_id,
                    effect_class, permission, ok, replayed,
                    elapsed_ms, request_bytes, response_bytes,
                    remote_calls, model_tokens_in, model_tokens_out,
                    created_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                    ?16, ?17, ?18, ?19
                 )",
                params![
                    id,
                    input.request_id,
                    input.command,
                    input.command_version as i64,
                    input.actor_id,
                    input.client_id,
                    input.delegator_id,
                    input.project_id,
                    input.effect_class,
                    input.permission,
                    if input.ok { 1 } else { 0 },
                    if input.replayed { 1 } else { 0 },
                    input.elapsed_ms as i64,
                    input.request_bytes as i64,
                    input.response_bytes as i64,
                    input.remote_calls as i64,
                    input.model_tokens_in as i64,
                    input.model_tokens_out as i64,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("record usage metric", error))?;
        Ok(())
    }

    pub fn usage_summary(&self) -> Result<UsageSummary, StorageError> {
        self.conn
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(ok),0),
                    COALESCE(SUM(CASE WHEN ok=0 THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(replayed),0),
                    COALESCE(SUM(elapsed_ms),0),
                    COALESCE(SUM(request_bytes),0),
                    COALESCE(SUM(response_bytes),0),
                    COALESCE(SUM(remote_calls),0),
                    COALESCE(SUM(model_tokens_in),0),
                    COALESCE(SUM(model_tokens_out),0)
                 FROM usage_metrics",
                [],
                |row| {
                    Ok(UsageSummary {
                        command_count: row.get::<_, i64>(0)? as u64,
                        success_count: row.get::<_, i64>(1)? as u64,
                        failure_count: row.get::<_, i64>(2)? as u64,
                        replay_count: row.get::<_, i64>(3)? as u64,
                        total_elapsed_ms: row.get::<_, i64>(4)? as u64,
                        total_request_bytes: row.get::<_, i64>(5)? as u64,
                        total_response_bytes: row.get::<_, i64>(6)? as u64,
                        remote_calls: row.get::<_, i64>(7)? as u64,
                        model_tokens_in: row.get::<_, i64>(8)? as u64,
                        model_tokens_out: row.get::<_, i64>(9)? as u64,
                    })
                },
            )
            .map_err(|error| StorageError::sqlite("read usage summary", error))
    }

    pub fn upsert_credential_handle(
        &self,
        id: &str,
        integration: &str,
        scopes: &[String],
        status: &str,
    ) -> Result<CredentialHandleRecord, StorageError> {
        let now = sqlite_now(&self.conn)?;
        let scopes_json = json_text(&serde_json::to_value(scopes).map_err(|error| {
            StorageError::new(
                "STORAGE_ERROR",
                format!("serialize credential scopes: {error}"),
            )
        })?)?;
        self.conn
            .execute(
                "INSERT INTO credential_handles(
                    id, integration, scopes_json, status,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                   integration=excluded.integration,
                   scopes_json=excluded.scopes_json,
                   status=excluded.status,
                   updated_at=excluded.updated_at",
                params![id, integration, scopes_json, status, now],
            )
            .map_err(|error| StorageError::sqlite("store credential handle metadata", error))?;
        self.get_credential_handle(id)?.ok_or_else(|| {
            StorageError::new("STORAGE_ERROR", "credential handle disappeared after write")
        })
    }

    pub fn get_credential_handle(
        &self,
        id: &str,
    ) -> Result<Option<CredentialHandleRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, integration, scopes_json, status,
                        created_at, updated_at
                 FROM credential_handles WHERE id=?1",
                [id],
                |row| {
                    let scopes_json: String = row.get(2)?;
                    let scopes: Vec<String> =
                        serde_json::from_str(&scopes_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(CredentialHandleRecord {
                        id: row.get(0)?,
                        integration: row.get(1)?,
                        scopes,
                        status: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read credential handle metadata", error))
    }

    pub fn revoke_credential_handle(&self, id: &str) -> Result<(), StorageError> {
        let now = sqlite_now(&self.conn)?;
        let changed = self
            .conn
            .execute(
                "UPDATE credential_handles
                 SET status='revoked', updated_at=?2
                 WHERE id=?1",
                params![id, now],
            )
            .map_err(|error| StorageError::sqlite("revoke credential handle metadata", error))?;
        if changed == 0 {
            return Err(StorageError::new(
                "CREDENTIAL_HANDLE_NOT_FOUND",
                "credential handle not found",
            ));
        }
        Ok(())
    }

    pub fn record_egress(
        &self,
        input: EgressLedgerInput<'_>,
    ) -> Result<EgressLedgerRecord, StorageError> {
        let id = opaque_id("EGR");
        let now = sqlite_now(&self.conn)?;
        let data_classes_json = serde_json::to_string(input.data_classes).map_err(|error| {
            StorageError::new(
                "STORAGE_ERROR",
                format!("serialize egress data classes: {error}"),
            )
        })?;
        let modalities_json = serde_json::to_string(input.modalities).map_err(|error| {
            StorageError::new(
                "STORAGE_ERROR",
                format!("serialize egress modalities: {error}"),
            )
        })?;
        let source_refs_json = serde_json::to_string(input.source_refs).map_err(|error| {
            StorageError::new(
                "STORAGE_ERROR",
                format!("serialize egress source refs: {error}"),
            )
        })?;

        self.conn
            .execute(
                "INSERT INTO egress_ledger(
                    id, project_id, request_id,
                    actor_id, client_id, delegator_id,
                    destination, data_classes_json, modalities_json,
                    source_refs_json, purpose,
                    approx_bytes, approx_tokens, credential_handle,
                    decision, reason, created_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
                 )",
                params![
                    id,
                    input.project_id,
                    input.request_id,
                    input.actor_id,
                    input.client_id,
                    input.delegator_id,
                    input.destination,
                    data_classes_json,
                    modalities_json,
                    source_refs_json,
                    input.purpose,
                    input.approx_bytes as i64,
                    input.approx_tokens as i64,
                    input.credential_handle,
                    input.decision,
                    input.reason,
                    now,
                ],
            )
            .map_err(|error| StorageError::sqlite("record egress ledger entry", error))?;
        self.get_egress(&id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "egress ledger entry disappeared after write",
            )
        })
    }

    pub fn get_egress(&self, id: &str) -> Result<Option<EgressLedgerRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, request_id,
                        actor_id, client_id, delegator_id,
                        destination, data_classes_json, modalities_json,
                        source_refs_json, purpose,
                        approx_bytes, approx_tokens, credential_handle,
                        decision, reason, created_at
                 FROM egress_ledger WHERE id=?1",
                [id],
                |row| {
                    let data_classes_json: String = row.get(7)?;
                    let modalities_json: String = row.get(8)?;
                    let source_refs_json: String = row.get(9)?;
                    Ok(EgressLedgerRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        request_id: row.get(2)?,
                        actor_id: row.get(3)?,
                        client_id: row.get(4)?,
                        delegator_id: row.get(5)?,
                        destination: row.get(6)?,
                        data_classes: serde_json::from_str(&data_classes_json).map_err(
                            |error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            },
                        )?,
                        modalities: serde_json::from_str(&modalities_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                8,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        source_refs: serde_json::from_str(&source_refs_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                9,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        purpose: row.get(10)?,
                        approx_bytes: row.get::<_, i64>(11)? as u64,
                        approx_tokens: row.get::<_, i64>(12)? as u64,
                        credential_handle: row.get(13)?,
                        decision: row.get(14)?,
                        reason: row.get(15)?,
                        created_at: row.get(16)?,
                    })
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read egress ledger entry", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "relay-core-storage-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn create_schema_one_fixture(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE schema_migrations (
               version INTEGER PRIMARY KEY,
               applied_at TEXT NOT NULL
             );
             INSERT INTO schema_migrations VALUES (1,'fixture');
             CREATE TABLE projects (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               root_uri TEXT NOT NULL,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );",
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TABLE results (
               id TEXT PRIMARY KEY,
               project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
               kind TEXT NOT NULL,
               schema_version INTEGER NOT NULL,
               producer_version TEXT NOT NULL,
               payload_json TEXT NOT NULL,
               payload_sha256 TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE TABLE jobs (
               id TEXT PRIMARY KEY,
               project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
               command TEXT NOT NULL,
               state TEXT NOT NULL,
               checkpoint_json TEXT NOT NULL,
               result_id TEXT REFERENCES results(id) ON DELETE SET NULL,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );
             CREATE INDEX results_project_created
               ON results(project_id, created_at);
             CREATE INDEX jobs_project_updated
               ON jobs(project_id, updated_at);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects VALUES (?1,?2,?3,?4,?5)",
            params![
                "PRJ-phase1",
                "Phase 1 Fixture",
                "file:///fixture",
                "2026-09-25T00:00:00Z",
                "2026-09-25T00:00:00Z"
            ],
        )
        .unwrap();
        let payload = r#"{"value":42}"#;
        conn.execute(
            "INSERT INTO results VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                "RES-phase1",
                "PRJ-phase1",
                "TEST",
                1,
                "0.1.0-phase1-rust",
                payload,
                sha256_hex(payload.as_bytes()),
                "2026-09-25T00:00:00Z"
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO jobs VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                "JOB-phase1",
                "PRJ-phase1",
                "fixture.work",
                "CHECKPOINTED",
                r#"{"stage":2}"#,
                "RES-phase1",
                "2026-09-25T00:00:00Z",
                "2026-09-25T00:00:00Z"
            ],
        )
        .unwrap();
    }

    fn create_schema_two_fixture(path: &Path) {
        create_schema_one_fixture(path);
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"ALTER TABLE results
               ADD COLUMN provenance_json TEXT NOT NULL DEFAULT '{}';
             ALTER TABLE results
               ADD COLUMN trust TEXT NOT NULL DEFAULT 'local';
             ALTER TABLE jobs
               ADD COLUMN provenance_json TEXT NOT NULL DEFAULT '{}';
             ALTER TABLE jobs
               ADD COLUMN trust TEXT NOT NULL DEFAULT 'local';
             CREATE TABLE idempotency_records (
               key TEXT PRIMARY KEY,
               command TEXT NOT NULL,
               request_sha256 TEXT NOT NULL,
               response_json TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             INSERT INTO schema_migrations(version, applied_at)
               VALUES (2,'fixture-schema2');
             INSERT INTO idempotency_records(
               key, command, request_sha256, response_json, created_at
             ) VALUES (
               'IDEMP-schema2','result.put','fixture-sha',
               '{"ok":true}','2026-09-25T00:00:00Z'
             );"#,
        )
        .unwrap();
    }

    #[test]
    fn restoring_archived_project_requires_full_content_verification() {
        let dir = temp_dir("lifecycle-restore-index");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-life"), "Fixture", "file:///fixture")
            .unwrap();
        assert_eq!(
            storage
                .replace_project_baseline("PRJ-life", &[], 0)
                .unwrap()
                .status,
            "ready"
        );
        storage
            .set_project_lifecycle("PRJ-life", "archived")
            .unwrap();
        storage.set_project_lifecycle("PRJ-life", "active").unwrap();
        let state = storage
            .get_project_index_state("PRJ-life")
            .unwrap()
            .unwrap();
        assert_eq!(state.status, "stale");
        assert!(state.content_verification_required);
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn removal_approval_is_single_use_durable_and_revision_guarded() {
        let dir = temp_dir("removal-approval");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-a"), "A", "file:///a")
            .unwrap();
        let stale = storage
            .plan_project_removal("PRJ-a", "requester", "client", None)
            .unwrap();
        storage
            .register_project(Some("PRJ-a"), "A2", "file:///a2")
            .unwrap();
        assert_eq!(
            storage
                .get_project_removal_approval("PRJ-a", &stale.id)
                .unwrap()
                .state,
            "stale"
        );
        let rejected = storage
            .plan_project_removal("PRJ-a", "requester", "client", None)
            .unwrap();
        assert_eq!(
            storage
                .decide_project_removal("PRJ-a", &rejected.id, false, "approver", "client")
                .unwrap()
                .state,
            "rejected"
        );
        assert_eq!(
            storage
                .execute_project_removal("PRJ-a", &rejected.id, "executor", "client")
                .unwrap_err()
                .code,
            "APPROVAL_STATE_CONFLICT"
        );
        let expired = storage
            .plan_project_removal("PRJ-a", "requester", "client", None)
            .unwrap();
        storage
            .conn
            .execute(
                "UPDATE project_removal_approvals SET expires_at_ms = 1 WHERE id = ?1",
                [&expired.id],
            )
            .unwrap();
        assert_eq!(
            storage
                .get_project_removal_approval("PRJ-a", &expired.id)
                .unwrap()
                .state,
            "expired"
        );
        assert_eq!(
            storage
                .decide_project_removal("PRJ-a", &expired.id, true, "approver", "client")
                .unwrap()
                .state,
            "expired"
        );
        let approved = storage
            .plan_project_removal("PRJ-a", "requester", "client", Some("owner"))
            .unwrap();
        assert_eq!(
            storage
                .decide_project_removal("PRJ-a", &approved.id, true, "approver", "client")
                .unwrap()
                .state,
            "approved_pending_execution"
        );
        drop(storage);
        let reopened = RelayStorage::open(&path).unwrap();
        assert_eq!(
            reopened
                .get_project_removal_approval("PRJ-a", &approved.id)
                .unwrap()
                .state,
            "approved_pending_execution"
        );
        assert!(
            reopened
                .execute_project_removal("PRJ-a", &approved.id, "executor", "client")
                .unwrap()
                .1
        );
        assert!(
            !reopened
                .execute_project_removal("PRJ-a", &approved.id, "executor", "client")
                .unwrap()
                .1
        );
        assert_eq!(
            reopened
                .get_project("PRJ-a")
                .unwrap()
                .unwrap()
                .lifecycle_state,
            "removed"
        );
        assert_eq!(
            reopened
                .get_project_removal_approval("PRJ-a", &approved.id)
                .unwrap()
                .state,
            "executed"
        );
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lifecycle_tombstone_preserves_result_and_job_links_across_restart() {
        let dir = temp_dir("lifecycle-history");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-life"), "Fixture", "file:///fixture")
            .unwrap();
        let result = storage
            .put_result(
                Some("PRJ-life"),
                "TEST",
                &json!({ "value": 7 }),
                "0.1.0",
                &json!({}),
                "local",
            )
            .unwrap();
        let job = storage
            .checkpoint_job(
                Some("JOB-life"),
                Some("PRJ-life"),
                "fixture.work",
                "DONE",
                &json!({ "step": 1 }),
                Some(&result.id),
                &json!({}),
                "local",
            )
            .unwrap();
        assert_eq!(
            storage
                .set_project_lifecycle("PRJ-life", "removed")
                .unwrap_err()
                .code,
            "VALIDATION_FAILED"
        );
        let plan = storage
            .plan_project_removal("PRJ-life", "requester", "client", None)
            .unwrap();
        storage
            .decide_project_removal("PRJ-life", &plan.id, true, "approver", "client")
            .unwrap();
        assert!(
            storage
                .execute_project_removal("PRJ-life", &plan.id, "executor", "client")
                .unwrap()
                .1
        );
        assert!(
            !storage
                .execute_project_removal("PRJ-life", &plan.id, "executor", "client")
                .unwrap()
                .1
        );
        assert!(storage.list_projects().unwrap().is_empty());
        assert_eq!(
            storage
                .get_result(&result.id)
                .unwrap()
                .unwrap()
                .project_id
                .as_deref(),
            Some("PRJ-life")
        );
        assert_eq!(
            storage
                .get_job(&job.id)
                .unwrap()
                .unwrap()
                .result_id
                .as_deref(),
            Some(result.id.as_str())
        );
        assert_eq!(
            storage
                .put_result(
                    Some("PRJ-life"),
                    "TEST",
                    &json!({}),
                    "0.1.0",
                    &json!({}),
                    "local"
                )
                .unwrap_err()
                .code,
            "PROJECT_NOT_FOUND"
        );
        drop(storage);

        let reopened = RelayStorage::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert_eq!(
            reopened
                .get_project("PRJ-life")
                .unwrap()
                .unwrap()
                .lifecycle_state,
            "removed"
        );
        assert_eq!(
            reopened
                .get_job(&job.id)
                .unwrap()
                .unwrap()
                .project_id
                .as_deref(),
            Some("PRJ-life")
        );
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fresh_store_uses_current_schema_and_typed_records() {
        let dir = temp_dir("fresh");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        let project = storage
            .register_project(
                Some("PRJ-production"),
                "Production Fixture",
                "file:///fixture",
            )
            .unwrap();
        let provenance = json!({
            "source": "relay-core",
            "actor_id": "fixture-actor"
        });
        let result = storage
            .put_result(
                Some(&project.id),
                "TEST",
                &json!({ "value": 7 }),
                "0.1.0",
                &provenance,
                "trusted-local",
            )
            .unwrap();
        assert_eq!(result.payload["value"], 7);
        assert_eq!(result.provenance["actor_id"], "fixture-actor");
        assert_eq!(result.trust, "trusted-local");

        let job = storage
            .checkpoint_job(
                None,
                Some(&project.id),
                "fixture.work",
                "CHECKPOINTED",
                &json!({ "stage": 1 }),
                Some(&result.id),
                &provenance,
                "trusted-local",
            )
            .unwrap();
        assert_eq!(job.checkpoint["stage"], 1);
        assert_eq!(job.result_id.as_deref(), Some(result.id.as_str()));
        assert!(storage.integrity().unwrap().ok);
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_one_fixture_migrates_without_data_loss() {
        let dir = temp_dir("phase1");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        create_schema_one_fixture(&path);

        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        let project = storage.get_project("PRJ-phase1").unwrap().unwrap();
        assert_eq!(project.name, "Phase 1 Fixture");

        let result = storage.get_result("RES-phase1").unwrap().unwrap();
        assert_eq!(result.payload["value"], 42);
        assert_eq!(result.provenance, json!({}));
        assert_eq!(result.trust, "local");

        let job = storage.get_job("JOB-phase1").unwrap().unwrap();
        assert_eq!(job.checkpoint["stage"], 2);
        assert_eq!(job.provenance, json!({}));
        assert_eq!(job.trust, "local");
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn result_read_rejects_payload_changed_without_matching_digest() {
        let dir = temp_dir("result-digest");
        fs::create_dir_all(&dir).unwrap();
        let storage = RelayStorage::open(dir.join("relay.sqlite3")).unwrap();
        let result = storage
            .put_result(
                None,
                "TEST",
                &json!({ "status": "original" }),
                "0.1.0",
                &json!({}),
                "local",
            )
            .unwrap();
        storage
            .conn
            .execute(
                "UPDATE results SET payload_json = ?1 WHERE id = ?2",
                rusqlite::params![r#"{"status":"changed"}"#, &result.id],
            )
            .unwrap();
        assert!(storage.get_result(&result.id).is_err());
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn schema_two_fixture_migrates_to_five_without_replay_loss() {
        let dir = temp_dir("schema2");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        create_schema_two_fixture(&path);

        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        let result = storage.get_result("RES-phase1").unwrap().unwrap();
        assert_eq!(result.payload["value"], 42);
        assert_eq!(result.trust, "local");
        let replay = storage.get_idempotency("IDEMP-schema2").unwrap().unwrap();
        assert_eq!(replay.command, "result.put");
        assert_eq!(replay.request_sha256, "fixture-sha");
        assert_eq!(storage.usage_summary().unwrap().command_count, 0);
        assert!(storage.list_transactions(None, 10).unwrap().is_empty());

        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_three_fixture_migrates_to_five_without_authority_loss() {
        let dir = temp_dir("schema3-to-schema4");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        create_schema_two_fixture(&path);

        let conn = Connection::open(&path).unwrap();
        let mut schema_three = RelayStorage {
            db_path: path.clone(),
            conn,
        };
        schema_three.configure().unwrap();
        schema_three.apply_schema_three().unwrap();
        assert_eq!(schema_three.schema_version().unwrap(), 3);

        let transaction = schema_three
            .begin_transaction(NewTransaction {
                project_id: Some("PRJ-phase1"),
                request_id: "REQ-schema3",
                command: "result.put",
                effect_class: "relay_state_write",
                permission: "state_write",
                actor_id: "ACTOR-schema3",
                client_id: "CLIENT-schema3",
                delegator_id: Some("USER-schema3"),
                request_sha256: "schema3-sha",
                intended_summary: "preserve authority state",
                rollback_status: "not_available",
            })
            .unwrap();
        schema_three
            .finish_transaction(
                &transaction.id,
                "COMPLETED",
                Some("RES-phase1"),
                "verified",
                Some("RES-phase1"),
                Some("JOB-phase1"),
                None,
            )
            .unwrap();
        schema_three
            .record_usage(UsageMetricInput {
                request_id: "REQ-schema3",
                command: "result.put",
                command_version: 1,
                actor_id: "ACTOR-schema3",
                client_id: "CLIENT-schema3",
                delegator_id: Some("USER-schema3"),
                project_id: Some("PRJ-phase1"),
                effect_class: "relay_state_write",
                permission: "state_write",
                ok: true,
                replayed: false,
                elapsed_ms: 7,
                request_bytes: 100,
                response_bytes: 80,
                remote_calls: 0,
                model_tokens_in: 0,
                model_tokens_out: 0,
            })
            .unwrap();
        schema_three
            .upsert_credential_handle(
                "credential.schema3",
                "fixture",
                &["read".to_string()],
                "active",
            )
            .unwrap();
        let egress = schema_three
            .record_egress(EgressLedgerInput {
                project_id: Some("PRJ-phase1"),
                request_id: "REQ-schema3-egress",
                actor_id: "ACTOR-schema3",
                client_id: "CLIENT-schema3",
                delegator_id: Some("USER-schema3"),
                destination: "fixture-destination",
                data_classes: &["project".to_string()],
                modalities: &["text".to_string()],
                source_refs: &["RES-phase1".to_string()],
                purpose: "fixture",
                approx_bytes: 42,
                approx_tokens: 10,
                credential_handle: Some("credential.schema3"),
                decision: "blocked",
                reason: "fixture policy",
            })
            .unwrap();
        drop(schema_three);

        let migrated = RelayStorage::open(&path).unwrap();
        assert_eq!(migrated.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert_eq!(
            migrated.get_project("PRJ-phase1").unwrap().unwrap().name,
            "Phase 1 Fixture"
        );
        assert_eq!(
            migrated.get_result("RES-phase1").unwrap().unwrap().payload["value"],
            42
        );
        assert!(migrated.get_job("JOB-phase1").unwrap().is_some());
        assert!(migrated.get_idempotency("IDEMP-schema2").unwrap().is_some());
        let preserved = migrated.get_transaction(&transaction.id).unwrap().unwrap();
        assert_eq!(preserved.actor_id, "ACTOR-schema3");
        assert_eq!(preserved.client_id, "CLIENT-schema3");
        assert_eq!(preserved.delegator_id.as_deref(), Some("USER-schema3"));
        assert_eq!(preserved.state, "COMPLETED");
        assert_eq!(migrated.usage_summary().unwrap().command_count, 1);
        assert_eq!(
            migrated
                .get_credential_handle("credential.schema3")
                .unwrap()
                .unwrap()
                .status,
            "active"
        );
        assert_eq!(
            migrated.get_egress(&egress.id).unwrap().unwrap().decision,
            "blocked"
        );
        assert!(
            migrated
                .get_project_index_state("PRJ-phase1")
                .unwrap()
                .is_none()
        );
        assert!(
            migrated
                .list_project_files("PRJ-phase1")
                .unwrap()
                .is_empty()
        );
        drop(migrated);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_four_index_migrates_to_six_with_conservative_delta_boundary() {
        let dir = temp_dir("schema4-to-schema5");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let conn = Connection::open(&path).unwrap();
        let mut schema_four = RelayStorage {
            db_path: path.clone(),
            conn,
        };
        schema_four.configure().unwrap();
        schema_four
            .conn
            .execute_batch(
                "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
            )
            .unwrap();
        schema_four.apply_schema_one().unwrap();
        schema_four.apply_schema_two().unwrap();
        schema_four.apply_schema_three().unwrap();
        schema_four.apply_schema_four().unwrap();
        assert_eq!(schema_four.schema_version().unwrap(), 4);
        schema_four
            .conn
            .execute(
                "INSERT INTO projects(id, name, root_uri, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
                params!["PRJ-schema4", "Schema 4", "file:///fixture", "fixture"],
            )
            .unwrap();
        schema_four
            .conn
            .execute(
                "INSERT INTO project_index_state(
                project_id, generation, status, file_count,
                total_bytes, last_reconciled_at, updated_at
             ) VALUES (?1, 3, 'ready', 1, 4, 'fixture', 'fixture')",
                ["PRJ-schema4"],
            )
            .unwrap();
        schema_four
            .conn
            .execute(
                "INSERT INTO project_files(
                project_id, relative_path, size_bytes,
                modified_unix_ns, content_sha256, indexed_at
             ) VALUES (?1, ?2, 4, 42, ?3, 'fixture')",
                params!["PRJ-schema4", "src/a.txt", "a".repeat(64)],
            )
            .unwrap();
        schema_four
            .conn
            .execute(
                "INSERT INTO project_changes(
                id, project_id, generation, change_kind,
                relative_path, previous_path, before_sha256,
                after_sha256, detected_at
             ) VALUES (?1, ?2, 3, 'added', ?3, NULL, NULL, ?4, 'fixture')",
                params!["CHG-schema4", "PRJ-schema4", "src/a.txt", "a".repeat(64)],
            )
            .unwrap();
        drop(schema_four);

        let migrated = RelayStorage::open(&path).unwrap();
        assert_eq!(migrated.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        let state = migrated
            .get_project_index_state("PRJ-schema4")
            .unwrap()
            .unwrap();
        assert_eq!(state.generation, 3);
        assert_eq!(state.baseline_generation, 3);
        assert_eq!(state.status, "stale");
        assert!(state.content_verification_required);
        assert_eq!(migrated.list_project_files("PRJ-schema4").unwrap().len(), 1);
        assert_eq!(
            migrated
                .list_project_changes("PRJ-schema4", 10)
                .unwrap()
                .len(),
            1
        );
        assert!(
            migrated
                .list_project_dependency_edges("PRJ-schema4", None, 10)
                .unwrap()
                .is_empty()
        );
        drop(migrated);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_five_ready_index_migrates_to_required_verification() {
        let dir = temp_dir("schema5-to-schema6");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let conn = Connection::open(&path).unwrap();
        let mut schema_five = RelayStorage {
            db_path: path.clone(),
            conn,
        };
        schema_five.configure().unwrap();
        schema_five.conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);"
        ).unwrap();
        schema_five.apply_schema_one().unwrap();
        schema_five.apply_schema_two().unwrap();
        schema_five.apply_schema_three().unwrap();
        schema_five.apply_schema_four().unwrap();
        schema_five.apply_schema_five().unwrap();
        assert_eq!(schema_five.schema_version().unwrap(), 5);
        schema_five
            .conn
            .execute(
                "INSERT INTO projects(id, name, root_uri, created_at, updated_at)
             VALUES ('PRJ-schema5', 'Schema 5', 'file:///fixture', 'fixture', 'fixture')",
                [],
            )
            .unwrap();
        schema_five
            .conn
            .execute(
                "INSERT INTO project_index_state(project_id, generation, baseline_generation,
             status, file_count, total_bytes, last_reconciled_at, updated_at)
             VALUES ('PRJ-schema5', 4, 1, 'ready', 0, 0, 'fixture', 'fixture')",
                [],
            )
            .unwrap();
        drop(schema_five);

        let migrated = RelayStorage::open(&path).unwrap();
        assert_eq!(migrated.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        let state = migrated
            .get_project_index_state("PRJ-schema5")
            .unwrap()
            .unwrap();
        assert_eq!(state.generation, 4);
        assert_eq!(state.baseline_generation, 1);
        assert_eq!(state.status, "stale");
        assert!(state.content_verification_required);
        drop(migrated);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_six_project_configuration_migrates_to_seven_without_touching_index() {
        let dir = temp_dir("schema6-to-schema7");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let conn = Connection::open(&path).unwrap();
        let mut schema_six = RelayStorage {
            db_path: path.clone(),
            conn,
        };
        schema_six.configure().unwrap();
        schema_six.conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);"
        ).unwrap();
        schema_six.apply_schema_one().unwrap();
        schema_six.apply_schema_two().unwrap();
        schema_six.apply_schema_three().unwrap();
        schema_six.apply_schema_four().unwrap();
        schema_six.apply_schema_five().unwrap();
        schema_six.apply_schema_six().unwrap();
        assert_eq!(schema_six.schema_version().unwrap(), 6);
        schema_six
            .conn
            .execute(
                "INSERT INTO projects(id, name, root_uri, created_at, updated_at)
             VALUES ('PRJ-schema6', 'Schema 6', 'file:///fixture', 'fixture', 'fixture')",
                [],
            )
            .unwrap();
        schema_six
            .conn
            .execute(
                "INSERT INTO project_index_state(project_id, generation, baseline_generation,
             status, file_count, total_bytes, last_reconciled_at, updated_at,
             content_verification_required)
             VALUES ('PRJ-schema6', 4, 1, 'stale', 0, 0, 'fixture', 'fixture', 1)",
                [],
            )
            .unwrap();
        drop(schema_six);

        let migrated = RelayStorage::open(&path).unwrap();
        assert_eq!(migrated.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert!(
            migrated
                .get_project_configuration("PRJ-schema6")
                .unwrap()
                .is_none()
        );
        let index = migrated
            .get_project_index_state("PRJ-schema6")
            .unwrap()
            .unwrap();
        assert_eq!(index.generation, 4);
        assert!(index.content_verification_required);
        drop(migrated);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_five_project_index_round_trip_and_generation_guard() {
        let dir = temp_dir("schema4-index");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-index-a"), "Index A", "file:///fixture-a")
            .unwrap();
        storage
            .register_project(Some("PRJ-index-b"), "Index B", "file:///fixture-b")
            .unwrap();

        let a_files = vec![
            IndexedFileSnapshot {
                relative_path: "src/a.txt".to_string(),
                size_bytes: 10,
                modified_unix_ns: 100,
                content_sha256: "a".repeat(64),
            },
            IndexedFileSnapshot {
                relative_path: "src/shared.txt".to_string(),
                size_bytes: 20,
                modified_unix_ns: 101,
                content_sha256: "b".repeat(64),
            },
        ];
        let b_files = vec![IndexedFileSnapshot {
            relative_path: "src/shared.txt".to_string(),
            size_bytes: 30,
            modified_unix_ns: 200,
            content_sha256: "c".repeat(64),
        }];

        let a_state = storage
            .replace_project_baseline("PRJ-index-a", &a_files, 30)
            .unwrap();
        let b_state = storage
            .replace_project_baseline("PRJ-index-b", &b_files, 30)
            .unwrap();
        assert_eq!(a_state.generation, 1);
        assert!(!a_state.content_verification_required);
        assert_eq!(b_state.generation, 1);
        assert_eq!(storage.list_project_files("PRJ-index-a").unwrap(), a_files);
        assert_eq!(storage.list_project_files("PRJ-index-b").unwrap(), b_files);

        let touched = vec![IndexedFileSnapshot {
            relative_path: "src/a.txt".to_string(),
            size_bytes: 11,
            modified_unix_ns: 300,
            content_sha256: "d".repeat(64),
        }];
        let changes = vec![IndexChange {
            change_kind: "modified".to_string(),
            relative_path: "src/a.txt".to_string(),
            previous_path: None,
            before_sha256: Some("a".repeat(64)),
            after_sha256: Some("d".repeat(64)),
        }];
        let next = storage
            .apply_project_reconciliation(ProjectIndexCommit {
                project_id: "PRJ-index-a",
                expected_generation: 1,
                touched_files: &touched,
                changes: &changes,
                file_count: 2,
                total_bytes: 31,
                mode: IndexCommitMode::Authoritative,
                content_verified: false,
            })
            .unwrap();
        assert_eq!(next.generation, 2);
        assert_eq!(
            storage.list_project_files("PRJ-index-a").unwrap()[0].content_sha256,
            "d".repeat(64)
        );
        assert_eq!(storage.list_project_files("PRJ-index-b").unwrap(), b_files);
        let history = storage.list_project_changes("PRJ-index-a", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].generation, 2);

        let error = storage
            .apply_project_reconciliation(ProjectIndexCommit {
                project_id: "PRJ-index-a",
                expected_generation: 1,
                touched_files: &[],
                changes: &[],
                file_count: 2,
                total_bytes: 31,
                mode: IndexCommitMode::Authoritative,
                content_verified: false,
            })
            .expect_err("stale generation must fail");
        assert_eq!(error.code, "INDEX_GENERATION_CONFLICT");

        drop(storage);
        let reopened = RelayStorage::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert_eq!(
            reopened
                .get_project_index_state("PRJ-index-a")
                .unwrap()
                .unwrap()
                .generation,
            2
        );
        assert_eq!(reopened.list_project_files("PRJ-index-b").unwrap(), b_files);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parser_coverage_records_zero_edges_and_invalidation_history() {
        let dir = temp_dir("parser-coverage");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-coverage"), "Coverage", "file:///fixture")
            .unwrap();
        let files = [
            IndexedFileSnapshot {
                relative_path: "source.txt".into(),
                size_bytes: 1,
                modified_unix_ns: 1,
                content_sha256: "a".repeat(64),
            },
            IndexedFileSnapshot {
                relative_path: "empty.txt".into(),
                size_bytes: 1,
                modified_unix_ns: 1,
                content_sha256: "b".repeat(64),
            },
            IndexedFileSnapshot {
                relative_path: "target.txt".into(),
                size_bytes: 1,
                modified_unix_ns: 1,
                content_sha256: "c".repeat(64),
            },
        ];
        storage
            .replace_project_baseline("PRJ-coverage", &files, 3)
            .unwrap();
        storage
            .replace_project_dependencies(DependencyReplacement {
                project_id: "PRJ-coverage",
                expected_generation: 1,
                source_path: "source.txt",
                expected_source_sha256: &files[0].content_sha256,
                producer_id: "fixture",
                producer_version: "1",
                configuration_guard: None,
                targets: &["target.txt".to_string()],
            })
            .unwrap();
        storage
            .replace_project_dependencies(DependencyReplacement {
                project_id: "PRJ-coverage",
                expected_generation: 1,
                source_path: "empty.txt",
                expected_source_sha256: &files[1].content_sha256,
                producer_id: "fixture",
                producer_version: "1",
                configuration_guard: None,
                targets: &[],
            })
            .unwrap();
        let coverage = storage
            .list_project_dependency_coverage("PRJ-coverage", 10)
            .unwrap();
        assert_eq!(coverage.len(), 2);
        assert_eq!(coverage[0].source_path, "empty.txt");
        assert_eq!(coverage[0].target_count, 0);
        assert_eq!(coverage[1].target_count, 1);
        drop(storage);

        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(
            storage
                .list_project_dependency_coverage("PRJ-coverage", 10)
                .unwrap()
                .len(),
            2
        );
        let changed_target = IndexedFileSnapshot {
            relative_path: "target.txt".into(),
            size_bytes: 2,
            modified_unix_ns: 2,
            content_sha256: "d".repeat(64),
        };
        storage
            .apply_project_reconciliation(ProjectIndexCommit {
                project_id: "PRJ-coverage",
                expected_generation: 1,
                touched_files: &[changed_target.clone()],
                changes: &[IndexChange {
                    change_kind: "modified".into(),
                    relative_path: "target.txt".into(),
                    previous_path: None,
                    before_sha256: Some(files[2].content_sha256.clone()),
                    after_sha256: Some(changed_target.content_sha256.clone()),
                }],
                file_count: 3,
                total_bytes: 4,
                mode: IndexCommitMode::Authoritative,
                content_verified: false,
            })
            .unwrap();
        let coverage = storage
            .list_project_dependency_coverage("PRJ-coverage", 10)
            .unwrap();
        assert_eq!(coverage.len(), 1);
        assert_eq!(coverage[0].source_path, "empty.txt");
        let history = storage
            .list_project_dependency_invalidations_since("PRJ-coverage", 1, 10)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].generation, 2);
        assert_eq!(history[0].source_path, "source.txt");
        assert_eq!(history[0].target_path, "target.txt");
        storage.mark_project_index_stale("PRJ-coverage").unwrap();
        assert!(
            storage
                .list_project_dependency_coverage("PRJ-coverage", 10)
                .unwrap()
                .is_empty()
        );
        storage
            .replace_project_baseline(
                "PRJ-coverage",
                &[files[0].clone(), files[1].clone(), changed_target],
                4,
            )
            .unwrap();
        assert!(
            storage
                .list_project_dependency_coverage("PRJ-coverage", 10)
                .unwrap()
                .is_empty()
        );
        assert!(
            storage
                .list_project_dependency_invalidations_since("PRJ-coverage", 0, 10)
                .unwrap()
                .is_empty()
        );
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn project_check_catalog_is_revisioned_and_digest_verified() {
        let dir = temp_dir("check-catalog");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-checks"), "Checks", "file:///fixture")
            .unwrap();
        storage
            .replace_project_baseline("PRJ-checks", &[], 0)
            .unwrap();
        let catalog = serde_json::json!({"format_version": 1, "checks": []});
        let first = storage
            .put_project_check_catalog("PRJ-checks", 0, &catalog)
            .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.index_generation, 1);
        assert_eq!(first.catalog, catalog);
        assert_eq!(
            storage
                .put_project_check_catalog("PRJ-checks", 0, &catalog)
                .unwrap_err()
                .code,
            "CHECK_CATALOG_CONFLICT"
        );
        drop(storage);

        let reopened = RelayStorage::open(&path).unwrap();
        assert_eq!(
            reopened
                .get_project_check_catalog("PRJ-checks")
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        reopened
            .conn
            .execute(
                "UPDATE project_check_catalog SET catalog_json = ?1 WHERE project_id = ?2",
                params![
                    r#"{"format_version":1,"checks":[{"id":"changed"}]}"#,
                    "PRJ-checks"
                ],
            )
            .unwrap();
        assert!(reopened.get_project_check_catalog("PRJ-checks").is_err());
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lost_continuity_requires_verified_commit_across_restart() {
        let dir = temp_dir("verified-recovery");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        storage
            .register_project(Some("PRJ-recovery"), "Recovery", "file:///fixture")
            .unwrap();
        let file = IndexedFileSnapshot {
            relative_path: "one.txt".to_string(),
            size_bytes: 1,
            modified_unix_ns: 1,
            content_sha256: "a".repeat(64),
        };
        storage
            .replace_project_baseline("PRJ-recovery", &[file], 1)
            .unwrap();
        storage.mark_project_index_stale("PRJ-recovery").unwrap();
        drop(storage);

        let reopened = RelayStorage::open(&path).unwrap();
        let state = reopened
            .get_project_index_state("PRJ-recovery")
            .unwrap()
            .unwrap();
        assert_eq!(state.status, "stale");
        assert!(state.content_verification_required);
        let commit = |content_verified| ProjectIndexCommit {
            project_id: "PRJ-recovery",
            expected_generation: 1,
            touched_files: &[],
            changes: &[],
            file_count: 1,
            total_bytes: 1,
            mode: IndexCommitMode::Authoritative,
            content_verified,
        };
        let error = reopened
            .apply_project_reconciliation(commit(false))
            .unwrap_err();
        assert_eq!(error.code, "INDEX_CONTENT_VERIFICATION_REQUIRED");
        assert_eq!(
            reopened
                .get_project_index_state("PRJ-recovery")
                .unwrap()
                .unwrap()
                .generation,
            1
        );
        let ready = reopened.apply_project_reconciliation(commit(true)).unwrap();
        assert_eq!(ready.status, "ready");
        assert!(!ready.content_verification_required);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn idempotency_record_round_trip_and_conflict() {
        let dir = temp_dir("idempotency");
        fs::create_dir_all(&dir).unwrap();
        let storage = RelayStorage::open(dir.join("relay.sqlite3")).unwrap();

        storage
            .put_idempotency("IDEMP-fixture", "result.put", "abc123", r#"{"ok":true}"#)
            .unwrap();
        let record = storage.get_idempotency("IDEMP-fixture").unwrap().unwrap();
        assert_eq!(record.command, "result.put");
        assert_eq!(record.request_sha256, "abc123");

        let error = storage
            .put_idempotency("IDEMP-fixture", "result.put", "different", r#"{"ok":true}"#)
            .expect_err("duplicate idempotency key must fail");
        assert_eq!(error.code, "IDEMPOTENCY_CONFLICT");
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn schema_five_transaction_usage_credential_and_egress_round_trip() {
        let dir = temp_dir("schema3");
        fs::create_dir_all(&dir).unwrap();
        let storage = RelayStorage::open(dir.join("relay.sqlite3")).unwrap();
        storage
            .register_project(
                Some("PRJ-created"),
                "Transaction Fixture",
                "file:///transaction-fixture",
            )
            .unwrap();

        let transaction = storage
            .begin_transaction(NewTransaction {
                project_id: None,
                request_id: "REQ-write",
                command: "project.register",
                effect_class: "relay_state_write",
                permission: "state_write",
                actor_id: "ACTOR-test",
                client_id: "CLIENT-test",
                delegator_id: Some("USER-owner"),
                request_sha256: "abc123",
                intended_summary: "register project",
                rollback_status: "not_available",
            })
            .unwrap();
        assert_eq!(transaction.state, "REQUESTED");
        let completed = storage
            .finish_transaction(
                &transaction.id,
                "COMPLETED",
                Some("PRJ-created"),
                "verified",
                None,
                None,
                Some("PRJ-created"),
            )
            .unwrap();
        assert_eq!(completed.verification, "verified");
        assert_eq!(completed.after_ref.as_deref(), Some("PRJ-created"));

        storage
            .record_usage(UsageMetricInput {
                request_id: "REQ-write",
                command: "project.register",
                command_version: 1,
                actor_id: "ACTOR-test",
                client_id: "CLIENT-test",
                delegator_id: Some("USER-owner"),
                project_id: None,
                effect_class: "relay_state_write",
                permission: "state_write",
                ok: true,
                replayed: false,
                elapsed_ms: 7,
                request_bytes: 100,
                response_bytes: 80,
                remote_calls: 0,
                model_tokens_in: 0,
                model_tokens_out: 0,
            })
            .unwrap();
        let usage = storage.usage_summary().unwrap();
        assert_eq!(usage.command_count, 1);
        assert_eq!(usage.success_count, 1);
        assert_eq!(usage.total_elapsed_ms, 7);

        let handle = storage
            .upsert_credential_handle(
                "github.connection.fixture",
                "github",
                &["read_repo".to_string(), "egress".to_string()],
                "active",
            )
            .unwrap();
        assert_eq!(handle.scopes.len(), 2);
        storage
            .revoke_credential_handle("github.connection.fixture")
            .unwrap();
        assert_eq!(
            storage
                .get_credential_handle("github.connection.fixture")
                .unwrap()
                .unwrap()
                .status,
            "revoked"
        );

        let egress = storage
            .record_egress(EgressLedgerInput {
                project_id: Some("PRJ-fixture"),
                request_id: "REQ-egress",
                actor_id: "ACTOR-test",
                client_id: "CLIENT-test",
                delegator_id: Some("USER-owner"),
                destination: "remote-ai",
                data_classes: &["project".to_string()],
                modalities: &["text".to_string()],
                source_refs: &["RES-fixture".to_string()],
                purpose: "diagnose",
                approx_bytes: 512,
                approx_tokens: 128,
                credential_handle: Some("github.connection.fixture"),
                decision: "blocked",
                reason: "fixture policy",
            })
            .unwrap();
        assert_eq!(egress.decision, "blocked");
        assert_eq!(egress.source_refs, vec!["RES-fixture"]);

        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn future_and_malformed_stores_fail_without_replacement() {
        let future_dir = temp_dir("future");
        fs::create_dir_all(&future_dir).unwrap();
        let future = future_dir.join("relay.sqlite3");
        {
            let conn = Connection::open(&future).unwrap();
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                   version INTEGER PRIMARY KEY,
                   applied_at TEXT NOT NULL
                 );
                 INSERT INTO schema_migrations VALUES (999,'future');",
            )
            .unwrap();
        }
        let error = RelayStorage::open(&future)
            .err()
            .expect("future schema must fail");
        assert_eq!(error.code, "STORAGE_SCHEMA_FUTURE");
        fs::remove_dir_all(future_dir).unwrap();

        let bad_dir = temp_dir("malformed");
        fs::create_dir_all(&bad_dir).unwrap();
        let bad = bad_dir.join("relay.sqlite3");
        fs::write(&bad, b"not sqlite").unwrap();
        assert!(RelayStorage::open(&bad).is_err());
        assert_eq!(fs::read(&bad).unwrap(), b"not sqlite");
        fs::remove_dir_all(bad_dir).unwrap();
    }
}
