#![cfg(windows)]

use relay::client;
use relay_contracts::{CommandRequest, CommandResponse, LocalHostState, RequestContext};
use relay_core::storage::RelayStorage;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn fixture_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("relay-phase3-edges-{}-{nanos}", std::process::id()))
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
        context: RequestContext::default(),
    }
}

fn call(state: &LocalHostState, request: CommandRequest) -> CommandResponse {
    client::call(state, &request).expect("daemon command")
}

fn spawn_host(state_dir: &Path) -> (Child, LocalHostState) {
    let child = Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_STATE_DIR", state_dir)
        .env("RELAY_INSTANCE", "phase3-edges")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn relayd");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(state_dir.join("host.json")) {
            if let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes) {
                if state.pid == child.id() {
                    return (child, state);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("daemon did not become ready");
}

#[test]
fn project_edges_and_deltas_survive_hard_restart_without_scope_leakage() {
    let dir = fixture_dir();
    let root_a = dir.join("alpha");
    let root_b = dir.join("bravo");
    let state_dir = dir.join("state");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();
    fs::create_dir_all(&state_dir).unwrap();
    fs::write(root_a.join("source.txt"), b"source A").unwrap();
    fs::write(root_a.join("target.txt"), b"target A").unwrap();
    fs::write(root_b.join("source.txt"), b"source B").unwrap();
    fs::write(root_b.join("target.txt"), b"target B").unwrap();

    let (mut first, state) = spawn_host(&state_dir);
    assert_eq!(state.storage_schema_version, Some(5));
    for (project_id, root) in [("PRJ-alpha", &root_a), ("PRJ-bravo", &root_b)] {
        let imported = call(
            &state,
            request(
                &format!("REQ-import-{project_id}"),
                "project.import",
                json!({
                    "id": project_id,
                    "name": project_id,
                    "root_path": root.to_string_lossy()
                }),
                Some(&format!("IDEMP-import-{project_id}")),
            ),
        );
        assert!(imported.ok);
        let built = call(
            &state,
            request(
                &format!("REQ-build-{project_id}"),
                "project.index.build",
                json!({ "project_id": project_id }),
                Some(&format!("IDEMP-build-{project_id}")),
            ),
        );
        assert!(built.ok);
    }

    let storage = RelayStorage::open(state_dir.join("relay.sqlite3")).unwrap();
    let source_sha = storage
        .list_project_files("PRJ-alpha")
        .unwrap()
        .into_iter()
        .find(|file| file.relative_path == "source.txt")
        .unwrap()
        .content_sha256;
    drop(storage);

    let replaced = call(
        &state,
        request(
            "REQ-edges-replace",
            "project.dependencies.replace",
            json!({
                "project_id": "PRJ-alpha",
                "expected_generation": 1,
                "source_path": "source.txt",
                "source_sha256": source_sha,
                "producer_id": "fixture.parser",
                "producer_version": "1",
                "targets": ["target.txt"]
            }),
            Some("IDEMP-edges-replace"),
        ),
    );
    assert!(replaced.ok, "{:?}", replaced.error);
    let alpha_edges = call(
        &state,
        request(
            "REQ-alpha-edges",
            "project.dependencies.list",
            json!({ "project_id": "PRJ-alpha" }),
            None,
        ),
    );
    let bravo_edges = call(
        &state,
        request(
            "REQ-bravo-edges",
            "project.dependencies.list",
            json!({ "project_id": "PRJ-bravo" }),
            None,
        ),
    );
    assert_eq!(
        alpha_edges.result.unwrap()["edges"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(bravo_edges.result.unwrap()["edges"]
        .as_array()
        .unwrap()
        .is_empty());

    fs::write(root_a.join("target.txt"), b"target A changed").unwrap();
    let reconciled = call(
        &state,
        request(
            "REQ-alpha-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-alpha" }),
            Some("IDEMP-alpha-reconcile"),
        ),
    );
    assert!(reconciled.ok);
    assert_eq!(reconciled.result.unwrap()["generation"], 2);
    let alpha_edges = call(
        &state,
        request(
            "REQ-alpha-edges-invalidated",
            "project.dependencies.list",
            json!({ "project_id": "PRJ-alpha" }),
            None,
        ),
    );
    assert!(alpha_edges.result.unwrap()["edges"]
        .as_array()
        .unwrap()
        .is_empty());

    let replaced_again = call(
        &state,
        request(
            "REQ-edges-replace-again",
            "project.dependencies.replace",
            json!({
                "project_id": "PRJ-alpha",
                "expected_generation": 2,
                "source_path": "source.txt",
                "source_sha256": source_sha,
                "producer_id": "fixture.parser",
                "producer_version": "1",
                "targets": ["target.txt"]
            }),
            Some("IDEMP-edges-replace-again"),
        ),
    );
    assert!(replaced_again.ok);
    fs::rename(root_a.join("source.txt"), root_a.join("source-renamed.txt")).unwrap();
    fs::remove_file(root_a.join("target.txt")).unwrap();
    let reconciled_again = call(
        &state,
        request(
            "REQ-alpha-reconcile-again",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-alpha" }),
            Some("IDEMP-alpha-reconcile-again"),
        ),
    );
    assert!(reconciled_again.ok);
    assert_eq!(reconciled_again.result.unwrap()["generation"], 3);
    let alpha_edges = call(
        &state,
        request(
            "REQ-alpha-edges-after-rename",
            "project.dependencies.list",
            json!({ "project_id": "PRJ-alpha" }),
            None,
        ),
    );
    assert!(alpha_edges.result.unwrap()["edges"]
        .as_array()
        .unwrap()
        .is_empty());

    first.kill().unwrap();
    first.wait().unwrap();
    let (mut second, state) = spawn_host(&state_dir);
    assert_eq!(state.storage_schema_version, Some(5));
    let alpha_delta = call(
        &state,
        request(
            "REQ-alpha-delta",
            "project.changes",
            json!({ "project_id": "PRJ-alpha", "after_generation": 1 }),
            None,
        ),
    );
    assert!(alpha_delta.ok);
    let changes = alpha_delta.result.unwrap()["changes"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().any(|change| {
        change["change_kind"] == "modified" && change["relative_path"] == "target.txt"
    }));
    assert!(changes.iter().any(|change| {
        change["change_kind"] == "renamed"
            && change["previous_path"] == "source.txt"
            && change["relative_path"] == "source-renamed.txt"
    }));
    assert!(changes.iter().any(|change| {
        change["change_kind"] == "deleted" && change["relative_path"] == "target.txt"
    }));
    let bravo_delta = call(
        &state,
        request(
            "REQ-bravo-delta",
            "project.changes",
            json!({ "project_id": "PRJ-bravo", "after_generation": 1 }),
            None,
        ),
    );
    assert!(bravo_delta.result.unwrap()["changes"]
        .as_array()
        .unwrap()
        .is_empty());

    let stopped = call(
        &state,
        request("REQ-shutdown", "system.shutdown", json!({}), None),
    );
    assert!(stopped.ok);
    assert!(second.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}
