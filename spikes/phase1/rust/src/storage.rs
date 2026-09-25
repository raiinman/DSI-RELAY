use crate::security::random_hex;
use crate::RELAY_VERSION;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};

pub const STORAGE_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize)]
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
        Self { code, message: message.into() }
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

pub struct RelayStorage {
    db_path: PathBuf,
    conn: Connection,
}

fn opaque_id(prefix: &str) -> Result<String, StorageError> {
    random_hex(16)
        .map(|value| format!("{prefix}-{value}"))
        .map_err(|error| StorageError::new("STORAGE_ERROR", error))
}

fn payload_hash(payload_json: &str) -> String {
    let digest = Sha256::digest(payload_json.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn json_text(value: Option<&Value>) -> Result<String, StorageError> {
    serde_json::to_string(value.unwrap_or(&Value::Null))
        .map_err(|error| StorageError::new("STORAGE_ERROR", format!("serialize JSON: {error}")))
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
                StorageError::new("STORAGE_ERROR", format!("create database directory: {error}"))
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
                    "Storage schema {current} is newer than supported {STORAGE_SCHEMA_VERSION}"
                ),
            ));
        }

        if current < 1 {
            let applied_at = sqlite_now(&self.conn)?;
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|error| StorageError::sqlite("begin migration", error))?;

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
                CREATE INDEX results_project_created ON results(project_id, created_at);
                CREATE INDEX jobs_project_updated ON jobs(project_id, updated_at);",
            )
            .map_err(|error| StorageError::sqlite("apply schema migration 1", error))?;

            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                params![1i64, applied_at],
            )
            .map_err(|error| StorageError::sqlite("record schema migration 1", error))?;
            tx.commit()
                .map_err(|error| StorageError::sqlite("commit schema migration 1", error))?;
        }
        Ok(())
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
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
        let schema_version = self.schema_version()?;
        let sqlite_version = self.sqlite_version()?;
        Ok(StorageHealth {
            ok: check == "ok",
            check,
            schema_version: Some(schema_version),
            sqlite_version: Some(sqlite_version),
            error: None,
        })
    }

    pub fn register_project(&self, args: &Value) -> Result<Value, StorageError> {
        let name = args
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| StorageError::new("VALIDATION_FAILED", "name and root_uri are required"))?;
        let root_uri = args
            .get("root_uri")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| StorageError::new("VALIDATION_FAILED", "name and root_uri are required"))?;
        let id = match args.get("id").and_then(Value::as_str) {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => opaque_id("PRJ")?,
        };
        let now = sqlite_now(&self.conn)?;

        self.conn
            .execute(
                "INSERT INTO projects(id, name, root_uri, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                   name=excluded.name,
                   root_uri=excluded.root_uri,
                   updated_at=excluded.updated_at",
                params![id, name, root_uri, now, now],
            )
            .map_err(|error| StorageError::sqlite("register project", error))?;

        self.get_project(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "project disappeared after write"))
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Value>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, name, root_uri, created_at, updated_at FROM projects WHERE id = ?1",
                [id],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "name": row.get::<_, String>(1)?,
                        "root_uri": row.get::<_, String>(2)?,
                        "created_at": row.get::<_, String>(3)?,
                        "updated_at": row.get::<_, String>(4)?
                    }))
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read project", error))
    }

    pub fn list_projects(&self) -> Result<Vec<Value>, StorageError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, name, root_uri, created_at, updated_at
                 FROM projects ORDER BY created_at, id",
            )
            .map_err(|error| StorageError::sqlite("prepare project list", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "root_uri": row.get::<_, String>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "updated_at": row.get::<_, String>(4)?
                }))
            })
            .map_err(|error| StorageError::sqlite("query project list", error))?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(
                row.map_err(|error| StorageError::sqlite("decode project row", error))?,
            );
        }
        Ok(projects)
    }

    pub fn put_result(&self, args: &Value) -> Result<Value, StorageError> {
        let id = opaque_id("RES")?;
        let project_id = args.get("project_id").and_then(Value::as_str);
        let kind = args
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("GENERIC");
        let payload_json = json_text(args.get("payload"))?;
        let payload_sha256 = payload_hash(&payload_json);
        let created_at = sqlite_now(&self.conn)?;

        self.conn
            .execute(
                "INSERT INTO results(
                    id, project_id, kind, schema_version, producer_version,
                    payload_json, payload_sha256, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    project_id,
                    kind,
                    STORAGE_SCHEMA_VERSION,
                    RELAY_VERSION,
                    payload_json,
                    payload_sha256,
                    created_at
                ],
            )
            .map_err(|error| StorageError::sqlite("store result", error))?;

        self.get_result(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "result disappeared after write"))
    }

    pub fn get_result(&self, id: &str) -> Result<Option<Value>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, kind, schema_version, producer_version,
                        payload_json, payload_sha256, created_at
                 FROM results WHERE id = ?1",
                [id],
                |row| {
                    let payload_json: String = row.get(5)?;
                    let payload: Value =
                        serde_json::from_str(&payload_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "project_id": row.get::<_, Option<String>>(1)?,
                        "kind": row.get::<_, String>(2)?,
                        "schema_version": row.get::<_, i64>(3)?,
                        "producer_version": row.get::<_, String>(4)?,
                        "payload_json": payload_json,
                        "payload_sha256": row.get::<_, String>(6)?,
                        "created_at": row.get::<_, String>(7)?,
                        "payload": payload
                    }))
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read result", error))
    }

    pub fn checkpoint_job(&self, args: &Value) -> Result<Value, StorageError> {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| StorageError::new("VALIDATION_FAILED", "command and state are required"))?;
        let state = args
            .get("state")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| StorageError::new("VALIDATION_FAILED", "command and state are required"))?;
        let id = match args.get("id").and_then(Value::as_str) {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => opaque_id("JOB")?,
        };
        let project_id = args.get("project_id").and_then(Value::as_str);
        let result_id = args.get("result_id").and_then(Value::as_str);
        let checkpoint_value = args
            .get("checkpoint")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let checkpoint_json = json_text(Some(&checkpoint_value))?;
        let now = sqlite_now(&self.conn)?;

        self.conn
            .execute(
                "INSERT INTO jobs(
                    id, project_id, command, state, checkpoint_json, result_id,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   state=excluded.state,
                   checkpoint_json=excluded.checkpoint_json,
                   result_id=excluded.result_id,
                   updated_at=excluded.updated_at",
                params![id, project_id, command, state, checkpoint_json, result_id, now, now],
            )
            .map_err(|error| StorageError::sqlite("checkpoint job", error))?;

        self.get_job(&id)?
            .ok_or_else(|| StorageError::new("STORAGE_ERROR", "job disappeared after write"))
    }

    pub fn get_job(&self, id: &str) -> Result<Option<Value>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, project_id, command, state, checkpoint_json, result_id,
                        created_at, updated_at
                 FROM jobs WHERE id = ?1",
                [id],
                |row| {
                    let checkpoint_json: String = row.get(4)?;
                    let checkpoint: Value =
                        serde_json::from_str(&checkpoint_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "project_id": row.get::<_, Option<String>>(1)?,
                        "command": row.get::<_, String>(2)?,
                        "state": row.get::<_, String>(3)?,
                        "checkpoint_json": checkpoint_json,
                        "result_id": row.get::<_, Option<String>>(5)?,
                        "created_at": row.get::<_, String>(6)?,
                        "updated_at": row.get::<_, String>(7)?,
                        "checkpoint": checkpoint
                    }))
                },
            )
            .optional()
            .map_err(|error| StorageError::sqlite("read job", error))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "relay-rust-storage-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    #[test]
    fn durable_round_trip_reopens_with_schema_one() {
        let dir = temp_dir("roundtrip");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        let result_id;
        let job_id;

        {
            let storage = RelayStorage::open(&path).expect("open storage");
            let project = storage
                .register_project(&json!({
                    "id": "PRJ-rust-storage-fixture",
                    "name": "Rust Storage Fixture",
                    "root_uri": "file:///relay-rust-storage-fixture"
                }))
                .expect("project");
            assert_eq!(project["id"], "PRJ-rust-storage-fixture");

            let result = storage
                .put_result(&json!({
                    "project_id": "PRJ-rust-storage-fixture",
                    "kind": "TEST",
                    "payload": { "exact_number": 42, "message": "durable round trip" }
                }))
                .expect("result");
            assert_eq!(result["payload"]["exact_number"], 42);
            assert_eq!(result["payload_sha256"].as_str().unwrap().len(), 64);
            result_id = result["id"].as_str().unwrap().to_string();

            let job = storage
                .checkpoint_job(&json!({
                    "project_id": "PRJ-rust-storage-fixture",
                    "command": "fixture.work",
                    "state": "CHECKPOINTED",
                    "checkpoint": { "completed_stage": 2 },
                    "result_id": result_id
                }))
                .expect("checkpoint");
            job_id = job["id"].as_str().unwrap().to_string();

            let health = storage.integrity().expect("integrity");
            assert!(health.ok);
            assert_eq!(health.schema_version, Some(1));
            assert!(!health.sqlite_version.unwrap().is_empty());
        }

        let storage = RelayStorage::open(&path).expect("reopen storage");
        let result = storage
            .get_result(&result_id)
            .expect("read result")
            .expect("stored result");
        assert_eq!(result["payload"]["exact_number"], 42);
        let job = storage
            .get_job(&job_id)
            .expect("read job")
            .expect("stored job");
        assert_eq!(job["checkpoint"]["completed_stage"], 2);
        assert_eq!(storage.list_projects().unwrap().len(), 1);
        drop(storage);
        fs::remove_dir_all(dir).unwrap();
    }


    #[test]
    fn future_schema_is_rejected() {
        let dir = temp_dir("future");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                 );
                 INSERT INTO schema_migrations(version, applied_at)
                 VALUES (999, 'future');",
            )
            .unwrap();
        }

        let error = RelayStorage::open(&path)
            .err()
            .expect("future schema must fail");
        assert_eq!(error.code, "STORAGE_SCHEMA_FUTURE");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_database_is_rejected_without_replacement() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.sqlite3");
        fs::write(&path, b"this is not sqlite").unwrap();

        let error = RelayStorage::open(&path)
            .err()
            .expect("corrupt store must fail");
        assert!(error.message.contains("database") || error.message.contains("file"));
        assert_eq!(fs::read(&path).unwrap(), b"this is not sqlite");
        fs::remove_dir_all(dir).unwrap();
    }
}
