#![cfg(windows)]

use relay_rust_challenger::pipe;
use relay_rust_challenger::state::{read_state, HostState};
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
        "relay-rust-spike14-{label}-{}-{nanos}",
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
fn live_host_logs_structure_without_command_arguments() {
    let dir = unique_dir("privacy");
    fs::create_dir_all(&dir).unwrap();
    let mut child = spawn_host(&dir, "spike14-privacy");
    let state = wait_state(&dir, child.id());

    let status = call(&state, "system.status", json!({}));
    assert_eq!(status["ok"], true);
    assert_eq!(status["result"]["diagnostics"]["ok"], true);
    assert_eq!(status["result"]["recovery_state"], "Healthy");

    let secret = "SHOULD-NOT-APPEAR-IN-DIAGNOSTICS";
    let echo = call(&state, "system.echo", json!({ "value": secret }));
    assert_eq!(echo["ok"], true);

    let doctor = call(&state, "system.doctor", json!({}));
    assert_eq!(doctor["result"]["healthy"], true);
    assert!(doctor["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| {
            check["id"] == "diagnostics.capture" && check["status"] == "pass"
        }));

    shutdown(&state, &mut child);

    let diagnostics_dir = dir.join("diagnostics");
    let mut raw = String::new();
    for entry in fs::read_dir(&diagnostics_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jsonl") {
            raw.push_str(&fs::read_to_string(entry.path()).unwrap());
        }
    }
    assert!(raw.contains("relay.host.started"));
    assert!(raw.contains("relay.command.completed"));
    assert!(!raw.contains(secret));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn diagnostics_open_failure_degrades_health_without_breaking_storage() {
    let dir = unique_dir("degraded");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("diagnostics"), b"blocks directory creation").unwrap();

    let mut child = spawn_host(&dir, "spike14-degraded");
    let state = wait_state(&dir, child.id());
    assert_eq!(state.recovery_state, "Degraded");

    let status = call(&state, "system.status", json!({}));
    assert_eq!(status["result"]["storage"]["ok"], true);
    assert_eq!(status["result"]["diagnostics"]["ok"], false);
    assert_eq!(status["result"]["recovery_state"], "Degraded");

    let projects = call(&state, "project.list", json!({}));
    assert_eq!(projects["ok"], true);

    let doctor = call(&state, "system.doctor", json!({}));
    assert_eq!(doctor["result"]["healthy"], false);
    assert!(doctor["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| {
            check["id"] == "diagnostics.capture" && check["status"] == "fail"
        }));

    shutdown(&state, &mut child);
    fs::remove_dir_all(dir).unwrap();
}
