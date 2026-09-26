#![cfg(windows)]

use relay::client;
use relay_contracts::{CommandRequest, CommandResponse, LocalHostState, RequestContext};
use relay_core::storage::RelayStorage;
use serde_json::{json, Value};
use std::fs;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{FILETIME, HANDLE};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetProcessTimes;

#[derive(Debug, Clone, Copy)]
struct ProcessSample {
    rss_bytes: u64,
    cpu_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct IdleSample {
    rss_bytes: u64,
    cpu_delta_ms: u64,
    sample_ms: u64,
}

fn unique_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase3-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn write_file(path: &Path, content: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn fixture_relative(index: usize) -> String {
    let group = index / 20;
    let ext = match index % 3 {
        0 => "txt",
        1 => "json",
        _ => "src",
    };
    format!("group-{group:02}/file-{index:03}.{ext}")
}

fn populate_project(root: &Path, tag: &str, count: usize) {
    for index in 0..count {
        let relative = fixture_relative(index);
        let payload = format!(
            "{tag}|{relative}|fixture-index={index}|{}",
            "x".repeat(48 + (index % 17))
        );
        write_file(&root.join(relative), payload.as_bytes());
    }
}

fn request(
    request_id: impl Into<String>,
    command: impl Into<String>,
    arguments: Value,
    idempotency_key: Option<&str>,
) -> CommandRequest {
    CommandRequest {
        request_id: request_id.into(),
        command: command.into(),
        command_version: Some(1),
        arguments,
        idempotency_key: idempotency_key.map(str::to_string),
        context: RequestContext::default(),
    }
}

fn call(state: &LocalHostState, request: CommandRequest) -> CommandResponse {
    client::call(state, &request).expect("client call")
}

fn wait_state(dir: &Path, expected_pid: u32) -> (LocalHostState, u64) {
    let path = dir.join("host.json");
    let started = Instant::now();
    let deadline = started + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes) {
                if state.pid == expected_pid {
                    return (state, started.elapsed().as_millis() as u64);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("host state did not become current for PID {expected_pid}");
}

fn spawn_host(dir: &Path, instance: &str) -> (Child, LocalHostState, u64) {
    let child = Command::new(env!("CARGO_BIN_EXE_relayd"))
        .env("RELAY_STATE_DIR", dir)
        .env("RELAY_INSTANCE", instance)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn relayd");
    let (state, ready_ms) = wait_state(dir, child.id());
    (child, state, ready_ms)
}

fn shutdown(state: &LocalHostState, child: &mut Child) {
    let response = call(
        state,
        request("REQ-phase3-shutdown", "system.shutdown", json!({}), None),
    );
    assert!(response.ok);
    assert!(child.wait().expect("wait relayd").success());
}

fn filetime_u64(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}

fn process_sample(child: &Child) -> ProcessSample {
    let handle = child.as_raw_handle() as HANDLE;
    let mut memory = PROCESS_MEMORY_COUNTERS::default();
    memory.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

    let memory_ok = unsafe { K32GetProcessMemoryInfo(handle, &mut memory, memory.cb) };
    assert_ne!(memory_ok, 0, "GetProcessMemoryInfo failed");

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let times_ok =
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    assert_ne!(times_ok, 0, "GetProcessTimes failed");

    ProcessSample {
        rss_bytes: memory.WorkingSetSize as u64,
        cpu_ms: (filetime_u64(kernel) + filetime_u64(user)) / 10_000,
    }
}

fn idle_sample(child: &Child, sample_ms: u64) -> IdleSample {
    let before = process_sample(child);
    thread::sleep(Duration::from_millis(sample_ms));
    let after = process_sample(child);
    IdleSample {
        rss_bytes: after.rss_bytes,
        cpu_delta_ms: after.cpu_ms.saturating_sub(before.cpu_ms),
        sample_ms,
    }
}

fn storage_bytes(dir: &Path) -> u64 {
    ["relay.sqlite3", "relay.sqlite3-wal", "relay.sqlite3-shm"]
        .iter()
        .map(|name| {
            fs::metadata(dir.join(name))
                .map(|metadata| metadata.len())
                .unwrap_or(0)
        })
        .sum()
}

fn import_project(
    state: &LocalHostState,
    id: &str,
    name: &str,
    root: &Path,
    request_id: &str,
    idempotency: &str,
) -> Value {
    let response = call(
        state,
        request(
            request_id,
            "project.import",
            json!({
                "id": id,
                "name": name,
                "root_path": root.to_string_lossy()
            }),
            Some(idempotency),
        ),
    );
    assert!(response.ok, "project import failed: {:?}", response.error);
    response.result.expect("project import result")
}

fn build_baseline(
    state: &LocalHostState,
    project_id: &str,
    request_id: &str,
    idempotency: &str,
) -> Value {
    let response = call(
        state,
        request(
            request_id,
            "project.index.build",
            json!({ "project_id": project_id }),
            Some(idempotency),
        ),
    );
    assert!(response.ok, "baseline failed: {:?}", response.error);
    response.result.expect("baseline result")
}

fn capabilities(state: &LocalHostState, project_id: &str) -> Value {
    let response = call(
        state,
        request(
            format!("REQ-cap-{project_id}"),
            "project.capabilities",
            json!({ "project_id": project_id }),
            None,
        ),
    );
    assert!(response.ok);
    response.result.expect("capability result")
}

fn reconcile(
    state: &LocalHostState,
    project_id: &str,
    hints: Vec<String>,
    request_id: &str,
    idempotency: &str,
) -> Value {
    let response = call(
        state,
        request(
            request_id,
            "project.index.reconcile",
            json!({ "project_id": project_id, "hints": hints }),
            Some(idempotency),
        ),
    );
    assert!(response.ok, "reconcile failed: {:?}", response.error);
    response.result.expect("reconcile result")
}

#[test]
fn two_projects_survive_restart_and_reconcile_changed_files_only() {
    let dir = unique_dir("two-projects");
    let root_a = dir.join("projects").join("alpha");
    let root_b = dir.join("projects").join("bravo");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();
    populate_project(&root_a, "alpha", 120);
    populate_project(&root_b, "bravo", 120);

    let state_dir = dir.join("state");
    fs::create_dir_all(&state_dir).unwrap();
    let (mut first, state, first_ready_ms) = spawn_host(&state_dir, "phase3-two-projects");

    let idle_zero = idle_sample(&first, 400);
    let storage_before_projects = storage_bytes(&state_dir);
    let imported_a = import_project(
        &state,
        "PRJ-alpha",
        "Alpha",
        &root_a,
        "REQ-import-alpha",
        "IDEM-import-alpha",
    );

    assert_eq!(imported_a["id"], "PRJ-alpha");
    assert_eq!(imported_a["baseline_state"], "missing");
    let import_text = serde_json::to_string(&imported_a).unwrap();
    assert!(!import_text.contains(&root_a.to_string_lossy().to_string()));

    let baseline_a = build_baseline(
        &state,
        "PRJ-alpha",
        "REQ-baseline-alpha",
        "IDEM-baseline-alpha",
    );
    assert_eq!(baseline_a["file_count"], 120);
    assert_eq!(baseline_a["files_hashed"], 120);
    assert_eq!(baseline_a["mode"], "baseline");
    let idle_one = idle_sample(&first, 400);

    let imported_b = import_project(
        &state,
        "PRJ-bravo",
        "Bravo",
        &root_b,
        "REQ-import-bravo",
        "IDEM-import-bravo",
    );
    assert_eq!(imported_b["id"], "PRJ-bravo");

    let baseline_b = build_baseline(
        &state,
        "PRJ-bravo",
        "REQ-baseline-bravo",
        "IDEM-baseline-bravo",
    );
    assert_eq!(baseline_b["file_count"], 120);
    assert_eq!(baseline_b["files_hashed"], 120);
    let listed = call(
        &state,
        request("REQ-phase3-list", "project.list", json!({}), None),
    );
    assert!(listed.ok);
    let listed_text = serde_json::to_string(&listed.result).unwrap();
    assert!(!listed_text.contains(&root_a.to_string_lossy().to_string()));
    assert!(!listed_text.contains(&root_b.to_string_lossy().to_string()));
    let idle_two = idle_sample(&first, 400);
    let storage_after_baselines = storage_bytes(&state_dir);

    let caps_a = capabilities(&state, "PRJ-alpha");
    let caps_b = capabilities(&state, "PRJ-bravo");
    assert_eq!(caps_a["baseline_state"], "ready");
    assert_eq!(caps_b["baseline_state"], "ready");
    assert!(caps_a["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| { item["id"] == "index.changed_only" && item["state"] == "available" }));

    shutdown(&state, &mut first);

    {
        let storage = RelayStorage::open(state_dir.join("relay.sqlite3")).unwrap();
        assert_eq!(storage.schema_version().unwrap(), 5);
        let alpha_files = storage.list_project_files("PRJ-alpha").unwrap();
        let bravo_files = storage.list_project_files("PRJ-bravo").unwrap();
        assert_eq!(alpha_files.len(), 120);
        assert_eq!(bravo_files.len(), 120);
        assert!(alpha_files.iter().all(|file| {
            !file.relative_path.contains(':') && !file.relative_path.contains('\\')
        }));
        assert!(bravo_files.iter().all(|file| {
            !file.relative_path.contains(':') && !file.relative_path.contains('\\')
        }));
        assert!(storage
            .list_project_changes("PRJ-bravo", 100)
            .unwrap()
            .is_empty());
    }

    let (mut second, state, graceful_ready_ms) = spawn_host(&state_dir, "phase3-two-projects");
    assert_eq!(capabilities(&state, "PRJ-alpha")["baseline_state"], "ready");

    assert_eq!(capabilities(&state, "PRJ-bravo")["baseline_state"], "ready");

    let hinted_path = fixture_relative(5);
    write_file(
        &root_a.join(&hinted_path),
        b"alpha changed through one watcher hint; length changed",
    );
    let changed_started = Instant::now();
    let changed = reconcile(
        &state,
        "PRJ-alpha",
        vec![hinted_path.clone()],
        "REQ-reconcile-hinted",
        "IDEM-reconcile-hinted",
    );
    let changed_wall_ms = changed_started.elapsed().as_millis() as u64;
    assert_eq!(changed["files_hashed"], 1);
    assert_eq!(changed["files_unchanged"], 119);
    assert_eq!(changed["hint_count"], 1);
    assert_eq!(changed["hint_hits"], 1);
    assert_eq!(changed["changes"].as_array().unwrap().len(), 1);
    assert_eq!(changed["changes"][0]["relative_path"], hinted_path);

    let missed_path = fixture_relative(17);
    write_file(
        &root_a.join(&missed_path),
        b"missed watcher event recovered by authoritative reconciliation",
    );
    let missed = reconcile(
        &state,
        "PRJ-alpha",
        Vec::new(),
        "REQ-reconcile-missed",
        "IDEM-reconcile-missed",
    );
    assert_eq!(missed["hint_count"], 0);
    assert_eq!(missed["files_hashed"], 1);
    assert_eq!(missed["changes"].as_array().unwrap().len(), 1);
    assert_eq!(missed["changes"][0]["relative_path"], missed_path);

    let rename_from = fixture_relative(21);
    let rename_to = "renamed/file-021.txt";
    fs::create_dir_all(root_a.join("renamed")).unwrap();
    fs::rename(root_a.join(&rename_from), root_a.join(rename_to)).unwrap();
    let delete_path = fixture_relative(22);
    fs::remove_file(root_a.join(&delete_path)).unwrap();
    let add_path = "new/deep-added.src";
    write_file(&root_a.join(add_path), b"new project content");

    let delta = reconcile(
        &state,
        "PRJ-alpha",
        Vec::new(),
        "REQ-reconcile-delta",
        "IDEM-reconcile-delta",
    );
    assert_eq!(delta["file_count"], 120);
    assert_eq!(delta["files_hashed"], 2);
    let changes = delta["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().any(|change| {
        change["change_kind"] == "renamed"
            && change["previous_path"] == rename_from
            && change["relative_path"] == rename_to
    }));
    assert!(changes.iter().any(|change| {
        change["change_kind"] == "deleted" && change["relative_path"] == delete_path
    }));
    assert!(changes
        .iter()
        .any(|change| { change["change_kind"] == "added" && change["relative_path"] == add_path }));

    second.kill().expect("hard kill relayd");
    second.wait().expect("wait hard-killed relayd");

    {
        let storage = RelayStorage::open(state_dir.join("relay.sqlite3")).unwrap();
        let alpha_files = storage.list_project_files("PRJ-alpha").unwrap();
        let bravo_files = storage.list_project_files("PRJ-bravo").unwrap();
        assert_eq!(alpha_files.len(), 120);
        assert_eq!(bravo_files.len(), 120);
        let alpha_changes = storage.list_project_changes("PRJ-alpha", 100).unwrap();
        let bravo_changes = storage.list_project_changes("PRJ-bravo", 100).unwrap();
        assert!(alpha_changes.len() >= 5);
        assert!(bravo_changes.is_empty());
        assert!(alpha_changes
            .iter()
            .all(|change| change.project_id == "PRJ-alpha"));
    }

    let (mut third, state, hard_ready_ms) = spawn_host(&state_dir, "phase3-two-projects");
    assert_eq!(capabilities(&state, "PRJ-alpha")["baseline_state"], "ready");
    assert_eq!(capabilities(&state, "PRJ-bravo")["baseline_state"], "ready");

    let clean = reconcile(
        &state,
        "PRJ-alpha",
        Vec::new(),
        "REQ-reconcile-after-hard-restart",
        "IDEM-reconcile-after-hard-restart",
    );
    assert_eq!(clean["files_hashed"], 0);
    assert_eq!(clean["changes"].as_array().unwrap().len(), 0);

    let rebuild = build_baseline(
        &state,
        "PRJ-alpha",
        "REQ-rebuild-alpha",
        "IDEM-rebuild-alpha",
    );
    assert_eq!(rebuild["file_count"], 120);
    assert_eq!(rebuild["files_hashed"], 120);
    shutdown(&state, &mut third);

    let storage_final = storage_bytes(&state_dir);
    assert!(idle_zero.rss_bytes < 128 * 1024 * 1024);
    assert!(idle_one.rss_bytes < 128 * 1024 * 1024);
    assert!(idle_two.rss_bytes < 128 * 1024 * 1024);
    assert!(idle_two.cpu_delta_ms < idle_two.sample_ms);
    assert!(changed_wall_ms < 2_000);

    println!(
        "PHASE3_FIRST_SLICE_METRICS={}",
        serde_json::to_string(&json!({
            "startup_ms": first_ready_ms,
            "graceful_restart_ready_ms": graceful_ready_ms,
            "hard_restart_ready_ms": hard_ready_ms,
            "baseline_alpha_ms": baseline_a["elapsed_ms"],
            "baseline_bravo_ms": baseline_b["elapsed_ms"],
            "changed_file_ms": changed["elapsed_ms"],
            "changed_file_wall_ms": changed_wall_ms,
            "missed_hint_reconcile_ms": missed["elapsed_ms"],
            "delta_reconcile_ms": delta["elapsed_ms"],
            "full_rebuild_ms": rebuild["elapsed_ms"],
            "idle_zero": {
                "rss_bytes": idle_zero.rss_bytes,
                "cpu_delta_ms": idle_zero.cpu_delta_ms,
                "sample_ms": idle_zero.sample_ms
            },
            "idle_one": {
                "rss_bytes": idle_one.rss_bytes,
                "cpu_delta_ms": idle_one.cpu_delta_ms,
                "sample_ms": idle_one.sample_ms
            },
            "idle_two": {
                "rss_bytes": idle_two.rss_bytes,
                "cpu_delta_ms": idle_two.cpu_delta_ms,
                "sample_ms": idle_two.sample_ms
            },
            "storage_before_projects_bytes": storage_before_projects,
            "storage_after_baselines_bytes": storage_after_baselines,
            "storage_growth_through_two_baselines_bytes":
                storage_after_baselines.saturating_sub(storage_before_projects),
            "storage_final_bytes": storage_final
        }))
        .unwrap()
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_roots_and_hints_fail_without_partial_index_state() {
    let dir = unique_dir("invalid-paths");
    let root = dir.join("project");
    fs::create_dir_all(&root).unwrap();
    populate_project(&root, "valid", 6);
    let state_dir = dir.join("state");
    fs::create_dir_all(&state_dir).unwrap();

    let (mut child, state, _) = spawn_host(&state_dir, "phase3-invalid-paths");

    let missing_root = dir.join("missing-project");
    let bad_import = call(
        &state,
        request(
            "REQ-import-missing",
            "project.import",
            json!({
                "id": "PRJ-missing",
                "name": "Missing",
                "root_path": missing_root.to_string_lossy()
            }),
            Some("IDEM-import-missing"),
        ),
    );
    assert!(!bad_import.ok);
    assert_eq!(
        bad_import.error.as_ref().unwrap().code,
        "PROJECT_PATH_INVALID"
    );

    import_project(
        &state,
        "PRJ-valid",
        "Valid",
        &root,
        "REQ-import-valid",
        "IDEM-import-valid",
    );

    let before_baseline = call(
        &state,
        request(
            "REQ-reconcile-before-baseline",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-valid", "hints": [] }),
            Some("IDEM-reconcile-before-baseline"),
        ),
    );
    assert!(!before_baseline.ok);
    assert_eq!(
        before_baseline.error.as_ref().unwrap().code,
        "INDEX_BASELINE_MISSING"
    );

    let baseline = build_baseline(
        &state,
        "PRJ-valid",
        "REQ-baseline-valid",
        "IDEM-baseline-valid",
    );
    assert_eq!(baseline["generation"], 1);

    let bad_hint = call(
        &state,
        request(
            "REQ-bad-hint",
            "project.index.reconcile",
            json!({
                "project_id": "PRJ-valid",
                "hints": ["../outside.txt"]
            }),
            Some("IDEM-bad-hint"),
        ),
    );
    assert!(!bad_hint.ok);
    assert_eq!(bad_hint.error.as_ref().unwrap().code, "PROJECT_PATH_ESCAPE");

    let vanishing_root = dir.join("vanishing");
    fs::create_dir_all(&vanishing_root).unwrap();
    write_file(&vanishing_root.join("one.txt"), b"temporary");
    import_project(
        &state,
        "PRJ-vanishing",
        "Vanishing",
        &vanishing_root,
        "REQ-import-vanishing",
        "IDEM-import-vanishing",
    );

    fs::remove_dir_all(&vanishing_root).unwrap();

    let vanished_caps = capabilities(&state, "PRJ-vanishing");
    assert_eq!(vanished_caps["baseline_state"], "unavailable");
    assert!(vanished_caps["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| { item["id"] == "index.changed_only" && item["state"] == "unavailable" }));

    let vanished = call(
        &state,
        request(
            "REQ-baseline-vanished",
            "project.index.build",
            json!({ "project_id": "PRJ-vanishing" }),
            Some("IDEM-baseline-vanished"),
        ),
    );
    assert!(!vanished.ok);
    assert_eq!(
        vanished.error.as_ref().unwrap().code,
        "PROJECT_PATH_INVALID"
    );

    let locked_root = dir.join("locked");
    fs::create_dir_all(&locked_root).unwrap();
    let locked_path = locked_root.join("locked.txt");
    write_file(&locked_path, b"locked content");
    import_project(
        &state,
        "PRJ-locked",
        "Locked",
        &locked_root,
        "REQ-import-locked",
        "IDEM-import-locked",
    );
    let locked_handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&locked_path)
        .unwrap();
    let locked_build = call(
        &state,
        request(
            "REQ-baseline-locked",
            "project.index.build",
            json!({ "project_id": "PRJ-locked" }),
            Some("IDEM-baseline-locked"),
        ),
    );
    assert!(!locked_build.ok);
    assert_eq!(
        locked_build.error.as_ref().unwrap().code,
        "PROJECT_FILE_UNREADABLE"
    );
    drop(locked_handle);

    shutdown(&state, &mut child);

    let storage = RelayStorage::open(state_dir.join("relay.sqlite3")).unwrap();
    assert!(storage.get_project("PRJ-missing").unwrap().is_none());
    let state = storage
        .get_project_index_state("PRJ-valid")
        .unwrap()
        .unwrap();
    assert_eq!(state.generation, 1);
    assert_eq!(state.file_count, 6);
    assert!(storage
        .list_project_changes("PRJ-valid", 100)
        .unwrap()
        .is_empty());
    assert!(storage
        .get_project_index_state("PRJ-vanishing")
        .unwrap()
        .is_none());
    assert!(storage
        .get_project_index_state("PRJ-locked")
        .unwrap()
        .is_none());

    drop(storage);
    fs::remove_dir_all(dir).unwrap();
}
