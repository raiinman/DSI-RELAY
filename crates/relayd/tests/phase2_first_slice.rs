#![cfg(windows)]

use relay::client;
use relay_contracts::{
    CommandRequest, CommandResponse, LocalHostState, RequestContext,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn unique_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase2-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn spawn_host(dir: &Path, instance: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_TEST_DISABLE_WATCHER", "1")
        .env("RELAY_STATE_DIR", dir)
        .env("RELAY_INSTANCE", instance)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn relayd")
}

fn wait_state(dir: &Path, expected_pid: u32) -> LocalHostState {
    let state_path = dir.join("host.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(&state_path) {
            if let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes) {
                if state.pid == expected_pid {
                    return state;
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("host state did not become current for PID {expected_pid}");
}

fn request(
    request_id: impl Into<String>,
    command: impl Into<String>,
    command_version: Option<u32>,
    arguments: Value,
    idempotency_key: Option<&str>,
) -> CommandRequest {
    CommandRequest {
        request_id: request_id.into(),
        command: command.into(),
        command_version,
        arguments,
        idempotency_key: idempotency_key.map(str::to_string),
        context: RequestContext::default(),
    }
}

fn call(
    state: &LocalHostState,
    request: CommandRequest,
) -> CommandResponse {
    client::call(state, &request).expect("client call")
}

fn shutdown(state: &LocalHostState, child: &mut Child) {
    let response = call(
        state,
        request(
            "REQ-shutdown",
            "system.shutdown",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(response.ok);
    assert!(child.wait().expect("wait relayd").success());
}

fn assert_durable_state(
    state: &LocalHostState,
    result_id: &str,
    job_id: &str,
) {
    let result = call(
        state,
        request(
            "REQ-result-check",
            "result.get",
            Some(1),
            json!({ "result_id": result_id }),
            None,
        ),
    );
    assert!(result.ok);
    assert_eq!(
        result.result.as_ref().unwrap()["payload"]["exact_number"],
        42
    );

    let job = call(
        state,
        request(
            "REQ-job-check",
            "job.get",
            Some(1),
            json!({ "job_id": job_id }),
            None,
        ),
    );
    assert!(job.ok);
    assert_eq!(
        job.result.as_ref().unwrap()["checkpoint"]["stage"],
        2
    );
}

#[test]
fn first_slice_survives_graceful_and_hard_restart() {
    let dir = unique_dir("vertical");
    fs::create_dir_all(&dir).unwrap();
    let instance = "phase2-first-slice";

    let mut first = spawn_host(&dir, instance);
    let first_state = wait_state(&dir, first.id());

    let status = call(
        &first_state,
        request(
            "REQ-status",
            "system.status",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(status.ok);
    assert_eq!(
        status.result.as_ref().unwrap()["recovery_state"],
        "Healthy"
    );

    let doctor = call(
        &first_state,
        request(
            "REQ-doctor",
            "system.doctor",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(doctor.ok);
    assert_eq!(doctor.result.as_ref().unwrap()["healthy"], true);

    let project = call(
        &first_state,
        request(
            "REQ-project",
            "project.register",
            Some(1),
            json!({
                "id": "PRJ-phase2-e2e",
                "name": "Phase 2 E2E",
                "root_uri": "file:///phase2-e2e"
            }),
            Some("IDEM-project-e2e"),
        ),
    );
    assert!(project.ok);
    assert_eq!(
        project.result.as_ref().unwrap()["id"],
        "PRJ-phase2-e2e"
    );

    let result_request = request(
        "REQ-result-1",
        "result.put",
        Some(1),
        json!({
            "project_id": "PRJ-phase2-e2e",
            "kind": "PHASE2_E2E",
            "payload": {
                "exact_number": 42,
                "message": "persistent phase2 result"
            }
        }),
        Some("IDEM-result-e2e"),
    );
    let result_one = call(&first_state, result_request);
    assert!(result_one.ok);
    let result_id = result_one.result.as_ref().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let replay = call(
        &first_state,
        request(
            "REQ-result-2",
            "result.put",
            Some(1),
            json!({
                "project_id": "PRJ-phase2-e2e",
                "kind": "PHASE2_E2E",
                "payload": {
                    "exact_number": 42,
                    "message": "persistent phase2 result"
                }
            }),
            Some("IDEM-result-e2e"),
        ),
    );
    assert!(replay.ok);
    assert!(replay.replayed);
    assert_eq!(
        replay.result.as_ref().unwrap()["id"],
        result_id
    );

    let job = call(
        &first_state,
        request(
            "REQ-job",
            "job.checkpoint",
            Some(1),
            json!({
                "project_id": "PRJ-phase2-e2e",
                "command": "phase2.e2e",
                "state": "CHECKPOINTED",
                "checkpoint": { "stage": 2, "verified": true },
                "result_id": result_id
            }),
            Some("IDEM-job-e2e"),
        ),
    );
    assert!(job.ok);
    let job_id = job.result.as_ref().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let incompatible = call(
        &first_state,
        request(
            "REQ-version-bad",
            "system.status",
            Some(99),
            json!({}),
            None,
        ),
    );
    assert!(!incompatible.ok);
    assert_eq!(
        incompatible.error.as_ref().unwrap().code,
        "COMMAND_VERSION_INCOMPATIBLE"
    );

    shutdown(&first_state, &mut first);
    assert!(!dir.join("host.json").exists());

    let mut second = spawn_host(&dir, instance);
    let second_state = wait_state(&dir, second.id());

    let projects = call(
        &second_state,
        request(
            "REQ-project-list-after-graceful",
            "project.list",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(projects.ok);
    assert_eq!(
        projects.result.as_ref().unwrap()["projects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    assert_durable_state(&second_state, &result_id, &job_id);

    second.kill().expect("hard kill relayd");
    second.wait().expect("wait hard-killed relayd");
    assert!(dir.join("host.json").exists());

    let mut third = spawn_host(&dir, instance);
    let third_state = wait_state(&dir, third.id());
    assert_eq!(third_state.recovery_state, "Healthy");
    assert_durable_state(&third_state, &result_id, &job_id);

    let doctor_after_restart = call(
        &third_state,
        request(
            "REQ-doctor-after-hard-kill",
            "system.doctor",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(doctor_after_restart.ok);
    assert_eq!(
        doctor_after_restart.result.as_ref().unwrap()["healthy"],
        true
    );

    shutdown(&third_state, &mut third);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn damaged_storage_degrades_without_overwrite() {
    let dir = unique_dir("damaged-storage");
    fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("relay.sqlite3");
    let damaged = b"this is not sqlite";
    fs::write(&db_path, damaged).unwrap();

    let mut child = spawn_host(&dir, "phase2-damaged-storage");
    let state = wait_state(&dir, child.id());
    assert_eq!(state.recovery_state, "Degraded");

    let status = call(
        &state,
        request(
            "REQ-damaged-status",
            "system.status",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(status.ok);
    assert_eq!(
        status.result.as_ref().unwrap()["recovery_state"],
        "Degraded"
    );
    assert_eq!(
        status.result.as_ref().unwrap()["storage"]["ok"],
        false
    );
    assert_eq!(
        status.result.as_ref().unwrap()["diagnostics"]["ok"],
        true
    );

    let projects = call(
        &state,
        request(
            "REQ-damaged-projects",
            "project.list",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(!projects.ok);
    assert_eq!(
        projects.error.as_ref().unwrap().code,
        "STORAGE_UNAVAILABLE"
    );

    let doctor = call(
        &state,
        request(
            "REQ-damaged-doctor",
            "system.doctor",
            Some(1),
            json!({}),
            None,
        ),
    );
    assert!(doctor.ok);
    assert_eq!(doctor.result.as_ref().unwrap()["healthy"], false);
    assert_eq!(fs::read(&db_path).unwrap(), damaged);

    shutdown(&state, &mut child);
    fs::remove_dir_all(dir).unwrap();
}
