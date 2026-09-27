#![cfg(windows)]

use relay::client;
use relay_contracts::{CommandRequest, LocalHostState, RequestContext};
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Host {
    child: Child,
    dir: PathBuf,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn request(command: &str) -> CommandRequest {
    CommandRequest {
        request_id: format!("REQ-dashboard-{command}"),
        command: command.to_string(),
        command_version: Some(1),
        arguments: json!({}),
        idempotency_key: None,
        context: RequestContext::default(),
    }
}

fn http(port: u16, token: &str, command: &str) -> String {
    let body = json!({ "command": command, "arguments": {} }).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "POST /api/execute HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nX-Relay-Dashboard-Token: {token}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    reply
}

#[test]
fn daemon_dashboard_shares_read_only_commands_with_pipe_clients() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "relay-phase5-dashboard-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_STATE_DIR", &dir)
        .env("RELAY_INSTANCE", format!("dashboard-{nanos}"))
        .env("RELAY_TEST_DISABLE_WATCHER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut host = Host { child, dir };
    let deadline = Instant::now() + Duration::from_secs(5);
    let state: LocalHostState = loop {
        if let Ok(bytes) = fs::read(host.dir.join("host.json")) {
            if let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes) {
                if state.pid == host.child.id() && host.dir.join("dashboard.json").exists() {
                    break state;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "dashboard state did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let info: serde_json::Value =
        serde_json::from_slice(&fs::read(host.dir.join("dashboard.json")).unwrap()).unwrap();
    let url = info["url"].as_str().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
    let (authority, token) = url.split_once("/#").unwrap();
    let port: u16 = authority.rsplit(':').next().unwrap().parse().unwrap();
    assert_eq!(token.len(), 64);
    let via_cli = client::call(&state, &request("system.status")).unwrap();
    assert!(via_cli.ok);
    let via_dashboard = http(port, token, "system.status");
    assert!(via_dashboard.contains("\"recovery_state\""));
    assert!(http(port, token, "system.shutdown").contains("DASHBOARD_COMMAND_NOT_EXPOSED"));
    assert!(
        client::call(&state, &request("system.shutdown"))
            .unwrap()
            .ok
    );
    assert!(host.child.wait().unwrap().success());
    assert!(!host.dir.join("dashboard.json").exists());
}
