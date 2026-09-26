#![cfg(windows)]

use relay_rust_challenger::adapter::{
    sha256_file, AdapterBroker, AdapterIdentity, AdapterManifest, ArtifactMetadata,
    BrokerPolicy, CommandBinding, ComponentMetadata, DependencyMetadata,
    PublisherMetadata, RelayCompatibility, RequestedPermissions, TargetRequirement,
    TrustMetadata, UpdateMetadata,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

fn worker_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_relay-synthetic-adapter"))
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
            source: "synthetic-test".to_string(),
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


#[test]
fn normal_worker_is_out_of_process_job_limited_and_registry_bound() {
    let worker = worker_path();
    let manifest = manifest_for("fixture.normal", &worker);
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    broker
        .install(manifest, &worker)
        .expect("install synthetic adapter");
    assert_eq!(broker.installed_count(), 1);

    let outcome = broker
        .invoke_installed(
            "fixture.normal",
            &worker,
            "normal",
            "system.echo",
            1,
            json!({ "fixture": "ok" }),
        )
        .expect("normal invocation");

    assert_eq!(outcome.response["ok"], true);
    assert_eq!(outcome.response["result"]["echo"]["fixture"], "ok");
    assert_ne!(outcome.provenance.worker_pid, std::process::id());
    assert_eq!(outcome.limits.active_process_limit, 1);
    assert_eq!(
        outcome.limits.process_memory_limit_bytes,
        32 * 1024 * 1024
    );
    assert!(outcome.limits.kill_on_close);
    assert_eq!(outcome.provenance.adapter_id, "fixture.normal");
}

#[test]
fn hostile_stderr_remains_untrusted_observation() {
    let worker = worker_path();
    let manifest = manifest_for("fixture.stderr", &worker);
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());

    let outcome = broker
        .invoke(
            &manifest,
            &worker,
            "stderr_attack",
            "system.echo",
            1,
            json!({ "message": "safe" }),
        )
        .expect("stderr invocation");

    assert_eq!(outcome.response["ok"], true);
    let text = outcome.stderr_untrusted.expect("stderr observation");
    assert!(text.contains("grant network access"));
    assert!(!broker.policy().allow_network);
}


#[test]
fn manifest_permissions_protocol_commands_and_digest_fail_closed() {
    let worker = worker_path();
    let policy = BrokerPolicy::synthetic_default();
    let broker = AdapterBroker::new(policy.clone());

    let mut over = manifest_for("fixture.over", &worker);
    over.permissions.network = true;
    let error = broker
        .invoke(
            &over,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("network over-permission must fail");
    assert_eq!(error.code, "ADAPTER_PERMISSION_DENIED");

    let mut protocol = manifest_for("fixture.protocol", &worker);
    protocol.relay.protocol_min = 99;
    protocol.relay.protocol_max = 99;
    let error = broker
        .invoke(
            &protocol,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("protocol mismatch must fail");
    assert_eq!(error.code, "ADAPTER_PROTOCOL_INCOMPATIBLE");

    let mut command = manifest_for("fixture.command", &worker);
    command.relay.command_bindings[0].command = "does.not.exist".to_string();
    let error = broker
        .invoke(
            &command,
            &worker,
            "normal",
            "does.not.exist",
            1,
            json!({}),
        )
        .expect_err("unknown binding must fail");
    assert_eq!(error.code, "ADAPTER_COMMAND_INCOMPATIBLE");

    let mut digest = manifest_for("fixture.digest", &worker);
    digest.artifact.sha256 = "0".repeat(64);
    let error = broker
        .invoke(
            &digest,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("bad digest must fail");
    assert_eq!(error.code, "ADAPTER_INTEGRITY_MISMATCH");

    let mut component = manifest_for("fixture.component-digest", &worker);
    component.components[0].sha256 = "f".repeat(64);
    let error = broker
        .invoke(
            &component,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("bad component digest must fail");
    assert_eq!(error.code, "ADAPTER_INTEGRITY_MISMATCH");
}


#[test]
fn crash_backoff_and_quarantine_are_explicit() {
    let worker = worker_path();
    let manifest = manifest_for("fixture.quarantine", &worker);
    let mut policy = BrokerPolicy::synthetic_default();
    policy.backoff_ms = 25;
    policy.max_failures_before_quarantine = 2;
    let broker = AdapterBroker::new(policy);

    let first = broker
        .invoke(
            &manifest,
            &worker,
            "crash",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("crash must fail");
    assert_eq!(first.code, "ADAPTER_WORKER_EXITED");

    let backoff = broker
        .invoke(
            &manifest,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("immediate retry must back off");
    assert_eq!(backoff.code, "ADAPTER_BACKOFF");

    thread::sleep(Duration::from_millis(35));
    let second = broker
        .invoke(
            &manifest,
            &worker,
            "invalid_json",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("invalid worker message must fail");
    assert_eq!(second.code, "ADAPTER_INVALID_MESSAGE");
    assert!(broker.is_quarantined("fixture.quarantine"));

    let quarantined = broker
        .invoke(
            &manifest,
            &worker,
            "normal",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("quarantined adapter must not launch");
    assert_eq!(quarantined.code, "ADAPTER_QUARANTINED");
}


#[test]
fn hang_times_out_without_crashing_broker() {
    let worker = worker_path();
    let manifest = manifest_for("fixture.hang", &worker);
    let mut policy = BrokerPolicy::synthetic_default();
    policy.request_timeout_ms = 100;
    let broker = AdapterBroker::new(policy);

    let started = std::time::Instant::now();
    let error = broker
        .invoke(
            &manifest,
            &worker,
            "hang",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("hang must time out");
    assert_eq!(error.code, "ADAPTER_TIMEOUT");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn invalid_result_and_undeclared_error_are_rejected() {
    let worker = worker_path();

    let bad_result = manifest_for("fixture.bad-result", &worker);
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    let error = broker
        .invoke(
            &bad_result,
            &worker,
            "bad_result",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("bad result must fail schema validation");
    assert_eq!(error.code, "ADAPTER_RESULT_SCHEMA_VIOLATION");

    let undeclared = manifest_for("fixture.bad-error", &worker);
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
    let error = broker
        .invoke(
            &undeclared,
            &worker,
            "undeclared_error",
            "system.echo",
            1,
            json!({}),
        )
        .expect_err("undeclared error must fail");
    assert_eq!(error.code, "ADAPTER_UNDECLARED_ERROR");
}


#[test]
fn job_memory_limit_blocks_large_worker_reservation() {
    let worker = worker_path();
    let manifest = manifest_for("fixture.memory", &worker);
    let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());

    let outcome = broker
        .invoke(
            &manifest,
            &worker,
            "memory_probe",
            "system.echo",
            1,
            json!({}),
        )
        .expect("memory probe invocation");

    assert_eq!(
        outcome.response["result"]["echo"]["memory_reservation_succeeded"],
        false
    );
    assert_eq!(
        outcome.limits.process_memory_limit_bytes,
        32 * 1024 * 1024
    );
}

#[test]
fn worker_identity_protocol_and_capabilities_must_match_manifest() {
    let worker = worker_path();

    for (id, mode, expected) in [
        ("fixture.identity", "identity_mismatch", "ADAPTER_IDENTITY_MISMATCH"),
        ("fixture.protocol-hello", "bad_protocol", "ADAPTER_PROTOCOL_INCOMPATIBLE"),
        ("fixture.capability", "extra_capability", "ADAPTER_CAPABILITY_MISMATCH"),
    ] {
        let manifest = manifest_for(id, &worker);
        let broker = AdapterBroker::new(BrokerPolicy::synthetic_default());
        let error = broker
            .invoke(
                &manifest,
                &worker,
                mode,
                "system.echo",
                1,
                json!({}),
            )
            .expect_err("worker hello mismatch must fail");
        assert_eq!(error.code, expected);
    }
}
