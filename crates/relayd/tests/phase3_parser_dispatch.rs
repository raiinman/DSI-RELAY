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

#[test]
fn offline_installer_grant_activates_after_daemon_start() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root = dir.join("project");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("source.json"), br#"{"targets":["target.txt"]}"#).unwrap();
    fs::write(root.join("target.txt"), b"target").unwrap();
    install_fixture(&state_dir, &dir);
    fs::remove_file(state_dir.join("parser-installations.json")).unwrap();
    let package = dir.join("installed-parser");
    let installed = relay::parser_install::install_in_state_dir(
        &state_dir,
        relay::parser_install::InstallOptions {
            project_id: "PRJ-parser-alpha".to_string(),
            manifest_path: package.join("manifest.json"),
            worker_path: package.join("relay-adapter-fixture.exe"),
            source_extensions: vec!["json".to_string()],
            allow_source_delivery: true,
        },
    )
    .unwrap();
    assert!(installed.contains("Start RELAY"));
    let (mut host, state) = spawn_host(&state_dir);
    for (id, args) in [
        (
            "install-import",
            json!({ "id": "PRJ-parser-alpha", "name": "Parser install fixture", "root_path": root.to_string_lossy() }),
        ),
        (
            "install-config",
            json!({ "project_id": "PRJ-parser-alpha", "expected_revision": 0, "format_version": 1, "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "1.0.0" }),
        ),
        ("install-build", json!({ "project_id": "PRJ-parser-alpha" })),
    ] {
        let command = match id {
            "install-import" => "project.import",
            "install-config" => "project.configuration.put",
            _ => "project.index.build",
        };
        let response = call(&state, id, command, args, true);
        assert!(response.ok, "{:?}", response.error);
    }
    wait_edges(&state, "PRJ-parser-alpha", &["target.txt"]);
    assert_eq!(
        wait_parser_state(&state, "healthy")["host_components"][0]["installed_count"],
        1
    );
    assert!(call(&state, "install-stop", "system.shutdown", json!({}), false).ok);
    assert!(host.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
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

fn parser_status(state: &LocalHostState) -> Value {
    let id = format!("parser-health-{}", READ_ID.fetch_add(1, Ordering::Relaxed));
    let response = call(state, &id, "system.status", json!({}), false);
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()
}

fn wait_parser_state(state: &LocalHostState, expected: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        let status = parser_status(state);
        if status["host_components"][0]["state"] == expected {
            return status;
        }
        thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "parser health did not reach {expected}; found {:?}",
        parser_status(state)["host_components"]
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
fn invalid_installation_is_visible_and_recovers_after_restart() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    install_fixture(&state_dir, &dir);
    let path = state_dir.join("parser-installations.json");
    let mut grant: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    grant["installations"][0]["worker_sha256"] = json!("0".repeat(64));
    fs::write(&path, serde_json::to_vec(&grant).unwrap()).unwrap();
    let (mut first, state) = spawn_host(&state_dir);
    let degraded = wait_parser_state(&state, "degraded");
    assert_eq!(degraded["recovery_state"], "Degraded");
    assert_eq!(
        degraded["host_components"][0]["error_code"],
        "PARSER_INSTALLATION_INVALID"
    );
    let doctor = call(
        &state,
        "doctor-install-invalid",
        "system.doctor",
        json!({}),
        false,
    );
    assert!(!doctor.result.unwrap()["healthy"].as_bool().unwrap());
    let stopped = call(
        &state,
        "stop-install-invalid",
        "system.shutdown",
        json!({}),
        false,
    );
    assert!(stopped.ok);
    assert!(first.wait().unwrap().success());

    grant["installations"][0]["worker_sha256"] =
        json!(sha256_file(&dir.join("installed-parser/relay-adapter-fixture.exe")).unwrap());
    fs::write(&path, serde_json::to_vec(&grant).unwrap()).unwrap();
    let (mut second, state) = spawn_host(&state_dir);
    let recovered = wait_parser_state(&state, "healthy");
    assert_eq!(recovered["host_components"][0]["installed_count"], 1);
    let stopped = call(
        &state,
        "stop-install-recovered",
        "system.shutdown",
        json!({}),
        false,
    );
    assert!(stopped.ok);
    assert!(second.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_installation_is_visible_and_recovers_after_install_and_restart() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root = dir.join("project");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("source.json"), br#"{"targets":["target.txt"]}"#).unwrap();
    fs::write(root.join("target.txt"), b"target").unwrap();
    let (mut first, state) = spawn_host(&state_dir);
    let imported = call(
        &state,
        "missing-import",
        "project.import",
        json!({
            "id": "PRJ-parser-alpha", "name": "Missing parser fixture",
            "root_path": root.to_string_lossy()
        }),
        true,
    );
    assert!(imported.ok, "{:?}", imported.error);
    let configured = call(
        &state,
        "missing-config",
        "project.configuration.put",
        json!({
            "project_id": "PRJ-parser-alpha", "expected_revision": 0, "format_version": 1,
            "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "1.0.0"
        }),
        true,
    );
    assert!(configured.ok, "{:?}", configured.error);
    let built = call(
        &state,
        "missing-build",
        "project.index.build",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(built.ok, "{:?}", built.error);
    let missing = wait_parser_state(&state, "degraded");
    assert_eq!(
        missing["host_components"][0]["error_code"],
        "PARSER_NOT_INSTALLED"
    );
    assert_eq!(missing["host_components"][0]["unavailable_count"], 1);
    assert!(edges(&state, "PRJ-parser-alpha").is_empty());
    let stopped = call(&state, "missing-stop", "system.shutdown", json!({}), false);
    assert!(stopped.ok);
    assert!(first.wait().unwrap().success());

    install_fixture(&state_dir, &dir);
    let (mut second, state) = spawn_host(&state_dir);
    wait_edges(&state, "PRJ-parser-alpha", &["target.txt"]);
    let healthy = wait_parser_state(&state, "healthy");
    assert_eq!(healthy["host_components"][0]["unavailable_count"], 0);
    let stopped = call(
        &state,
        "missing-recovered-stop",
        "system.shutdown",
        json!({}),
        false,
    );
    assert!(stopped.ok);
    assert!(second.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn quarantined_parser_reports_failure_and_recovers_after_source_repair() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root = dir.join("project");
    fs::create_dir_all(&root).unwrap();
    install_fixture(&state_dir, &dir);
    let malformed = br#"{"targets":"invalid-target-list"}"#;
    fs::write(root.join("source.json"), malformed).unwrap();
    fs::write(root.join("target.txt"), b"target").unwrap();
    let (mut host, state) = spawn_host(&state_dir);
    let imported = call(
        &state,
        "health-import",
        "project.import",
        json!({
            "id": "PRJ-parser-alpha", "name": "Parser health fixture",
            "root_path": root.to_string_lossy()
        }),
        true,
    );
    assert!(imported.ok, "{:?}", imported.error);
    let configured = call(
        &state,
        "health-config",
        "project.configuration.put",
        json!({
            "project_id": "PRJ-parser-alpha", "expected_revision": 0, "format_version": 1,
            "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "1.0.0"
        }),
        true,
    );
    assert!(configured.ok, "{:?}", configured.error);
    let built = call(
        &state,
        "health-build",
        "project.index.build",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(built.ok, "{:?}", built.error);

    let deadline = Instant::now() + Duration::from_secs(20);
    let quarantined = loop {
        let status = parser_status(&state);
        if status["host_components"][0]["quarantined_count"] == 1 {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "parser did not quarantine: {:?}",
            status["host_components"]
        );
        thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(quarantined["recovery_state"], "Degraded");
    assert_eq!(
        quarantined["host_components"][0]["error_code"],
        "ADAPTER_QUARANTINED"
    );
    assert!(
        quarantined["host_components"][0]["failure_count"]
            .as_u64()
            .unwrap()
            >= 2
    );
    let doctor = call(
        &state,
        "doctor-quarantined",
        "system.doctor",
        json!({}),
        false,
    );
    let report = doctor.result.unwrap();
    assert_eq!(report["healthy"], false);
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == "adapter.dependencies.parse" && check["status"] == "fail")
    );

    fs::write(root.join("source.json"), br#"{"targets":["target.txt"]}"#).unwrap();
    let reconciled = call(
        &state,
        "health-reconcile",
        "project.index.reconcile",
        json!({
            "project_id": "PRJ-parser-alpha"
        }),
        true,
    );
    assert!(reconciled.ok, "{:?}", reconciled.error);
    wait_edges(&state, "PRJ-parser-alpha", &["target.txt"]);
    let healthy = wait_parser_state(&state, "healthy");
    assert_eq!(healthy["host_components"][0]["quarantined_count"], 0);
    assert_eq!(healthy["host_components"][0]["error_code"], Value::Null);
    assert!(
        healthy["host_components"][0]["success_count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let stopped = call(&state, "health-stop", "system.shutdown", json!({}), false);
    assert!(stopped.ok);
    assert!(host.wait().unwrap().success());
    let diagnostics =
        fs::read_to_string(state_dir.join("diagnostics/relay-diagnostics.jsonl")).unwrap();
    assert!(diagnostics.contains("relay.host.component.failed"));
    assert!(diagnostics.contains("relay.host.component.recovered"));
    assert!(!diagnostics.contains("invalid-target-list"));
    assert!(!diagnostics.contains(&root.to_string_lossy().to_string()));
    fs::remove_dir_all(dir).unwrap();
}
