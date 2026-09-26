#![cfg(windows)]

use relay::client;
use relay_adapter::{
    AdapterIdentity, AdapterManifest, ArtifactMetadata, CommandBinding, ComponentMetadata,
    PublisherMetadata, RelayCompatibility, RequestedPermissions, TargetRequirement, TrustMetadata,
    UpdateMetadata, sha256_file,
};
use relay_contracts::{CommandRequest, CommandResponse, LocalHostState, RequestContext};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static READ_ID: AtomicU64 = AtomicU64::new(1);

fn fixture_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase3-parser-{}-{nanos}",
        std::process::id()
    ))
}

fn request(id: &str, command: &str, arguments: Value, keyed: bool) -> CommandRequest {
    CommandRequest {
        request_id: id.to_string(),
        command: command.to_string(),
        command_version: Some(1),
        arguments,
        idempotency_key: keyed.then(|| format!("IDEMP-{id}")),
        context: RequestContext::default(),
    }
}

fn call(
    state: &LocalHostState,
    id: &str,
    command: &str,
    arguments: Value,
    keyed: bool,
) -> CommandResponse {
    client::call(state, &request(id, command, arguments, keyed)).expect("daemon command")
}

fn spawn_host(state_dir: &Path) -> (Child, LocalHostState) {
    let instance = state_dir
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy();
    let child = Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_TEST_DISABLE_WATCHER", "1")
        .env("RELAY_TEST_PARSER_IDLE_MS", "1")
        .env("RELAY_STATE_DIR", state_dir)
        .env("RELAY_INSTANCE", instance.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn relayd");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(state_dir.join("host.json"))
            && let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes)
            && state.pid == child.id()
        {
            return (child, state);
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("daemon did not become ready");
}

fn install_fixture(state_dir: &Path, root: &Path) {
    fs::create_dir_all(state_dir).unwrap();
    let package = root.join("installed-parser");
    fs::create_dir_all(&package).unwrap();
    let source =
        PathBuf::from(env!("CARGO_BIN_EXE_relayd")).with_file_name("relay-adapter-fixture.exe");
    assert!(
        source.is_file(),
        "build relay-adapter-fixture before this targeted test"
    );
    let worker = package.join("relay-adapter-fixture.exe");
    fs::copy(&source, &worker).unwrap();
    let digest = sha256_file(&worker).unwrap();
    let manifest = AdapterManifest {
        manifest_format: 1,
        adapter: AdapterIdentity {
            id: "fixture.parser".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Generic fixture parser".to_string(),
        },
        publisher: PublisherMetadata {
            id: "fixture.publisher".to_string(),
            source: "local-test".to_string(),
        },
        artifact: ArtifactMetadata {
            sha256: digest.clone(),
            source: "local-test".to_string(),
        },
        relay: RelayCompatibility {
            protocol_min: 1,
            protocol_max: 1,
            command_bindings: vec![CommandBinding {
                command: "adapter.dependencies.parse".to_string(),
                command_version: 1,
                capability: "synthetic.dependencies.parse".to_string(),
            }],
        },
        permissions: RequestedPermissions {
            project_read: vec!["PRJ-parser-alpha".to_string()],
            ..RequestedPermissions::default()
        },
        target: TargetRequirement {
            tool: "synthetic".to_string(),
            version: "1.0.0".to_string(),
        },
        components: vec![ComponentMetadata {
            id: "worker".to_string(),
            kind: "worker".to_string(),
            version: "1.0.0".to_string(),
            sha256: digest.clone(),
            source: "local-test".to_string(),
        }],
        dependencies: Vec::new(),
        update: UpdateMetadata {
            channel: "local-test".to_string(),
            source: "local-test".to_string(),
        },
        trust: TrustMetadata {
            build_provenance: "synthetic-fixture".to_string(),
            review_status: "test-fixture".to_string(),
        },
    };
    let manifest_path = package.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(
        state_dir.join("parser-installations.json"),
        serde_json::to_vec_pretty(&json!({
            "format_version": 1,
            "installations": [{
                "project_id": "PRJ-parser-alpha",
                "manifest_path": manifest_path,
                "worker_path": worker,
                "adapter_id": "fixture.parser",
                "adapter_version": "1.0.0",
                "worker_sha256": digest,
                "target_tool": "synthetic",
                "target_version": "1.0.0",
                "allow_source_delivery": true,
                "source_extensions": ["json"]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn edges(state: &LocalHostState, project_id: &str) -> Vec<String> {
    let response = call(
        state,
        &format!(
            "edges-{project_id}-{}",
            READ_ID.fetch_add(1, Ordering::Relaxed)
        ),
        "project.dependencies.list",
        json!({ "project_id": project_id }),
        false,
    );
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["target_path"].as_str().unwrap().to_string())
        .collect()
}

fn wait_edges(state: &LocalHostState, project_id: &str, expected: &[&str]) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let current = edges(state, project_id);
        if current == expected {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "parser edges did not reach {expected:?}; found {:?}",
        edges(state, project_id)
    );
}

#[test]
fn installed_parser_extracts_and_reparses_only_authorized_project() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let alpha = dir.join("alpha");
    let bravo = dir.join("bravo");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&bravo).unwrap();
    install_fixture(&state_dir, &dir);
    for root in [&alpha, &bravo] {
        fs::write(root.join("source.json"), br#"{"targets":["target1.txt"]}"#).unwrap();
        fs::write(root.join("target1.txt"), b"one").unwrap();
        fs::write(root.join("target2.txt"), b"two").unwrap();
    }
    let (mut host, state) = spawn_host(&state_dir);
    for (id, root) in [("PRJ-parser-alpha", &alpha), ("PRJ-parser-bravo", &bravo)] {
        let imported = call(
            &state,
            &format!("import-{id}"),
            "project.import",
            json!({
                "id": id, "name": id, "root_path": root.to_string_lossy()
            }),
            true,
        );
        assert!(imported.ok, "{:?}", imported.error);
        let configured = call(
            &state,
            &format!("config-{id}"),
            "project.configuration.put",
            json!({
                "project_id": id, "expected_revision": 0, "format_version": 1,
                "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "1.0.0"
            }),
            true,
        );
        assert!(configured.ok, "{:?}", configured.error);
        let built = call(
            &state,
            &format!("build-{id}"),
            "project.index.build",
            json!({ "project_id": id }),
            true,
        );
        assert!(built.ok, "{:?}", built.error);
    }
    wait_edges(&state, "PRJ-parser-alpha", &["target1.txt"]);
    assert!(edges(&state, "PRJ-parser-bravo").is_empty());

    fs::write(alpha.join("source.json"), br#"{"targets":["target2.txt"]}"#).unwrap();
    let updated = call(
        &state,
        "reconcile-source",
        "project.index.reconcile",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(updated.ok, "{:?}", updated.error);
    wait_edges(&state, "PRJ-parser-alpha", &["target2.txt"]);

    fs::remove_file(alpha.join("target2.txt")).unwrap();
    let deleted = call(
        &state,
        "reconcile-delete",
        "project.index.reconcile",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(deleted.ok, "{:?}", deleted.error);
    wait_edges(&state, "PRJ-parser-alpha", &[]);

    fs::write(alpha.join("target2.txt"), b"restored").unwrap();
    let restored = call(
        &state,
        "reconcile-restore",
        "project.index.reconcile",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(restored.ok, "{:?}", restored.error);
    wait_edges(&state, "PRJ-parser-alpha", &["target2.txt"]);

    let revoked = call(
        &state,
        "config-revoke",
        "project.configuration.put",
        json!({
            "project_id": "PRJ-parser-alpha", "expected_revision": 1, "format_version": 1,
            "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "2.0.0"
        }),
        true,
    );
    assert!(revoked.ok, "{:?}", revoked.error);
    wait_edges(&state, "PRJ-parser-alpha", &[]);
    let stale_guard = call(
        &state,
        "stale-parser-selection",
        "project.dependencies.replace",
        json!({
            "project_id": "PRJ-parser-alpha",
            "expected_generation": restored.result.unwrap()["generation"],
            "source_path": "source.json",
            "source_sha256": sha256_file(&alpha.join("source.json")).unwrap(),
            "producer_id": "fixture.parser",
            "producer_version": "1.0.0",
            "expected_configuration_revision": 1,
            "expected_adapter_id": "fixture.parser",
            "expected_adapter_version": "1.0.0",
            "targets": ["target2.txt"]
        }),
        true,
    );
    assert_eq!(stale_guard.error.unwrap().code, "PROJECT_CONFIG_CONFLICT");
    let stopped = call(&state, "shutdown", "system.shutdown", json!({}), false);
    assert!(stopped.ok);
    assert!(host.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn installation_grant_rejects_worker_digest_mismatch_before_daemon_readiness() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    install_fixture(&state_dir, &dir);
    let path = state_dir.join("parser-installations.json");
    let mut grant: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    grant["installations"][0]["worker_sha256"] = json!("0".repeat(64));
    fs::write(&path, serde_json::to_vec(&grant).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_STATE_DIR", &state_dir)
        .env("RELAY_INSTANCE", "parser-digest-reject")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!state_dir.join("host.json").exists());
    fs::remove_dir_all(dir).unwrap();
}
