use relay_adapter::{
    sha256_file, AdapterBroker, AdapterIdentity, AdapterManifest,
    ArtifactMetadata, BrokerPolicy, CommandBinding, ComponentMetadata,
    DependencyMetadata, PublisherMetadata, RelayCompatibility,
    RequestedPermissions, TargetRequirement, TrustMetadata, UpdateMetadata,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase2-adapter-resource-{label}-{}-{suffix}",
        std::process::id()
    ))
}

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
            source: "phase2-resource".to_string(),
        },
        artifact: ArtifactMetadata {
            sha256: digest.clone(),
            source: "local-release-build".to_string(),
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
            source: "local-release-build".to_string(),
        }],
        dependencies: vec![DependencyMetadata {
            name: "fixture".to_string(),
            version: "1".to_string(),
            license: "MIT".to_string(),
        }],
        update: UpdateMetadata {
            channel: "fixture".to_string(),
            source: "local".to_string(),
        },
        trust: TrustMetadata {
            build_provenance: "phase2-resource".to_string(),
            review_status: "test-fixture".to_string(),
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
fn summarize(values: &[f64]) -> serde_json::Value {
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
    let root = temp_root("idle");
    let mailbox_root = root.join("mailboxes");
    let broker =
        AdapterBroker::new(BrokerPolicy::synthetic_default(), &mailbox_root)
            .expect("broker");

    let started = Instant::now();
    for index in 0..count {
        let id = format!("fixture.resource.{index:04}");
        broker.install(manifest_for(&id, worker), worker).expect("install");
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
    use std::io::Write as _;
    std::io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(duration_ms));
    drop(broker);
    let _ = std::fs::remove_dir_all(root);
}

fn run_invoke(worker: &Path, iterations: usize) {
    let root = temp_root("invoke");
    let mailbox_root = root.join("mailboxes");
    let worker_dir = root.join("worker");
    std::fs::create_dir_all(&worker_dir).expect("worker dir");
    let worker_copy = worker_dir.join("relay-adapter-fixture.exe");
    std::fs::copy(worker, &worker_copy).expect("copy worker");
    let broker =
        AdapterBroker::new(BrokerPolicy::synthetic_default(), &mailbox_root)
            .expect("broker");
    broker
        .install(
            manifest_for("fixture.resource.invoke", &worker_copy),
            &worker_copy,
        )
        .expect("install");
    let mut timings = Vec::with_capacity(iterations);
    let mut last = None;
    for index in 0..iterations {
        let started = Instant::now();
        let outcome = broker
            .invoke(
                "fixture.resource.invoke",
                "system.echo",
                1,
                json!({ "sequence": index }),
            )
            .expect("invoke");
        timings.push(started.elapsed().as_secs_f64() * 1000.0);
        last = Some(outcome);
    }
    let last = last.expect("at least one invocation");
    println!(
        "{}",
        serde_json::to_string(&json!({
            "mode": "invoke",
            "pid": std::process::id(),
            "iterations": iterations,
            "latency": summarize(&timings),
            "sandbox_backend": last.sandbox_backend,
            "sandbox_total_ms": last.sandbox_total_ms,
            "job_limits": last.job_limits
        }))
        .unwrap()
    );

    drop(broker);
    let _ = std::fs::remove_dir_all(root);
}

fn usage() -> ! {
    eprintln!(
        "usage: phase2_resource_probe idle <worker> <count> <duration_ms> | invoke <worker> <iterations>"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("idle") if args.len() == 4 => {
            let worker = Path::new(&args[1]);
            let count = args[2].parse::<usize>().unwrap_or_else(|_| usage());
            let duration_ms =
                args[3].parse::<u64>().unwrap_or_else(|_| usage());
            run_idle(worker, count, duration_ms);
        }
        Some("invoke") if args.len() == 3 => {
            let worker = Path::new(&args[1]);
            let iterations =
                args[2].parse::<usize>().unwrap_or_else(|_| usage());
            if iterations == 0 {
                usage();
            }
            run_invoke(worker, iterations);
        }
        _ => usage(),
    }
}
