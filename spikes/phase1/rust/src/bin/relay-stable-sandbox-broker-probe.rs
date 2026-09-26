use relay_rust_challenger::sandbox_backend::{
    select_backend_for, select_release_backend, WindowsVersion,
};
use relay_rust_challenger::stable_sandbox::{
    broker_egress_connect, run_stable_probe, security_descriptor_sddl,
    BrokerEgressPolicy, StableSandboxPolicy,
};
use serde_json::{json, Value};
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        "relay-spike12-bench-{}-{nanos}",
        std::process::id()
    ))
}

fn identity(label: &str, index: usize) -> String {
    format!(
        "DSIRelay.Spike12.Bench.{label}.{}.{}",
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
    values.push(("RELAY_SANDBOX_MARKER".to_string(), "spike12-bench".to_string()));
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
        eprintln!("usage: relay-stable-sandbox-broker-probe <worker> <iterations>");
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
        std::env::set_var(
            "RELAY_SPIKE11_PARENT_SECRET",
            "synthetic-parent-secret",
        );
    }

    let mut prelaunch_ms = Vec::with_capacity(iterations);
    let mut launch_ms = Vec::with_capacity(iterations);
    let mut total_ms = Vec::with_capacity(iterations);
    let mut last = None;
    let mut temporary_security_restored = true;

    for index in 0..iterations {
        let mailbox = root.join(format!("mailbox-stable-{index:03}"));
        prepare_request(
            &mailbox,
            &blocked,
            &worker_dir,
            "1.1.1.1:443",
        );
        let mailbox_sddl_before =
            security_descriptor_sddl(&mailbox).expect("mailbox descriptor before");
        let worker_sddl_before =
            security_descriptor_sddl(&worker_dir).expect("worker descriptor before");

        let evidence = run_stable_probe(
            &worker,
            &mailbox,
            &StableSandboxPolicy {
                identity: identity("deny", index),
                read_write_paths: vec![mailbox.clone()],
                read_only_paths: vec![worker_dir.clone()],
                capabilities: vec![],
                disallow_win32k: true,
                max_process_memory_bytes: 32 * 1024 * 1024,
                timeout_ms: 3000,
                environment: minimal_environment(),
            },
        )
        .expect("stable sandbox run");

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

        let mailbox_sddl_after =
            security_descriptor_sddl(&mailbox).expect("mailbox descriptor after");
        let worker_sddl_after =
            security_descriptor_sddl(&worker_dir).expect("worker descriptor after");
        temporary_security_restored &= descriptor_restored(
            &mailbox_sddl_before,
            &mailbox_sddl_after,
            false,
        ) && descriptor_restored(
            &worker_sddl_before,
            &worker_sddl_after,
            true,
        );

        prelaunch_ms.push(evidence.prelaunch_ms);
        launch_ms.push(evidence.launch_ms);
        total_ms.push(evidence.total_ms);
        last = Some(evidence);
    }

    unsafe {
        std::env::remove_var("RELAY_SPIKE11_PARENT_SECRET");
    }

    let target: SocketAddr = "1.1.1.1:443".parse().unwrap();
    let host_network_baseline =
        TcpStream::connect_timeout(&target, Duration::from_millis(750)).is_ok();
    let egress = broker_egress_connect(
        "1.1.1.1:443",
        &BrokerEgressPolicy {
            allowed_targets: vec!["1.1.1.1:443".to_string()],
            timeout_ms: 750,
        },
    )
    .expect("allowlisted broker egress");
    let denied_egress = broker_egress_connect(
        "8.8.8.8:443",
        &BrokerEgressPolicy {
            allowed_targets: vec!["1.1.1.1:443".to_string()],
            timeout_ms: 750,
        },
    )
    .expect_err("unlisted egress must fail");

    let direct_capability_mailbox = root.join("mailbox-direct-capability");
    fs::create_dir_all(&direct_capability_mailbox).unwrap();
    let direct_capability = run_stable_probe(
        &worker,
        &direct_capability_mailbox,
        &StableSandboxPolicy {
            identity: identity("direct-capability", iterations + 1),
            read_write_paths: vec![direct_capability_mailbox.clone()],
            read_only_paths: vec![worker_dir.clone()],
            capabilities: vec!["internetClient".to_string()],
            disallow_win32k: true,
            max_process_memory_bytes: 32 * 1024 * 1024,
            timeout_ms: 3000,
            environment: minimal_environment(),
        },
    )
    .expect_err("stable LPAC direct capability must fail");

    let current_selection =
        select_release_backend().expect("current Windows version");
    let simulated_experimental_only = select_backend_for(
        current_selection.windows,
        false,
        true,
    );
    let simulated_unmeasured = select_backend_for(
        WindowsVersion {
            major: 10,
            minor: 0,
            build: 19045,
        },
        true,
        false,
    );
    let simulated_measured_stable_only = select_backend_for(
        current_selection.windows,
        true,
        false,
    );

    let last = last.expect("stable evidence");


    println!(
        "{}",
        serde_json::to_string(&json!({
            "mode": "spike12-stable-bench",
            "iterations": iterations,
            "prelaunch": summary(&prelaunch_ms),
            "launch": summary(&launch_ms),
            "total": summary(&total_ms),
            "deny_result": last.result,
            "direct_write_grant_count": 1,
            "temporary_acl_grant_count": last.temporary_acl_grant_count,
            "temporary_low_il_label_count": last.temporary_low_il_label_count,
            "temporary_security_restored": temporary_security_restored,
            "job_limits": last.job_limits,
            "host_network_baseline": host_network_baseline,
            "brokered_egress": egress,
            "denied_egress_error_code": denied_egress.code,
            "direct_capability_error_code": direct_capability.code,
            "backend_selection": current_selection,
            "simulated_matrix": {
                "experimental_only": simulated_experimental_only,
                "unmeasured_stable": simulated_unmeasured,
                "measured_stable_without_experimental":
                    simulated_measured_stable_only
            }
        }))
        .unwrap()
    );

    fs::remove_dir_all(root).unwrap();
}

fn dacl_part(sddl: &str) -> &str {
    sddl.split_once("S:").map(|(dacl, _)| dacl).unwrap_or(sddl)
}

fn descriptor_restored(
    before: &str,
    after: &str,
    require_exact: bool,
) -> bool {
    if require_exact {
        return before == after;
    }
    dacl_part(before) == dacl_part(after)
        && !after.contains(";;;LW")
        && !after.contains("S-1-16-4096")
}
