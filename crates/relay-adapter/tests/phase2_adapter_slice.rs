#![cfg(windows)]

use relay_adapter::{
    sha256_file, AdapterBroker, AdapterIdentity, AdapterManifest,
    ArtifactMetadata, BrokerPolicy, CommandBinding, ComponentMetadata,
    DependencyMetadata, PublisherMetadata, RelayCompatibility,
    RequestedPermissions, TargetRequirement, TrustMetadata, UpdateMetadata,
};
use relay_adapter::sandbox::security_descriptor_sddl;
use serde_json::json;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase2-adapter-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn copy_worker(root: &Path) -> PathBuf {
    let worker_dir = root.join("worker");
    fs::create_dir_all(&worker_dir).unwrap();
    let source = PathBuf::from(env!("CARGO_BIN_EXE_relay-adapter-fixture"));
    let target = worker_dir.join("relay-adapter-fixture.exe");
    fs::copy(source, &target).unwrap();
    target
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
            source: "phase2-test".to_string(),
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
            build_provenance: "phase2-synthetic".to_string(),
            review_status: "test-fixture".to_string(),
        },
    }
}


#[test]
fn stable_sandbox_blocks_ungranted_worker_access() {
    let root = unique_dir("sandbox");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let worker_dir = worker.parent().unwrap().to_path_buf();
    let worker_sddl_before =
        security_descriptor_sddl(&worker_dir).expect("worker SDDL");
    let mailbox_root = root.join("mailboxes");
    let blocked = root.join("blocked");
    fs::create_dir_all(&mailbox_root).unwrap();
    fs::create_dir_all(&blocked).unwrap();

    let blocked_secret = blocked.join("secret.txt");
    let blocked_write = blocked.join("escape.txt");
    let read_only_write = worker_dir.join("should-not-write.txt");
    fs::write(&blocked_secret, b"blocked-secret").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let network_target = listener.local_addr().unwrap().to_string();

    let broker =
        AdapterBroker::new(BrokerPolicy::synthetic_default(), &mailbox_root)
            .unwrap();
    broker
        .install(manifest_for("fixture.security", &worker), &worker)
        .unwrap();

    unsafe {
        std::env::set_var(
            "RELAY_ADAPTER_PARENT_SECRET",
            "synthetic-parent-secret",
        );
    }
    let outcome = broker.invoke(
        "fixture.security",
        "system.echo",
        1,
        json!({
            "_fixture_mode": "security_probe",
            "blocked_secret": blocked_secret,
            "blocked_write": blocked_write,
            "read_only_write": read_only_write,
            "network_target": network_target
        }),
    );
    unsafe {
        std::env::remove_var("RELAY_ADAPTER_PARENT_SECRET");
    }
    let outcome = outcome.expect("sandboxed invocation");
    let probe = &outcome.response["result"]["echo"];

    assert_eq!(probe["is_app_container"], true);
    assert_eq!(probe["capability_count"], 0);
    assert_eq!(probe["allowed_mailbox_write_ok"], true);
    assert_eq!(probe["blocked_read_ok"], false);
    assert_eq!(probe["blocked_write_ok"], false);

    assert_eq!(probe["read_only_write_ok"], false);
    assert_eq!(probe["network_connect_ok"], false);
    assert_eq!(probe["parent_secret_visible"], false);
    assert_eq!(probe["user_profile_visible"], false);
    assert_eq!(probe["child_process_created"], false);
    assert_eq!(probe["memory_reservation_succeeded"], false);

    assert_eq!(outcome.job_limits.active_process_limit, 1);
    assert_eq!(
        outcome.job_limits.process_memory_limit_bytes,
        32 * 1024 * 1024
    );
    assert!(outcome.job_limits.kill_on_close);
    assert_eq!(
        outcome.sandbox_backend,
        "legacy_appcontainer_lpac"
    );

    assert!(!blocked_write.exists());
    assert!(!read_only_write.exists());
    assert_eq!(
        security_descriptor_sddl(&worker_dir).unwrap(),
        worker_sddl_before
    );
    assert_eq!(
        fs::read_dir(&mailbox_root).unwrap().count(),
        0
    );

    drop(listener);
    fs::remove_dir_all(root).unwrap();
}


#[test]
fn manifest_integrity_permissions_and_bindings_fail_closed() {
    let root = unique_dir("manifest");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");
    let policy = BrokerPolicy::synthetic_default();

    let mut over = manifest_for("fixture.over", &worker);
    over.permissions.network = true;
    let broker = AdapterBroker::new(policy.clone(), &mailbox_root).unwrap();
    let error = broker.install(over, &worker).unwrap_err();
    assert_eq!(error.code, "ADAPTER_PERMISSION_DENIED");

    let mut digest = manifest_for("fixture.digest", &worker);
    digest.artifact.sha256 = "0".repeat(64);
    let error = broker.install(digest, &worker).unwrap_err();
    assert_eq!(error.code, "ADAPTER_INTEGRITY_MISMATCH");

    let mut component = manifest_for("fixture.component", &worker);
    component.components[0].sha256 = "1".repeat(64);
    let error = broker.install(component, &worker).unwrap_err();
    assert_eq!(error.code, "ADAPTER_INTEGRITY_MISMATCH");


    let mut protocol = manifest_for("fixture.protocol", &worker);
    protocol.relay.protocol_min = 99;
    protocol.relay.protocol_max = 99;
    let error = broker.install(protocol, &worker).unwrap_err();
    assert_eq!(error.code, "ADAPTER_PROTOCOL_INCOMPATIBLE");

    let mut command = manifest_for("fixture.command", &worker);
    command.relay.command_bindings[0].command =
        "does.not.exist".to_string();
    let error = broker.install(command, &worker).unwrap_err();
    assert_eq!(error.code, "ADAPTER_COMMAND_INCOMPATIBLE");

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn crash_backoff_invalid_response_and_quarantine_are_bounded() {
    let root = unique_dir("quarantine");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");

    let mut policy = BrokerPolicy::synthetic_default();
    policy.backoff_ms = 25;
    policy.max_failures_before_quarantine = 2;
    let broker = AdapterBroker::new(policy, &mailbox_root).unwrap();
    broker
        .install(manifest_for("fixture.quarantine", &worker), &worker)
        .unwrap();

    let first = broker
        .invoke(
            "fixture.quarantine",
            "system.echo",
            1,
            json!({ "_fixture_mode": "crash" }),
        )
        .unwrap_err();
    assert_eq!(first.code, "STABLE_SANDBOX_WORKER_EXITED");

    let backoff = broker
        .invoke(
            "fixture.quarantine",
            "system.echo",
            1,
            json!({}),
        )
        .unwrap_err();
    assert_eq!(backoff.code, "ADAPTER_BACKOFF");

    thread::sleep(Duration::from_millis(35));
    let second = broker
        .invoke(
            "fixture.quarantine",
            "system.echo",
            1,
            json!({ "_fixture_mode": "invalid_json" }),
        )
        .unwrap_err();
    assert_eq!(second.code, "STABLE_SANDBOX_RESPONSE_INVALID");
    assert!(broker.is_quarantined("fixture.quarantine"));

    let quarantined = broker
        .invoke(
            "fixture.quarantine",
            "system.echo",
            1,
            json!({}),
        )
        .unwrap_err();
    assert_eq!(quarantined.code, "ADAPTER_QUARANTINED");
    assert_eq!(fs::read_dir(&mailbox_root).unwrap().count(), 0);

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn hang_times_out_and_mailbox_is_cleaned() {
    let root = unique_dir("hang");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");

    let mut policy = BrokerPolicy::synthetic_default();
    policy.request_timeout_ms = 100;
    let broker = AdapterBroker::new(policy, &mailbox_root).unwrap();
    broker
        .install(manifest_for("fixture.hang", &worker), &worker)
        .unwrap();

    let started = std::time::Instant::now();
    let error = broker
        .invoke(
            "fixture.hang",
            "system.echo",
            1,
            json!({ "_fixture_mode": "hang" }),
        )
        .unwrap_err();
    assert_eq!(error.code, "STABLE_SANDBOX_TIMEOUT");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(fs::read_dir(&mailbox_root).unwrap().count(), 0);

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn worker_identity_protocol_capability_and_result_contracts_fail_closed() {
    let root = unique_dir("response-contracts");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");

    let cases = [
        ("identity_mismatch", "ADAPTER_IDENTITY_MISMATCH"),
        ("version_mismatch", "ADAPTER_IDENTITY_MISMATCH"),
        ("bad_protocol", "ADAPTER_PROTOCOL_INCOMPATIBLE"),
        ("extra_capability", "ADAPTER_CAPABILITY_MISMATCH"),
        ("bad_result", "ADAPTER_RESULT_SCHEMA_VIOLATION"),
        ("undeclared_error", "ADAPTER_UNDECLARED_ERROR"),
    ];

    for (index, (mode, expected)) in cases.iter().enumerate() {
        let id = format!("fixture.contract.{index}");
        let broker = AdapterBroker::new(
            BrokerPolicy::synthetic_default(),
            &mailbox_root,
        )
        .unwrap();
        broker.install(manifest_for(&id, &worker), &worker).unwrap();

        let error = broker
            .invoke(
                &id,
                "system.echo",
                1,
                json!({ "_fixture_mode": mode }),
            )
            .unwrap_err();
        assert_eq!(error.code, *expected, "mode={mode}");
        assert_eq!(fs::read_dir(&mailbox_root).unwrap().count(), 0);
    }

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn installed_worker_is_reverified_and_uninstall_clears_lifecycle_state() {
    let root = unique_dir("lifecycle");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");
    let broker =
        AdapterBroker::new(BrokerPolicy::synthetic_default(), &mailbox_root)
            .unwrap();

    broker
        .install(manifest_for("fixture.lifecycle", &worker), &worker)
        .unwrap();
    assert_eq!(broker.installed_count(), 1);

    let original = fs::read(&worker).unwrap();
    let mut tampered = original.clone();
    let index = tampered.len() / 2;
    tampered[index] ^= 0x01;
    fs::write(&worker, &tampered).unwrap();

    let error = broker
        .invoke(
            "fixture.lifecycle",
            "system.echo",
            1,
            json!({}),
        )
        .unwrap_err();
    assert_eq!(error.code, "ADAPTER_INTEGRITY_MISMATCH");

    fs::write(&worker, &original).unwrap();
    broker.clear_quarantine("fixture.lifecycle");
    let outcome = broker
        .invoke(
            "fixture.lifecycle",
            "system.echo",
            1,
            json!({ "restored": true }),
        )
        .expect("restored worker invocation");
    assert_eq!(outcome.response["result"]["echo"]["restored"], true);

    broker.uninstall("fixture.lifecycle").unwrap();
    assert_eq!(broker.installed_count(), 0);
    let error = broker
        .invoke(
            "fixture.lifecycle",
            "system.echo",
            1,
            json!({}),
        )
        .unwrap_err();
    assert_eq!(error.code, "ADAPTER_NOT_INSTALLED");

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn every_ungranted_manifest_permission_class_fails_closed() {
    let root = unique_dir("permissions");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let mailbox_root = root.join("mailboxes");

    let mut manifests = Vec::new();

    let mut network = manifest_for("fixture.permission.network", &worker);
    network.permissions.network = true;
    manifests.push(network);

    let mut subprocess = manifest_for("fixture.permission.subprocess", &worker);
    subprocess.permissions.subprocess = true;
    manifests.push(subprocess);

    let mut read = manifest_for("fixture.permission.read", &worker);
    read.permissions.project_read.push("project-a".to_string());
    manifests.push(read);

    let mut write = manifest_for("fixture.permission.write", &worker);
    write.permissions.project_write.push("project-a".to_string());
    manifests.push(write);

    let mut credential = manifest_for("fixture.permission.credential", &worker);
    credential
        .permissions
        .credentials
        .push("CRED-fixture".to_string());
    manifests.push(credential);

    let mut app = manifest_for("fixture.permission.app", &worker);
    app.permissions.external_apps.push("fixture.app".to_string());
    manifests.push(app);

    for manifest in manifests {
        let broker = AdapterBroker::new(
            BrokerPolicy::synthetic_default(),
            &mailbox_root,
        )
        .unwrap();
        let error = broker.install(manifest, &worker).unwrap_err();
        assert_eq!(error.code, "ADAPTER_PERMISSION_DENIED");
    }

    fs::remove_dir_all(root).unwrap();
}


#[test]
fn concurrent_invocations_share_package_lock_and_restore_acl() {
    let root = unique_dir("concurrent");
    fs::create_dir_all(&root).unwrap();
    let worker = copy_worker(&root);
    let worker_dir = worker.parent().unwrap().to_path_buf();
    let sddl_before =
        security_descriptor_sddl(&worker_dir).expect("worker SDDL");
    let mailbox_root = root.join("mailboxes");

    let broker = Arc::new(
        AdapterBroker::new(
            BrokerPolicy::synthetic_default(),
            &mailbox_root,
        )
        .unwrap(),
    );
    broker
        .install(manifest_for("fixture.concurrent", &worker), &worker)
        .unwrap();

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for sequence in 0..4usize {
        let broker = Arc::clone(&broker);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            broker
                .invoke(
                    "fixture.concurrent",
                    "system.echo",
                    1,
                    json!({ "sequence": sequence }),
                )
                .expect("concurrent invocation")
        }));
    }

    for handle in handles {
        let outcome = handle.join().expect("worker thread");
        assert_eq!(outcome.response["ok"], true);
    }

    assert_eq!(
        security_descriptor_sddl(&worker_dir).unwrap(),
        sddl_before
    );
    assert_eq!(fs::read_dir(&mailbox_root).unwrap().count(), 0);

    drop(broker);
    fs::remove_dir_all(root).unwrap();
}
