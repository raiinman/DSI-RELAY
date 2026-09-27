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
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{FILETIME, HANDLE};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetProcessTimes;

static READ_ID: AtomicU64 = AtomicU64::new(1);

struct TestHost(Child);

impl Drop for TestHost {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn fixture_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase3-parser-resource-{}-{nanos}",
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

fn spawn_host(state_dir: &Path) -> (TestHost, LocalHostState) {
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
            return (TestHost(child), state);
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("daemon did not become ready");
}

fn process_sample(child: &Child) -> (u64, u64) {
    let handle = child.as_raw_handle() as HANDLE;
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    assert_ne!(
        unsafe { K32GetProcessMemoryInfo(handle, &mut memory, memory.cb) },
        0
    );
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    assert_ne!(
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) },
        0
    );
    let ticks = |time: FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    (
        memory.WorkingSetSize as u64,
        (ticks(kernel) + ticks(user)) / 10_000,
    )
}

fn idle_sample(child: &Child) -> (u64, u64) {
    let (_, before) = process_sample(child);
    thread::sleep(Duration::from_secs(5));
    let (rss, after) = process_sample(child);
    (rss, after.saturating_sub(before))
}

fn install_fixture(state_dir: &Path, root: &Path) {
    let package = root.join("installed-parser");
    fs::create_dir_all(&package).unwrap();
    let source =
        PathBuf::from(env!("CARGO_BIN_EXE_relayd")).with_file_name("relay-adapter-fixture.exe");
    assert!(source.is_file(), "build relay-adapter-fixture first");
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
            project_read: vec!["PRJ-parser-resource".to_string()],
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
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(
        state_dir.join("parser-installations.json"),
        serde_json::to_vec(&json!({
            "format_version": 1,
            "installations": [{
                "project_id": "PRJ-parser-resource",
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

fn edge_target(state: &LocalHostState) -> Option<String> {
    let sequence = READ_ID.fetch_add(1, Ordering::Relaxed);
    let response = call(
        state,
        &format!("edges-{sequence}"),
        "project.dependencies.list",
        json!({ "project_id": "PRJ-parser-resource" }),
        false,
    );
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()["edges"]
        .as_array()
        .unwrap()
        .first()
        .map(|edge| edge["target_path"].as_str().unwrap().to_string())
}

fn wait_target(state: &LocalHostState, expected: &str) -> u64 {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(15);
    while Instant::now() < deadline {
        if edge_target(state).as_deref() == Some(expected) {
            return start.elapsed().as_millis() as u64;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("parser did not publish expected target {expected}");
}

#[test]
#[ignore = "manual installed-parser resource sample; launches sandboxed worker repeatedly"]
fn installed_parser_idle_and_reparse_cost() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root = dir.join("project");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("source.json"), br#"{"targets":["target-a.txt"]}"#).unwrap();
    fs::write(root.join("target-a.txt"), b"a").unwrap();
    fs::write(root.join("target-b.txt"), b"b").unwrap();

    let (mut unbound_host, unbound_state) = spawn_host(&state_dir);
    let imported = call(
        &unbound_state,
        "resource-import",
        "project.import",
        json!({ "id": "PRJ-parser-resource", "name": "Parser resource fixture", "root_path": root.to_string_lossy() }),
        true,
    );
    assert!(imported.ok, "{:?}", imported.error);
    let configured = call(
        &unbound_state,
        "resource-config",
        "project.configuration.put",
        json!({
            "project_id": "PRJ-parser-resource", "expected_revision": 0, "format_version": 1,
            "project_type": "synthetic.project", "adapter_id": "fixture.parser", "adapter_version": "1.0.0"
        }),
        true,
    );
    assert!(configured.ok, "{:?}", configured.error);
    let built = call(
        &unbound_state,
        "resource-build",
        "project.index.build",
        json!({ "project_id": "PRJ-parser-resource" }),
        true,
    );
    assert!(built.ok, "{:?}", built.error);
    let (unbound_rss, unbound_cpu) = idle_sample(&unbound_host.0);
    assert!(edge_target(&unbound_state).is_none());
    assert!(
        call(
            &unbound_state,
            "resource-stop-1",
            "system.shutdown",
            json!({}),
            false
        )
        .ok
    );
    assert!(unbound_host.0.wait().unwrap().success());

    install_fixture(&state_dir, &dir);
    let (mut installed_host, installed_state) = spawn_host(&state_dir);
    let verified = call(
        &installed_state,
        "resource-verify",
        "project.index.reconcile",
        json!({ "project_id": "PRJ-parser-resource", "verify_content": true }),
        true,
    );
    assert!(verified.ok, "{:?}", verified.error);
    let initial_publish_ms = wait_target(&installed_state, "target-a.txt");
    let mut reparse_publish_ms = Vec::new();
    for round in 0..5 {
        let expected = if round % 2 == 0 {
            "target-b.txt"
        } else {
            "target-a.txt"
        };
        fs::write(
            root.join("source.json"),
            format!(r#"{{"targets":["{expected}"]}}"#),
        )
        .unwrap();
        let response = call(
            &installed_state,
            &format!("resource-reconcile-{round}"),
            "project.index.reconcile",
            json!({ "project_id": "PRJ-parser-resource" }),
            true,
        );
        assert!(response.ok, "{:?}", response.error);
        reparse_publish_ms.push(wait_target(&installed_state, expected));
    }
    let (installed_rss, installed_cpu) = idle_sample(&installed_host.0);
    assert_eq!(
        edge_target(&installed_state).as_deref(),
        Some("target-b.txt")
    );
    println!(
        "PHASE3_PARSER_RESOURCE_METRICS={}",
        json!({
            "parser_grants": 1,
            "source_files": 1,
            "source_bytes": fs::metadata(root.join("source.json")).unwrap().len(),
            "idle_sample_ms": 5000,
            "unbound_idle_rss_bytes": unbound_rss,
            "unbound_idle_cpu_ms": unbound_cpu,
            "installed_idle_rss_bytes": installed_rss,
            "installed_idle_cpu_ms": installed_cpu,
            "initial_publish_ms_after_reconcile": initial_publish_ms,
            "reparse_publish_ms_after_reconcile": reparse_publish_ms
        })
    );
    assert!(
        call(
            &installed_state,
            "resource-stop-2",
            "system.shutdown",
            json!({}),
            false
        )
        .ok
    );
    assert!(installed_host.0.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}
