use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STORAGE_SCHEMA_VERSION: i64 = 3;
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
    let material = format!(
        "{}:{}:{}:{}",
        prefix,
        std::process::id(),
        now,
        counter
    );
    let digest = Sha256::digest(material.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}")
            .expect("writing to String cannot fail");
    }
    format!("{prefix}-{hex}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}")
            .expect("writing to String cannot fail");
    }
    output
}
fn json_text(value: &Value) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|error| {
        StorageError::new(
            "STORAGE_ERROR",
            format!("serialize JSON: {error}"),
        )
    })
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
    conn.query_row(
        "SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        [],
        |row| row.get(0),
    )
    .map_err(|error| StorageError::sqlite("read SQLite clock", error))
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
            .map_err(|error| {
                StorageError::sqlite("configure database", error)
            })
    }

    fn migrate(&mut self) -> Result<(), StorageError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );",
            )
            .map_err(|error| {
                StorageError::sqlite("create migration table", error)
            })?;

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
        Ok(())
    }

    fn apply_schema_one(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                StorageError::sqlite("begin schema-1 migration", error)
            })?;

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
        .map_err(|error| {
            StorageError::sqlite("create schema-1 tables", error)
        })?;
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
        .map_err(|error| {
            StorageError::sqlite("create schema-1 job tables", error)
        })?;

        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![1i64, applied_at],
        )
        .map_err(|error| {
            StorageError::sqlite("record schema migration 1", error)
        })?;
        tx.commit().map_err(|error| {
            StorageError::sqlite("commit schema migration 1", error)
        })
    }
    fn apply_schema_two(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                StorageError::sqlite("begin schema-2 migration", error)
            })?;

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
        .map_err(|error| {
            StorageError::sqlite("apply schema migration 2", error)
        })?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![2i64, applied_at],
        )
        .map_err(|error| {
            StorageError::sqlite("record schema migration 2", error)
        })?;
        tx.commit().map_err(|error| {
            StorageError::sqlite("commit schema migration 2", error)
        })
    }

    fn apply_schema_three(&mut self) -> Result<(), StorageError> {
        let applied_at = sqlite_now(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                StorageError::sqlite("begin schema-3 migration", error)
            })?;

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
        .map_err(|error| {
            StorageError::sqlite("create schema-3 transaction/usage tables", error)
        })?;

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
        .map_err(|error| {
            StorageError::sqlite("create schema-3 credential/egress tables", error)
        })?;

        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at)
             VALUES (?1, ?2)",
            params![3i64, applied_at],
        )
        .map_err(|error| {
            StorageError::sqlite("record schema migration 3", error)
        })?;
        tx.commit().map_err(|error| {
            StorageError::sqlite("commit schema migration 3", error)
        })
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
            .map_err(|error| {
                StorageError::sqlite("read schema version", error)
            })
    }

    pub fn sqlite_version(&self) -> Result<String, StorageError> {
        self.conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(|error| {
                StorageError::sqlite("read SQLite version", error)
            })
    }

    pub fn integrity(&self) -> Result<StorageHealth, StorageError> {
        let check: String = self.conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| {
                StorageError::sqlite("run quick_check", error)
            })?;
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
        self.conn
            .execute(
                "INSERT INTO projects(
                    id, name, root_uri, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                   name=excluded.name,
                   root_uri=excluded.root_uri,
                   updated_at=excluded.updated_at",
                params![id, name, root_uri, now, now],
            )
            .map_err(|error| {
                StorageError::sqlite("register project", error)
            })?;
        self.get_project(&id)?
            .ok_or_else(|| StorageError::new(
                "STORAGE_ERROR",
                "project disappeared after write",
            ))
    }
    pub fn get_project(
        &self,
        id: &str,
    ) -> Result<Option<ProjectRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, name, root_uri, created_at, updated_at
                 FROM projects WHERE id = ?1",
                [id],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_uri: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                StorageError::sqlite("read project", error)
            })
    }

    pub fn list_projects(
        &self,
    ) -> Result<Vec<ProjectRecord>, StorageError> {
        let mut statement = self.conn
            .prepare(
                "SELECT id, name, root_uri, created_at, updated_at
                 FROM projects ORDER BY created_at, id",
            )
            .map_err(|error| {
                StorageError::sqlite("prepare project list", error)
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_uri: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|error| {
                StorageError::sqlite("query project list", error)
            })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row.map_err(|error| {
                StorageError::sqlite("decode project row", error)
            })?);
        }
        Ok(projects)
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
            .map_err(|error| {
                StorageError::sqlite("store result", error)
            })?;

        self.get_result(&id)?
            .ok_or_else(|| StorageError::new(
                "STORAGE_ERROR",
                "result disappeared after write",
            ))
    }
    pub fn get_result(
        &self,
        id: &str,
    ) -> Result<Option<ResultRecord>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, kind, schema_version,
                        producer_version, payload_json, payload_sha256,
                        provenance_json, trust, created_at
                 FROM results WHERE id = ?1",
                [id],
                |row| {
                    let payload_json: String = row.get(5)?;
                    let provenance_json: String = row.get(7)?;
                    Ok(ResultRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        schema_version: row.get(3)?,
                        producer_version: row.get(4)?,
                        payload: parse_json(&payload_json, 5)?,
                        payload_sha256: row.get(6)?,
                        provenance: parse_json(&provenance_json, 7)?,
                        trust: row.get(8)?,
                        created_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                StorageError::sqlite("read result", error)
            })
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
            .map_err(|error| {
                StorageError::sqlite("checkpoint job", error)
            })?;

        self.get_job(&id)?
            .ok_or_else(|| StorageError::new(
                "STORAGE_ERROR",
                "job disappeared after write",
            ))
    }

    pub fn get_job(
        &self,
        id: &str,
    ) -> Result<Option<JobRecord>, StorageError> {
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
            .map_err(|error| {
                StorageError::sqlite("read job", error)
            })
    }

    pub fn get_idempotency(
        &self,
        key: &str,
    ) -> Result<Option<IdempotencyRecord>, StorageError> {
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
            .map_err(|error| {
                StorageError::sqlite("read idempotency record", error)
            })
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
                params![
                    key,
                    command,
                    request_sha256,
                    response_json,
                    created_at
                ],
            )
            .map_err(|error| {
                if matches!(
                    error.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::ConstraintViolation)
                ) {
                    StorageError::new(
                        "IDEMPOTENCY_CONFLICT",
                        "idempotency key already exists",
                    )
                } else {
                    StorageError::sqlite(
                        "store idempotency record",
                        error,
                    )
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
            .map_err(|error| {
                StorageError::sqlite("begin transaction record", error)
            })?;
        self.get_transaction(&id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "transaction disappeared after write",
            )
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
                    id, state, after_ref, verification,
                    result_id, job_id, now, project_id
                ],
            )
            .map_err(|error| {
                StorageError::sqlite("finish transaction record", error)
            })?;
        self.get_transaction(id)?.ok_or_else(|| {
            StorageError::new("TRANSACTION_NOT_FOUND", "transaction not found")
        })
    }
    pub fn get_transaction(
        &self,
        id: &str,
    ) -> Result<Option<TransactionRecord>, StorageError> {
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
            .map_err(|error| {
                StorageError::sqlite("read transaction record", error)
            })
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
            .map_err(|error| {
                StorageError::sqlite("prepare transaction list", error)
            })?;
        let ids = statement
            .query_map(params![project_id, limit], |row| row.get::<_, String>(0))
            .map_err(|error| {
                StorageError::sqlite("query transaction list", error)
            })?;
        let mut output = Vec::new();
        for id in ids {
            if let Some(record) = self.get_transaction(&id.map_err(|error| {
                StorageError::sqlite("decode transaction ID", error)
            })?)? {
                output.push(record);
            }
        }
        Ok(output)
    }

    pub fn record_usage(
        &self,
        input: UsageMetricInput<'_>,
    ) -> Result<(), StorageError> {
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
            .map_err(|error| {
                StorageError::sqlite("record usage metric", error)
            })?;
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
            .map_err(|error| {
                StorageError::sqlite("read usage summary", error)
            })
    }

    pub fn upsert_credential_handle(
        &self,
        id: &str,
        integration: &str,
        scopes: &[String],
        status: &str,
    ) -> Result<CredentialHandleRecord, StorageError> {
        let now = sqlite_now(&self.conn)?;
        let scopes_json = json_text(
            &serde_json::to_value(scopes).map_err(|error| {
                StorageError::new(
                    "STORAGE_ERROR",
                    format!("serialize credential scopes: {error}"),
                )
            })?,
        )?;
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
            .map_err(|error| {
                StorageError::sqlite(
                    "store credential handle metadata",
                    error,
                )
            })?;
        self.get_credential_handle(id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "credential handle disappeared after write",
            )
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
            .map_err(|error| {
                StorageError::sqlite(
                    "read credential handle metadata",
                    error,
                )
            })
    }

    pub fn revoke_credential_handle(
        &self,
        id: &str,
    ) -> Result<(), StorageError> {
        let now = sqlite_now(&self.conn)?;
        let changed = self.conn
            .execute(
                "UPDATE credential_handles
                 SET status='revoked', updated_at=?2
                 WHERE id=?1",
                params![id, now],
            )
            .map_err(|error| {
                StorageError::sqlite(
                    "revoke credential handle metadata",
                    error,
                )
            })?;
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
        let data_classes_json =
            serde_json::to_string(input.data_classes).map_err(|error| {
                StorageError::new(
                    "STORAGE_ERROR",
                    format!("serialize egress data classes: {error}"),
                )
            })?;
        let modalities_json =
            serde_json::to_string(input.modalities).map_err(|error| {
                StorageError::new(
                    "STORAGE_ERROR",
                    format!("serialize egress modalities: {error}"),
                )
            })?;
        let source_refs_json =
            serde_json::to_string(input.source_refs).map_err(|error| {
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
            .map_err(|error| {
                StorageError::sqlite("record egress ledger entry", error)
            })?;
        self.get_egress(&id)?.ok_or_else(|| {
            StorageError::new(
                "STORAGE_ERROR",
                "egress ledger entry disappeared after write",
            )
        })
    }

    pub fn get_egress(
        &self,
        id: &str,
    ) -> Result<Option<EgressLedgerRecord>, StorageError> {
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
                        data_classes: serde_json::from_str(
                            &data_classes_json,
                        )
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        modalities: serde_json::from_str(&modalities_json)
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    8,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        source_refs: serde_json::from_str(&source_refs_json)
                            .map_err(|error| {
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
            .map_err(|error| {
                StorageError::sqlite("read egress ledger entry", error)
            })
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
        ).unwrap();
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
        ).unwrap();
        conn.execute(
            "INSERT INTO projects VALUES (?1,?2,?3,?4,?5)",
            params![
                "PRJ-phase1",
                "Phase 1 Fixture",
                "file:///fixture",
                "2026-09-25T00:00:00Z",
                "2026-09-25T00:00:00Z"
            ],
        ).unwrap();
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
        ).unwrap();
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
        ).unwrap();
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
    fn fresh_store_uses_schema_three_and_typed_records() {
        let dir = temp_dir("fresh");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), 3);
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
        assert_eq!(storage.schema_version().unwrap(), 3);
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
    fn schema_two_fixture_migrates_to_three_without_replay_loss() {
        let dir = temp_dir("schema2");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        create_schema_two_fixture(&path);

        let storage = RelayStorage::open(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), 3);
        let result = storage.get_result("RES-phase1").unwrap().unwrap();
        assert_eq!(result.payload["value"], 42);
        assert_eq!(result.trust, "local");
        let replay = storage
            .get_idempotency("IDEMP-schema2")
            .unwrap()
            .unwrap();
        assert_eq!(replay.command, "result.put");
        assert_eq!(replay.request_sha256, "fixture-sha");
        assert_eq!(storage.usage_summary().unwrap().command_count, 0);
        assert!(storage.list_transactions(None, 10).unwrap().is_empty());

        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn idempotency_record_round_trip_and_conflict() {
        let dir = temp_dir("idempotency");
        fs::create_dir_all(&dir).unwrap();
        let storage =
            RelayStorage::open(dir.join("relay.sqlite3")).unwrap();

        storage
            .put_idempotency(
                "IDEMP-fixture",
                "result.put",
                "abc123",
                r#"{"ok":true}"#,
            )
            .unwrap();
        let record = storage
            .get_idempotency("IDEMP-fixture")
            .unwrap()
            .unwrap();
        assert_eq!(record.command, "result.put");
        assert_eq!(record.request_sha256, "abc123");

        let error = storage
            .put_idempotency(
                "IDEMP-fixture",
                "result.put",
                "different",
                r#"{"ok":true}"#,
            )
            .expect_err("duplicate idempotency key must fail");
        assert_eq!(error.code, "IDEMPOTENCY_CONFLICT");
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn schema_three_transaction_usage_credential_and_egress_round_trip() {
        let dir = temp_dir("schema3");
        fs::create_dir_all(&dir).unwrap();
        let storage =
            RelayStorage::open(dir.join("relay.sqlite3")).unwrap();
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
            ).unwrap();
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
