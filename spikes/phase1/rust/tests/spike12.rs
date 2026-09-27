#![cfg(windows)]

use relay_rust_challenger::stable_sandbox::{
    broker_egress_connect, run_stable_probe, security_descriptor_sddl,
    stable_appcontainer_api_available, BrokerEgressPolicy, StableSandboxPolicy,
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
        "relay-spike12-{label}-{}-{suffix}",
        std::process::id()
    ))
}

fn sandbox_identity(label: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("DSIRelay.Spike12.{label}.{}.{}", std::process::id(), suffix)
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
    values.push(("RELAY_SANDBOX_MARKER".to_string(), "spike12".to_string()));
    values
}


#[test]
fn stable_lpac_denies_ungranted_fs_network_env_and_child_process() {
    assert!(
        stable_appcontainer_api_available(),
        "stable AppContainer APIs must be available"
    );

    let root = unique_dir("deny");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    let blocked = root.join("blocked");
    fs::create_dir_all(&mailbox).unwrap();
    fs::create_dir_all(&blocked).unwrap();
    let worker = copy_worker(&worker_dir);
    let mailbox_sddl_before =
        security_descriptor_sddl(&mailbox).expect("mailbox security descriptor");
    let worker_sddl_before =
        security_descriptor_sddl(&worker_dir).expect("worker security descriptor");

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
        std::env::set_var(
            "RELAY_SPIKE11_PARENT_SECRET",
            "synthetic-parent-secret",
        );
    }

    let evidence = run_stable_probe(
        &worker,
        &mailbox,
        &StableSandboxPolicy {
            identity: sandbox_identity("deny"),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec![],
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect("stable LPAC probe");

    unsafe {
        std::env::remove_var("RELAY_SPIKE11_PARENT_SECRET");
    }

    assert!(evidence.result.is_app_container);
    assert!(evidence.low_privilege_appcontainer);
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

    let mailbox_sddl_after =
        security_descriptor_sddl(&mailbox).expect("restored mailbox descriptor");
    let worker_sddl_after =
        security_descriptor_sddl(&worker_dir).expect("restored worker descriptor");

    let mailbox_dacl_before = mailbox_sddl_before
        .split_once("S:")
        .map(|(dacl, _)| dacl)
        .unwrap_or(mailbox_sddl_before.as_str());
    let mailbox_dacl_after = mailbox_sddl_after
        .split_once("S:")
        .map(|(dacl, _)| dacl)
        .unwrap_or(mailbox_sddl_after.as_str());
    assert_eq!(mailbox_dacl_after, mailbox_dacl_before);
    assert!(!mailbox_sddl_after.contains(";;;LW"));
    assert!(!mailbox_sddl_after.contains("S-1-16-4096"));
    assert_eq!(worker_sddl_after, worker_sddl_before);

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn stable_lpac_keeps_direct_network_denied_and_uses_brokered_egress() {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let target: SocketAddr = "1.1.1.1:443".parse().unwrap();
    TcpStream::connect_timeout(&target, Duration::from_millis(750))
        .expect("host baseline must reach synthetic network target");

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

    fs::write(
        mailbox.join("request.json"),
        serde_json::to_vec_pretty(&json!({
            "allowed_input": input,
            "blocked_secret": blocked_secret,
            "blocked_write": blocked.join("escape.txt"),
            "read_only_write": worker_dir.join("should-not-write.txt"),
            "network_target": "1.1.1.1:443"
        }))
        .unwrap(),
    )
    .unwrap();

    let evidence = run_stable_probe(
        &worker,
        &mailbox,
        &StableSandboxPolicy {
            identity: sandbox_identity("network"),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec![],
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect("stable LPAC probe");

    assert!(evidence.result.is_app_container);
    assert!(!evidence.result.network_connect_ok);
    assert!(!evidence.result.blocked_read_ok);
    assert!(!evidence.result.blocked_write_ok);
    assert!(!evidence.result.read_only_write_ok);
    assert!(!evidence.result.child_process_created);

    let egress = broker_egress_connect(
        "1.1.1.1:443",
        &BrokerEgressPolicy {
            allowed_targets: vec!["1.1.1.1:443".to_string()],
            timeout_ms: 750,
        },
    )
    .expect("allowlisted broker egress");
    assert!(egress.allowed_by_policy);
    assert!(egress.connect_ok);

    let denied = broker_egress_connect(
        "8.8.8.8:443",
        &BrokerEgressPolicy {
            allowed_targets: vec!["1.1.1.1:443".to_string()],
            timeout_ms: 750,
        },
    )
    .expect_err("unlisted target must fail closed");
    assert_eq!(denied.code, "STABLE_SANDBOX_EGRESS_DENIED");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stable_lpac_rejects_direct_capability_grants_before_launch() {
    let root = unique_dir("capability");
    let worker_dir = root.join("worker");
    let mailbox = root.join("mailbox");
    fs::create_dir_all(&mailbox).unwrap();
    let worker = copy_worker(&worker_dir);

    let error = run_stable_probe(
        &worker,
        &mailbox,
        &StableSandboxPolicy {
            identity: sandbox_identity("capability"),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_dir],
            capabilities: vec!["internetClient".to_string()],
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("direct LPAC capability grant must fail in RELAY");

    assert_eq!(
        error.code,
        "STABLE_SANDBOX_DIRECT_CAPABILITY_UNSUPPORTED"
    );
    assert!(!mailbox.join("response.json").exists());
    fs::remove_dir_all(root).unwrap();
}
