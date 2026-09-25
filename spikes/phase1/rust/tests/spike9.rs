#![cfg(windows)]

use relay_rust_challenger::pipe;
use relay_rust_challenger::registry;
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
        "relay-rust-spike9-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn spawn_host(dir: &Path, dashboard: bool) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_relay-rust-challenger"));
    command.arg("host");
    if dashboard {
        command.arg("--dashboard");
    }
    command
        .env("RELAY_STATE_DIR", dir)
        .env("RELAY_INSTANCE", "spike9-registry-test")
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
fn live_registry_discovery_and_command_versions_work() {
    let dir = unique_dir("live");
    fs::create_dir_all(&dir).unwrap();
    let mut child = spawn_host(&dir, true);
    let state = wait_state(&dir, child.id());

    let compact = call(
        &state,
        "registry.list",
        json!({ "surface": "ai" }),
    );
    assert_eq!(compact["ok"], true);
    assert_eq!(compact["command_version"], 1);
    let commands = compact["result"]["commands"].as_array().unwrap();
    assert!(commands.iter().any(|value| value["id"] == "system.status"));
    assert!(commands.iter().any(|value| value["id"] == "registry.describe"));

    let described = call(
        &state,
        "registry.describe",
        json!({ "command": "project.register", "version": 1 }),
    );
    assert_eq!(described["ok"], true);
    assert_eq!(
        described["result"]["command"]["arguments_schema"]["required"][0],
        "name"
    );

    let old_style = call(&state, "system.status", json!({}));
    assert_eq!(old_style["ok"], true);
    assert_eq!(old_style["command_version"], 1);

    let incompatible = pipe::call_versioned(
        &state.pipe,
        &state.auth_token,
        "system.status",
        Some(999),
        json!({}),
        3000,
    )
    .expect("versioned call");
    assert_eq!(incompatible["ok"], false);
    assert_eq!(
        incompatible["error"]["code"],
        "COMMAND_VERSION_INCOMPATIBLE"
    );


    let invalid = call(
        &state,
        "project.register",
        json!({ "root_uri": "file:///missing-name" }),
    );
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["error"]["code"], "VALIDATION_FAILED");

    let dashboard_commands = state
        .dashboard
        .as_ref()
        .expect("dashboard state")
        .commands
        .clone();
    assert_eq!(
        dashboard_commands,
        registry::surface_command_ids("dashboard")
    );
    assert!(!dashboard_commands.contains(&"system.shutdown".to_string()));

    shutdown(&state, &mut child);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn cli_adapter_and_ai_metadata_are_derived_from_registry() {
    let cli = registry::surface_command_ids("cli");
    let adapter = registry::compact_list(Some("adapter"), None, 200);
    let ai = registry::compact_list(Some("ai"), None, 200);

    assert!(cli.contains(&"system.shutdown".to_string()));
    assert!(!registry::is_surface_exposed("system.shutdown", "dashboard"));
    assert!(adapter["commands"].as_array().unwrap().len() > 5);
    assert!(ai["commands"].as_array().unwrap().len() > 5);

    let help = registry::render_cli_catalog();
    assert!(help.contains("system.status@1"));
    assert!(help.contains("registry.describe@1"));
}
