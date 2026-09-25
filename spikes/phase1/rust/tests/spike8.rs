#![cfg(windows)]

use relay_rust_challenger::pipe;
use relay_rust_challenger::state::{read_state, HostState};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn unique_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-rust-spike8-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn spawn_host(dir: &Path, instance: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_relay-rust-challenger"))
        .arg("host")
        .env("RELAY_STATE_DIR", dir)
        .env("RELAY_INSTANCE", instance)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn host")
}

fn wait_state(dir: &Path, expected_pid: u32) -> HostState {
    let path = dir.join("host-rust.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(state) = read_state(&path) {
            if state.pid == expected_pid {
                return state;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("state did not become current for PID {expected_pid}");
}

fn call(state: &HostState, command: &str, arguments: Value) -> Value {
    pipe::call(&state.pipe, &state.auth_token, command, arguments, 3000)
        .expect("host call")
}

fn shutdown(state: &HostState, child: &mut Child) {
    let response = call(state, "system.shutdown", json!({}));
    assert_eq!(response["ok"], true);
    let status = child.wait().expect("wait host");
    assert!(status.success());
}

#[test]
fn durable_operational_state_survives_hard_kill() {
    let dir = unique_dir("durability");
    fs::create_dir_all(&dir).unwrap();
    let instance = "spike8-durability";
    let mut first = spawn_host(&dir, instance);
    let first_state = wait_state(&dir, first.id());
    assert_eq!(first_state.recovery_state, "Healthy");
    assert_eq!(first_state.storage_schema_version, Some(1));

    let project = call(
        &first_state,
        "project.register",
        json!({
            "id": "PRJ-spike8-fixture",
            "name": "Spike 8 Fixture",
            "root_uri": "file:///relay-spike8-fixture"
        }),
    );
    assert_eq!(project["ok"], true);

    let result = call(
        &first_state,
        "result.put",
        json!({
            "project_id": "PRJ-spike8-fixture",
            "kind": "TEST",
            "payload": { "exact_number": 42, "message": "hard kill persistence" }
        }),
    );
    assert_eq!(result["ok"], true);
    let result_id = result["result"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        result["result"]["payload_sha256"].as_str().unwrap().len(),
        64
    );

    let checkpoint = call(
        &first_state,
        "job.checkpoint",
        json!({
            "project_id": "PRJ-spike8-fixture",
            "command": "fixture.work",
            "state": "CHECKPOINTED",
            "checkpoint": { "completed_stage": 2 },
            "result_id": result_id
        }),
    );
    assert_eq!(checkpoint["ok"], true);
    let job_id = checkpoint["result"]["id"].as_str().unwrap().to_string();

    let integrity = call(&first_state, "storage.integrity", json!({}));
    assert_eq!(integrity["result"]["ok"], true);
    assert_eq!(integrity["result"]["schema_version"], 1);

    first.kill().expect("hard kill");
    first.wait().expect("wait killed host");
    assert!(dir.join("host-rust.json").exists());

    let mut second = spawn_host(&dir, instance);
    let second_state = wait_state(&dir, second.id());
    assert_eq!(second_state.recovery_state, "Healthy");

    let result_get = call(
        &second_state,
        "result.get",
        json!({ "result_id": result_id }),
    );
    assert_eq!(result_get["ok"], true);
    assert_eq!(result_get["result"]["payload"]["exact_number"], 42);

    let job_get = call(&second_state, "job.get", json!({ "job_id": job_id }));
    assert_eq!(job_get["ok"], true);
    assert_eq!(job_get["result"]["checkpoint"]["completed_stage"], 2);

    let projects = call(&second_state, "project.list", json!({}));
    assert_eq!(projects["result"]["projects"].as_array().unwrap().len(), 1);

    let doctor = call(&second_state, "system.doctor", json!({}));
    assert_eq!(doctor["result"]["healthy"], true);
    shutdown(&second_state, &mut second);
    fs::remove_dir_all(&dir).unwrap();
}


#[test]
fn malformed_store_starts_degraded_and_blocks_writes() {
    let dir = unique_dir("corrupt");
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("relay.sqlite3");
    let original = b"this is not sqlite";
    fs::write(&db, original).unwrap();

    let mut child = spawn_host(&dir, "spike8-corrupt");
    let state = wait_state(&dir, child.id());
    assert_eq!(state.recovery_state, "Degraded");
    assert_eq!(state.storage_schema_version, None);

    let status = call(&state, "system.status", json!({}));
    assert_eq!(status["result"]["recovery_state"], "Degraded");
    let doctor = call(&state, "system.doctor", json!({}));
    assert_eq!(doctor["result"]["healthy"], false);

    let write = call(
        &state,
        "result.put",
        json!({ "kind": "TEST", "payload": { "should_not": "write" } }),
    );
    assert_eq!(write["ok"], false);
    assert_eq!(write["error"]["code"], "STORAGE_UNAVAILABLE");
    assert_eq!(fs::read(&db).unwrap(), original);

    shutdown(&state, &mut child);
    fs::remove_dir_all(&dir).unwrap();
}


#[test]
fn future_schema_starts_degraded_without_mutation() {
    let dir = unique_dir("future");
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("relay.sqlite3");
    {
        let conn = Connection::open(&db).unwrap();
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

    let mut child = spawn_host(&dir, "spike8-future");
    let state = wait_state(&dir, child.id());
    assert_eq!(state.recovery_state, "Degraded");

    let status = call(&state, "system.status", json!({}));
    assert_eq!(status["result"]["recovery_state"], "Degraded");
    let projects = call(&state, "project.list", json!({}));
    assert_eq!(projects["ok"], false);
    assert_eq!(projects["error"]["code"], "STORAGE_UNAVAILABLE");

    let conn = Connection::open(&db).unwrap();
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 999);
    drop(conn);

    shutdown(&state, &mut child);
    fs::remove_dir_all(&dir).unwrap();
}
