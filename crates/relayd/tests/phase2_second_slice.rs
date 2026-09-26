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
        "relay-phase2-second-{label}-{}-{nanos}",
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
    let path = dir.join("host.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(&path) {
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
    id: &str,
    command: &str,
    arguments: Value,
    idempotency_key: Option<&str>,
) -> CommandRequest {
    CommandRequest {
        request_id: id.to_string(),
        command: command.to_string(),
        command_version: Some(1),
        arguments,
        idempotency_key: idempotency_key.map(str::to_string),
        context: RequestContext {
            actor_id: Some("ATTACKER".to_string()),
            client_id: Some("CLIENT-attacker".to_string()),
            delegator_id: Some("ATTACKER-owner".to_string()),
        },
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
        request("REQ-shutdown", "system.shutdown", json!({}), None),
    );
    assert!(response.ok);
    assert!(child.wait().expect("wait relayd").success());
}


#[test]
fn authority_transactions_usage_and_egress_survive_restart() {
    let dir = unique_dir("vertical");
    fs::create_dir_all(&dir).unwrap();
    let instance = "phase2-second-slice";

    let mut first = spawn_host(&dir, instance);
    let first_state = wait_state(&dir, first.id());

    let project = call(
        &first_state,
        request(
            "REQ-project",
            "project.register",
            json!({
                "id": "PRJ-phase2-second",
                "name": "Phase 2 Second Slice",
                "root_uri": "file:///phase2-second"
            }),
            Some("IDEM-project-second"),
        ),
    );
    assert!(project.ok);

    let mut result_request = request(
        "REQ-result-one",
        "result.put",
        json!({
            "project_id": "PRJ-phase2-second",
            "kind": "SECOND_SLICE",
            "payload": { "exact_number": 88 }
        }),
        Some("IDEM-result-second"),
    );
    let first_result = call(&first_state, result_request.clone());
    assert!(first_result.ok);
    assert!(!first_result.replayed);
    let result_id = first_result.result.as_ref().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    result_request.request_id = "REQ-result-replay".to_string();
    let replay = call(&first_state, result_request);
    assert!(replay.ok);
    assert!(replay.replayed);
    assert_eq!(replay.result.as_ref().unwrap()["id"], result_id);


    let transactions = call(
        &first_state,
        request(
            "REQ-transactions",
            "transaction.list",
            json!({
                "project_id": "PRJ-phase2-second",
                "limit": 50
            }),
            None,
        ),
    );
    assert!(transactions.ok);
    let rows = transactions.result.as_ref().unwrap()["transactions"]
        .as_array()
        .unwrap();
    let project_tx = rows
        .iter()
        .find(|row| row["command"] == "project.register")
        .expect("project transaction");
    assert_eq!(project_tx["actor_id"], "local-user");
    assert_eq!(project_tx["client_id"], "relay-cli");
    assert!(project_tx["delegator_id"].is_null());

    let result_tx_count = rows
        .iter()
        .filter(|row| row["command"] == "result.put")
        .count();
    assert_eq!(result_tx_count, 1);

    let usage = call(
        &first_state,
        request("REQ-usage", "usage.summary", json!({}), None),
    );
    assert!(usage.ok);
    let usage_value = usage.result.as_ref().unwrap();
    assert!(usage_value["command_count"].as_u64().unwrap() >= 4);
    assert!(usage_value["replay_count"].as_u64().unwrap() >= 1);
    assert_eq!(usage_value["remote_calls"], 0);
    assert_eq!(usage_value["model_tokens_in"], 0);
    assert_eq!(usage_value["model_tokens_out"], 0);

    let egress = call(
        &first_state,
        request(
            "REQ-egress",
            "policy.egress.check",
            json!({
                "destination": "remote-ai",
                "project_id": "PRJ-phase2-second",
                "data_classes": ["project"],
                "modalities": ["text"],
                "approx_bytes": 256,
                "approx_tokens": 64,
                "source_refs": [result_id],
                "purpose": "diagnose"
            }),
            None,
        ),
    );
    assert!(egress.ok);
    assert_eq!(egress.result.as_ref().unwrap()["allowed"], false);
    assert_eq!(
        egress.result.as_ref().unwrap()["strongest_class"],
        "project"
    );
    assert!(egress.result.as_ref().unwrap()["ledger_id"]
        .as_str()
        .unwrap()
        .starts_with("EGR-"));


    first.kill().expect("hard kill relayd");
    first.wait().expect("wait hard-killed relayd");
    assert!(dir.join("host.json").exists());

    let mut second = spawn_host(&dir, instance);
    let second_state = wait_state(&dir, second.id());
    assert_eq!(second_state.recovery_state, "Healthy");
    assert_eq!(second_state.storage_schema_version, Some(6));

    let transactions = call(
        &second_state,
        request(
            "REQ-transactions-after-restart",
            "transaction.list",
            json!({
                "project_id": "PRJ-phase2-second",
                "limit": 50
            }),
            None,
        ),
    );
    assert!(transactions.ok);
    let rows = transactions.result.as_ref().unwrap()["transactions"]
        .as_array()
        .unwrap();
    assert_eq!(
        rows.iter()
            .filter(|row| row["command"] == "result.put")
            .count(),
        1
    );

    let usage = call(
        &second_state,
        request(
            "REQ-usage-after-restart",
            "usage.summary",
            json!({}),
            None,
        ),
    );
    assert!(usage.ok);
    assert!(usage.result.as_ref().unwrap()["command_count"]
        .as_u64()
        .unwrap()
        >= 7);

    let result = call(
        &second_state,
        request(
            "REQ-result-after-restart",
            "result.get",
            json!({ "result_id": result_id }),
            None,
        ),
    );
    assert!(result.ok);
    assert_eq!(
        result.result.as_ref().unwrap()["payload"]["exact_number"],
        88
    );

    shutdown(&second_state, &mut second);
    fs::remove_dir_all(dir).unwrap();
}
