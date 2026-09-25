#![cfg(windows)]

use relay_rust_challenger::pipe;
use relay_rust_challenger::state::{read_state, HostState};
use serde_json::json;
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
    std::env::temp_dir().join(format!("relay-rust-{label}-{}-{nanos}", std::process::id()))
}

fn spawn_host(dir: &Path, instance: &str, dashboard: bool) -> Child {
    let exe = env!("CARGO_BIN_EXE_relay-rust-challenger");
    let mut command = Command::new(exe);
    command.arg("host");
    if dashboard { command.arg("--dashboard"); }
    command
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

fn shutdown(state: &HostState, child: &mut Child) {
    let response = pipe::call(
        &state.pipe,
        &state.auth_token,
        "system.shutdown",
        json!({}),
        3000,
    ).expect("shutdown call");
    assert_eq!(response["ok"], true);
    let status = child.wait().expect("wait host");
    assert!(status.success());
}

#[test]
fn live_round_trip_and_fail_closed_auth() {
    let dir = unique_dir("live");
    fs::create_dir_all(&dir).unwrap();
    let mut child = spawn_host(&dir, "spike7-live-test", false);
    let state = wait_state(&dir, child.id());

    let status = pipe::call(
        &state.pipe,
        &state.auth_token,
        "system.status",
        json!({}),
        3000,
    ).expect("status");
    assert_eq!(status["ok"], true);
    assert_eq!(status["result"]["runtime"], "rust");
    assert_eq!(status["result"]["ipc_security"]["explicit_dacl"], true);

    let denied = pipe::call(
        &state.pipe,
        &"00".repeat(32),
        "system.status",
        json!({}),
        3000,
    ).expect_err("wrong token must fail");
    assert_eq!(denied, "UNAUTHORIZED");

    shutdown(&state, &mut child);
    assert!(!dir.join("host-rust.json").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn hard_kill_restart_replaces_stale_state() {
    let dir = unique_dir("restart");
    fs::create_dir_all(&dir).unwrap();
    let instance = "spike7-restart-test";
    let mut first = spawn_host(&dir, instance, false);
    let first_state = wait_state(&dir, first.id());

    let before = pipe::call(
        &first_state.pipe,
        &first_state.auth_token,
        "system.status",
        json!({}),
        3000,
    ).expect("pre-kill status");
    assert_eq!(before["ok"], true);

    first.kill().expect("hard kill");
    first.wait().expect("wait killed host");
    assert!(dir.join("host-rust.json").exists());

    let mut second = spawn_host(&dir, instance, false);
    let second_state = wait_state(&dir, second.id());
    assert_ne!(second_state.pid, first_state.pid);

    let after = pipe::call(
        &second_state.pipe,
        &second_state.auth_token,
        "system.status",
        json!({}),
        3000,
    ).expect("post-restart status");
    assert_eq!(after["ok"], true);

    shutdown(&second_state, &mut second);
    assert!(!dir.join("host-rust.json").exists());
    fs::remove_dir_all(&dir).unwrap();
}
