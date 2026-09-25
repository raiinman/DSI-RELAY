use relay_rust_challenger::sandbox::{
    run_probe, sandbox_api_available, SandboxPolicy, SANDBOX_SPEC_VERSION,
};
use serde_json::{json, Value};
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let index = (((p / 100.0) * sorted.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted[index]
}

fn summary(values: &[f64]) -> Value {
    json!({
        "n": values.len(),
        "min_ms": values.iter().copied().fold(f64::INFINITY, f64::min),
        "p50_ms": percentile(values, 50.0),
        "p95_ms": percentile(values, 95.0),
        "p99_ms": percentile(values, 99.0),
        "max_ms": values.iter().copied().fold(0.0, f64::max),
        "mean_ms": values.iter().sum::<f64>() / values.len() as f64
    })
}

fn unique_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-spike11-bench-{}-{nanos}",
        std::process::id()
    ))
}


fn identity(label: &str, index: usize) -> String {
    format!(
        "DSIRelay.Spike11.Bench.{label}.{}.{}",
        std::process::id(),
        index
    )
}

fn minimal_environment() -> Vec<(String, String)> {
    let keys = [
        "SystemRoot", "WINDIR", "SystemDrive", "ComSpec", "Path", "PATHEXT",
        "TEMP", "TMP", "LOCALAPPDATA", "APPDATA", "ProgramData", "ProgramFiles",
        "ProgramFiles(x86)", "CommonProgramFiles", "CommonProgramFiles(x86)",
        "PROCESSOR_ARCHITECTURE", "NUMBER_OF_PROCESSORS", "OS", "USERNAME",
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
    values.push(("RELAY_SANDBOX_MARKER".to_string(), "spike11-bench".to_string()));
    values
}

fn prepare_request(
    mailbox: &Path,
    blocked: &Path,
    worker_dir: &Path,
    target: &str,
) {
    fs::create_dir_all(mailbox).unwrap();
    fs::create_dir_all(blocked).unwrap();
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
            "network_target": target
        }))
        .unwrap(),
    )
    .unwrap();
}


fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: relay-sandbox-broker-probe <worker> <iterations>");
        std::process::exit(2);
    }
    let source_worker = PathBuf::from(&args[0]);
    let iterations: usize = args[1].parse().expect("iterations");
    assert!(iterations > 0);

    let root = unique_root();
    let worker_dir = root.join("worker");
    let blocked = root.join("blocked");
    fs::create_dir_all(&worker_dir).unwrap();
    fs::create_dir_all(&blocked).unwrap();
    let worker = worker_dir.join("relay-sandbox-probe.exe");
    fs::copy(&source_worker, &worker).unwrap();

    unsafe {
        std::env::set_var("RELAY_SPIKE11_PARENT_SECRET", "synthetic-parent-secret");
    }

    let mut launch_ms = Vec::with_capacity(iterations);
    let mut total_ms = Vec::with_capacity(iterations);
    let mut last = None;

    for index in 0..iterations {
        let mailbox = root.join(format!("mailbox-deny-{index:03}"));
        prepare_request(&mailbox, &blocked, &worker_dir, "1.1.1.1:443");

        let evidence = run_probe(
            &worker,
            &mailbox,
            &SandboxPolicy {
                identity: identity("deny", index),
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
        .expect("denied sandbox run");

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

        launch_ms.push(evidence.launch_ms);
        total_ms.push(evidence.total_ms);
        last = Some(evidence);
    }


    let network_target: SocketAddr = "1.1.1.1:443".parse().unwrap();
    let host_network_baseline =
        TcpStream::connect_timeout(&network_target, Duration::from_millis(750)).is_ok();

    let network_mailbox = root.join("mailbox-network");
    prepare_request(&network_mailbox, &blocked, &worker_dir, "1.1.1.1:443");
    let network_started = Instant::now();
    let network = run_probe(
        &worker,
        &network_mailbox,
        &SandboxPolicy {
            identity: identity("network", iterations + 1),
            spec_version: SANDBOX_SPEC_VERSION.to_string(),
            read_write_paths: vec![network_mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec!["internetClient".to_string()],
            network_default_allow: true,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect("network sandbox run");
    let network_total_ms = network_started.elapsed().as_secs_f64() * 1000.0;

    let invalid_spec = run_probe(
        &worker,
        &root.join("invalid-spec-mailbox"),
        &SandboxPolicy {
            identity: identity("invalid-spec", iterations + 2),
            spec_version: "9.9.9".to_string(),
            read_write_paths: vec![],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec![],
            network_default_allow: false,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("invalid spec must fail");

    let unknown_capability = run_probe(
        &worker,
        &root.join("unknown-capability-mailbox"),
        &SandboxPolicy {
            identity: identity("unknown-capability", iterations + 3),
            spec_version: SANDBOX_SPEC_VERSION.to_string(),
            read_write_paths: vec![],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec!["relayCapabilityThatDoesNotExist".to_string()],
            network_default_allow: false,
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("unknown capability must fail");

    unsafe {
        std::env::remove_var("RELAY_SPIKE11_PARENT_SECRET");
    }

    let last = last.expect("deny evidence");


    println!(
        "{}",
        serde_json::to_string(&json!({
            "mode": "spike11-bench",
            "api_available": sandbox_api_available(),
            "iterations": iterations,
            "deny_launch": summary(&launch_ms),
            "deny_total": summary(&total_ms),
            "deny_result": last.result,
            "job_limits": last.job_limits,
            "host_network_baseline": host_network_baseline,
            "network_grant": {
                "total_ms": network_total_ms,
                "result": network.result,
                "job_limits": network.job_limits
            },
            "invalid_spec_error_code": invalid_spec.code,
            "unknown_capability_error_code": unknown_capability.code
        }))
        .unwrap()
    );

    fs::remove_dir_all(root).unwrap();
}
