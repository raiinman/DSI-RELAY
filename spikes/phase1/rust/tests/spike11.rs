#![cfg(windows)]

use relay_rust_challenger::sandbox::{
    run_probe, sandbox_api_available, SandboxPolicy, SANDBOX_SPEC_VERSION,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_dir(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-spike11-{label}-{}-{suffix}",
        std::process::id()
    ))
}

fn sandbox_identity(label: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("DSIRelay.Spike11.{label}.{}.{}", std::process::id(), suffix)
}

fn copy_worker(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let source = PathBuf::from(env!("CARGO_BIN_EXE_relay-sandbox-probe"));
    let target = dir.join("relay-sandbox-probe.exe");
    fs::copy(source, &target).unwrap();
    target
}

fn minimal_environment() -> Vec<(String, String)> {
    let keys = [
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "ComSpec",
        "Path",
        "PATHEXT",
        "TEMP",
        "TMP",
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "PROCESSOR_ARCHITECTURE",
        "NUMBER_OF_PROCESSORS",
        "OS",
        "USERNAME",
        "USERDOMAIN",
    ];
    let mut values: Vec<(String, String)> = keys
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_string(), value))
        })
        .collect();
    values.push((
        "RELAY_SANDBOX_MARKER".to_string(),
        "spike11".to_string(),
    ));
    values
}


#[test]
fn experimental_sandbox_denies_ungranted_fs_network_env_and_child_process() {
    assert!(sandbox_api_available(), "processmodel sandbox API must be available");

    let root = unique_dir("deny");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    let blocked = root.join("blocked");
    fs::create_dir_all(&mailbox).unwrap();
    fs::create_dir_all(&blocked).unwrap();
    let worker = copy_worker(&worker_dir);

    let input = mailbox.join("input.txt");
    fs::write(&input, b"allowed").unwrap();
    let blocked_secret = blocked.join("secret.txt");
    fs::write(&blocked_secret, b"blocked-secret").unwrap();
    let blocked_write = blocked.join("escape.txt");

    fs::write(
        mailbox.join("request.json"),
        serde_json::to_vec_pretty(&json!({
            "allowed_input": input,
            "blocked_secret": blocked_secret,
            "blocked_write": blocked_write,
            "read_only_write": worker_dir.join("should-not-write.txt"),
            "network_target": "1.1.1.1:443"
        }))
        .unwrap(),
    )
    .unwrap();

    unsafe {
        std::env::set_var("RELAY_SPIKE11_PARENT_SECRET", "synthetic-parent-secret");
    }

    let evidence = run_probe(
        &worker,
        &mailbox,
        &SandboxPolicy {
            identity: sandbox_identity("deny"),
            spec_version: SANDBOX_SPEC_VERSION.to_string(),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec![],
            network_default_allow: false,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect("sandboxed probe");

    unsafe {
        std::env::remove_var("RELAY_SPIKE11_PARENT_SECRET");
    }

    assert!(evidence.result.is_app_container);
    assert!(evidence.result.allowed_read_ok);
    assert!(evidence.result.allowed_write_ok);
    assert!(!evidence.result.read_only_write_ok);
    assert!(!evidence.result.blocked_read_ok);
    assert!(!evidence.result.blocked_write_ok);
    assert!(!evidence.result.network_connect_ok);
    assert!(!evidence.result.parent_secret_visible);
    assert!(!evidence.result.user_profile_visible);
    assert!(!evidence.result.child_process_created);
    assert_eq!(evidence.job_limits.active_process_limit, 1);
    assert!(evidence.job_limits.kill_on_close);
    assert_eq!(
        evidence.job_limits.process_memory_limit_bytes,
        32 * 1024 * 1024
    );

    assert!(!blocked_write.exists());
    assert_eq!(
        fs::read_to_string(mailbox.join("worker-write.txt")).unwrap(),
        "worker-write"
    );

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn sandbox_network_is_enabled_only_when_capability_is_granted() {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let target: SocketAddr = "1.1.1.1:443".parse().unwrap();
    TcpStream::connect_timeout(&target, Duration::from_millis(750))
        .expect("host baseline must reach the synthetic network target");

    let root = unique_dir("network");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    let blocked = root.join("blocked");
    fs::create_dir_all(&mailbox).unwrap();
    fs::create_dir_all(&blocked).unwrap();
    let worker = copy_worker(&worker_dir);

    let input = mailbox.join("input.txt");
    fs::write(&input, b"allowed").unwrap();
    let blocked_secret = blocked.join("secret.txt");
    fs::write(&blocked_secret, b"blocked-secret").unwrap();
    let blocked_write = blocked.join("escape.txt");

    fs::write(
        mailbox.join("request.json"),
        serde_json::to_vec_pretty(&json!({
            "allowed_input": input,
            "blocked_secret": blocked_secret,
            "blocked_write": blocked_write,
            "read_only_write": worker_dir.join("should-not-write.txt"),
            "network_target": "1.1.1.1:443"
        }))
        .unwrap(),
    )
    .unwrap();

    let evidence = run_probe(
        &worker,
        &mailbox,
        &SandboxPolicy {
            identity: sandbox_identity("network"),
            spec_version: SANDBOX_SPEC_VERSION.to_string(),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec!["internetClient".to_string()],
            network_default_allow: true,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect("network-granted sandbox probe");

    assert!(evidence.result.is_app_container);
    assert!(
        evidence.result.network_connect_ok,
        "internetClient capability did not permit TCP; capability_count={} error={:?}",
        evidence.result.capability_count,
        evidence.result.network_error_code
    );
    assert!(!evidence.result.read_only_write_ok);
    assert!(!evidence.result.blocked_read_ok);
    assert!(!evidence.result.blocked_write_ok);
    assert!(!evidence.result.child_process_created);

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn sandbox_policy_failure_never_falls_back_unrestricted() {
    let root = unique_dir("invalid-capability");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    let blocked = root.join("blocked");
    fs::create_dir_all(&mailbox).unwrap();
    fs::create_dir_all(&blocked).unwrap();
    let worker = copy_worker(&worker_dir);

    let input = mailbox.join("input.txt");
    fs::write(&input, b"allowed").unwrap();
    let blocked_secret = blocked.join("secret.txt");
    fs::write(&blocked_secret, b"blocked-secret").unwrap();
    let blocked_write = blocked.join("escape.txt");

    fs::write(
        mailbox.join("request.json"),
        serde_json::to_vec_pretty(&json!({
            "allowed_input": input,
            "blocked_secret": blocked_secret,
            "blocked_write": blocked_write,
            "read_only_write": worker_dir.join("should-not-write.txt"),
            "network_target": "1.1.1.1:443"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = run_probe(
        &worker,
        &mailbox,
        &SandboxPolicy {
            identity: sandbox_identity("invalid-spec"),
            spec_version: "9.9.9".to_string(),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir],
            capabilities: vec![],
            network_default_allow: false,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("unsupported sandbox spec version must fail closed");

    assert_eq!(error.code, "SANDBOX_SPEC_INCOMPATIBLE");
    assert!(!mailbox.join("response.json").exists());
    assert!(!mailbox.join("worker-write.txt").exists());
    assert!(!blocked_write.exists());

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn unsupported_capability_is_rejected_by_relay_before_launch() {
    let root = unique_dir("unknown-capability");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    fs::create_dir_all(&mailbox).unwrap();
    let worker = copy_worker(&worker_dir);

    let error = run_probe(
        &worker,
        &mailbox,
        &SandboxPolicy {
            identity: sandbox_identity("unknown-capability"),
            spec_version: SANDBOX_SPEC_VERSION.to_string(),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir],
            capabilities: vec!["relayCapabilityThatDoesNotExist".to_string()],
            network_default_allow: false,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("unknown sandbox capability must fail in RELAY policy validation");

    assert_eq!(error.code, "SANDBOX_CAPABILITY_UNSUPPORTED");
    assert!(!mailbox.join("response.json").exists());
    fs::remove_dir_all(root).unwrap();
}
