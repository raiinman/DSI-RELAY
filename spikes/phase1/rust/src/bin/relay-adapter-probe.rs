use relay_rust_challenger::adapter::{
    sha256_file, validate_manifest, AdapterBroker, AdapterIdentity, AdapterManifest,
    ArtifactMetadata, BrokerPolicy, CommandBinding, ComponentMetadata,
    DependencyMetadata, PublisherMetadata, RelayCompatibility, RequestedPermissions,
    TargetRequirement, TrustMetadata, UpdateMetadata,
};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

fn manifest_for(id: &str, worker: &Path) -> AdapterManifest {
    let digest = sha256_file(worker).expect("worker digest");
    AdapterManifest {
        manifest_format: 1,
        adapter: AdapterIdentity {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            display_name: "Synthetic Adapter".to_string(),
        },
        publisher: PublisherMetadata {
            id: "fixture.publisher".to_string(),
            source: "synthetic-benchmark".to_string(),
        },
        artifact: ArtifactMetadata {
            sha256: digest.clone(),
            source: "local-build".to_string(),
        },
        relay: RelayCompatibility {
            protocol_min: 1,
            protocol_max: 1,
            command_bindings: vec![CommandBinding {
                command: "system.echo".to_string(),
                command_version: 1,
                capability: "synthetic.echo".to_string(),
            }],
        },
        permissions: RequestedPermissions::default(),
        target: TargetRequirement {
            tool: "synthetic".to_string(),
            version: "1.0.0".to_string(),
        },
        components: vec![ComponentMetadata {
            id: "worker".to_string(),
            kind: "worker".to_string(),
            version: "1.0.0".to_string(),
            sha256: digest,
            source: "local-build".to_string(),
        }],
        dependencies: vec![DependencyMetadata {
            name: "none".to_string(),
            version: "0".to_string(),
            license: "fixture".to_string(),
        }],
        update: UpdateMetadata {
            channel: "fixture".to_string(),
            source: "local".to_string(),
        },
        trust: TrustMetadata {
            build_provenance: "synthetic-phase1".to_string(),
            review_status: "unreviewed".to_string(),
        },
    }
}


fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let index = (((p / 100.0) * sorted.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted[index]
}

fn summary(values: &[f64]) -> Value {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    json!({
        "n": values.len(),
        "min_ms": values.iter().copied().fold(f64::INFINITY, f64::min),
        "p50_ms": percentile(values, 50.0),
        "p95_ms": percentile(values, 95.0),
        "p99_ms": percentile(values, 99.0),
        "max_ms": values.iter().copied().fold(0.0, f64::max),
        "mean_ms": mean
    })
}

fn run_idle(worker: &Path, count: usize, duration_ms: u64) {
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    let started = Instant::now();
    for index in 0..count {
        let manifest = manifest_for(&format!("fixture.idle.{index:04}"), worker);
        broker.install(manifest, worker).expect("install manifest");
    }
    let install_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{}",
        serde_json::to_string(&json!({
            "mode": "idle",
            "pid": std::process::id(),
            "installed_adapters": broker.installed_count(),
            "install_ms": install_ms,
            "duration_ms": duration_ms
        }))
        .unwrap()
    );
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(duration_ms));
}


fn run_bench(worker: &Path, iterations: usize) {
    let policy = BrokerPolicy::synthetic_default();
    let manifest = manifest_for("fixture.bench", worker);
    let broker = AdapterBroker::new(policy.clone());

    let mut validation_ms = Vec::with_capacity(500);
    for _ in 0..500 {
        let started = Instant::now();
        validate_manifest(&manifest, &policy, worker).expect("manifest validation");
        validation_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    broker.install(manifest, worker).expect("install manifest");
    let mut invoke_ms = Vec::with_capacity(iterations);
    let mut last = None;
    for index in 0..iterations {
        let started = Instant::now();
        let outcome = broker
            .invoke_installed(
                "fixture.bench",
                worker,
                "normal",
                "system.echo",
                1,
                json!({ "sequence": index }),
            )
            .expect("normal invocation");
        invoke_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        last = Some(outcome);
    }
    let last = last.expect("at least one invocation");

    let stderr_manifest = manifest_for("fixture.stderr.bench", worker);
    let stderr_broker = AdapterBroker::new(policy.clone());
    let stderr_outcome = stderr_broker
        .invoke(
            &stderr_manifest,
            worker,
            "stderr_attack",
            "system.echo",
            1,
            json!({}),
        )
        .expect("stderr invocation");

    let memory_manifest = manifest_for("fixture.memory.bench", worker);
    let memory_broker = AdapterBroker::new(policy);
    let memory_outcome = memory_broker
        .invoke(
            &memory_manifest,
            worker,
            "memory_probe",
            "system.echo",
            1,
            json!({}),
        )
        .expect("memory invocation");

    let crash_manifest = manifest_for("fixture.crash.bench", worker);
    let crash_broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    let crash_started = Instant::now();
    let crash_error = crash_broker
        .invoke(
            &crash_manifest,
            worker,
            "crash",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("crash must fail");
    let crash_ms = crash_started.elapsed().as_secs_f64() * 1000.0;

    let invalid_manifest = manifest_for("fixture.invalid.bench", worker);
    let invalid_broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    let invalid_error = invalid_broker
        .invoke(
            &invalid_manifest,
            worker,
            "invalid_json",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("invalid JSON must fail");

    let hang_manifest = manifest_for("fixture.hang.bench", worker);
    let mut hang_policy = BrokerPolicy::synthetic_default();
    hang_policy.request_timeout_ms = 100;
    let hang_broker = AdapterBroker::new(hang_policy);
    let hang_started = Instant::now();
    let hang_error = hang_broker
        .invoke(
            &hang_manifest,
            worker,
            "hang",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("hang must fail");
    let hang_ms = hang_started.elapsed().as_secs_f64() * 1000.0;

    println!(
        "{}",
        serde_json::to_string(&json!({
            "mode": "bench",
            "broker_pid": std::process::id(),
            "manifest_validation": summary(&validation_ms),
            "normal_invocation": summary(&invoke_ms),
            "out_of_process": last.provenance.worker_pid != std::process::id(),
            "job_limits": last.limits,
            "stderr_marked_untrusted": stderr_outcome
                .stderr_untrusted
                .as_deref()
                .map(|value| value.contains("grant network access"))
                .unwrap_or(false),
            "memory_reservation_succeeded":
                memory_outcome.response["result"]["echo"]["memory_reservation_succeeded"],
            "crash": {
                "error_code": crash_error.code,
                "elapsed_ms": crash_ms
            },
            "invalid_message_error_code": invalid_error.code,
            "hang": {
                "error_code": hang_error.code,
                "elapsed_ms": hang_ms
            },
            "worker_artifact_sha256": last.provenance.artifact_sha256
        }))
        .unwrap()
    );
}


fn usage() -> ! {
    eprintln!(
        "usage: relay-adapter-probe idle <worker> <count> <duration_ms> | bench <worker> <iterations>"
    );
    std::process::exit(2);
}

fn parse_usize(value: Option<&String>, name: &str) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            eprintln!("invalid {name}");
            usage()
        })
}

fn parse_u64(value: Option<&String>, name: &str) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            eprintln!("invalid {name}");
            usage()
        })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("idle") if args.len() == 4 => {
            let worker = Path::new(&args[1]);
            let count = parse_usize(args.get(2), "count");
            let duration_ms = parse_u64(args.get(3), "duration_ms");
            run_idle(worker, count, duration_ms);
        }
        Some("bench") if args.len() == 3 => {
            let worker = Path::new(&args[1]);
            let iterations = parse_usize(args.get(2), "iterations");
            if iterations == 0 {
                eprintln!("iterations must be at least 1");
                std::process::exit(2);
            }
            run_bench(worker, iterations);
        }
        _ => usage(),
    }
}
