#![cfg(windows)]

use relay::client;
use relay_contracts::{CommandRequest, CommandResponse, LocalHostState, RequestContext};
use serde_json::{Value, json};
use std::fs;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{FILETIME, HANDLE};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetProcessTimes;

struct TestHost(Child);

impl Drop for TestHost {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
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

fn fixture_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "relay-phase3-watcher-{}-{nanos}",
        std::process::id()
    ))
}

fn spawn_host(state_dir: &Path) -> (TestHost, LocalHostState) {
    let instance = state_dir
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy();
    let child = TestHost(
        Command::new(env!("CARGO_BIN_EXE_relayd"))
            .env("RELAY_STATE_DIR", state_dir)
            .env("RELAY_INSTANCE", instance.as_ref())
            .env_remove("RELAY_TEST_DISABLE_WATCHER")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(state_dir.join("host.json"))
            && let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes)
            && state.pid == child.0.id()
        {
            return (child, state);
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("watcher host did not become ready");
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

fn call(state: &LocalHostState, request: CommandRequest) -> CommandResponse {
    client::call(state, &request).unwrap()
}

#[test]
fn os_notifications_apply_hints_and_restart_marks_continuity_uncertain() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root = dir.join("project");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("one.txt"), b"baseline").unwrap();
    let (mut first, state) = spawn_host(&state_dir);
    let imported = call(
        &state,
        request(
            "watch-import",
            "project.import",
            json!({ "id": "PRJ-watch", "name": "Watcher fixture", "root_path": root.to_string_lossy() }),
            true,
        ),
    );
    assert!(imported.ok, "{:?}", imported.error);
    let built = call(
        &state,
        request(
            "watch-build",
            "project.index.build",
            json!({ "project_id": "PRJ-watch" }),
            true,
        ),
    );
    assert!(built.ok, "{:?}", built.error);
    thread::sleep(Duration::from_millis(500));
    let attached = call(
        &state,
        request(
            "watch-attached-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-watch" }),
            true,
        ),
    );
    assert!(attached.ok, "{:?}", attached.error);
    let second_import = call(
        &state,
        request(
            "watch-second-import",
            "project.import",
            json!({ "id": "PRJ-watch-second", "name": "Shared-root fixture", "root_path": root.to_string_lossy() }),
            true,
        ),
    );
    assert!(second_import.ok, "{:?}", second_import.error);
    let second_build = call(
        &state,
        request(
            "watch-second-build",
            "project.index.build",
            json!({ "project_id": "PRJ-watch-second" }),
            true,
        ),
    );
    assert!(second_build.ok, "{:?}", second_build.error);
    thread::sleep(Duration::from_millis(200));
    let second_attached = call(
        &state,
        request(
            "watch-second-attached",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-watch-second" }),
            true,
        ),
    );
    assert!(second_attached.ok, "{:?}", second_attached.error);
    let (_, cpu_before) = process_sample(&first.0);
    thread::sleep(Duration::from_millis(1_000));
    let (rss_bytes, cpu_after) = process_sample(&first.0);
    let idle_cpu_ms = cpu_after.saturating_sub(cpu_before);
    assert!(rss_bytes < 128 * 1024 * 1024);
    assert!(idle_cpu_ms < 1_000);
    println!(
        "PHASE3_WATCHER_IDLE_METRICS={}",
        json!({
            "watched_projects": 2,
            "distinct_roots": 1,
            "rss_bytes": rss_bytes,
            "cpu_delta_ms": idle_cpu_ms,
            "sample_ms": 1_000
        })
    );
    fs::write(root.join("two.txt"), b"from OS notification").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stale = false;
    while Instant::now() < deadline {
        let first_caps = call(
            &state,
            request(
                "watch-caps",
                "project.capabilities",
                json!({ "project_id": "PRJ-watch" }),
                false,
            ),
        );
        let second_caps = call(
            &state,
            request(
                "watch-second-caps",
                "project.capabilities",
                json!({ "project_id": "PRJ-watch-second" }),
                false,
            ),
        );
        if first_caps.result.unwrap()["index_status"] == "stale"
            && second_caps.result.unwrap()["index_status"] == "stale"
        {
            stale = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(stale, "OS event did not mark the project index stale");
    let reconciled = call(
        &state,
        request(
            "watch-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-watch" }),
            true,
        ),
    );
    assert!(reconciled.ok, "{:?}", reconciled.error);
    assert_eq!(
        reconciled.result.unwrap()["changes"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let second_reconciled = call(
        &state,
        request(
            "watch-second-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-watch-second" }),
            true,
        ),
    );
    assert!(second_reconciled.ok, "{:?}", second_reconciled.error);
    assert_eq!(
        second_reconciled.result.unwrap()["changes"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    fs::create_dir_all(root.join("new-directory")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut directory_stale = false;
    while Instant::now() < deadline {
        let caps = call(
            &state,
            request(
                "watch-directory-caps",
                "project.capabilities",
                json!({ "project_id": "PRJ-watch" }),
                false,
            ),
        );
        if caps.result.unwrap()["index_status"] == "stale" {
            directory_stale = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        directory_stale,
        "directory notification did not signal uncertain continuity"
    );

    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let (mut second, state) = spawn_host(&state_dir);
    let caps = call(
        &state,
        request(
            "watch-restart-caps",
            "project.capabilities",
            json!({ "project_id": "PRJ-watch" }),
            false,
        ),
    );
    assert_eq!(caps.result.unwrap()["index_status"], "stale");
    let second_caps = call(
        &state,
        request(
            "watch-second-restart-caps",
            "project.capabilities",
            json!({ "project_id": "PRJ-watch-second" }),
            false,
        ),
    );
    assert_eq!(second_caps.result.unwrap()["index_status"], "stale");
    let recovered = call(
        &state,
        request(
            "watch-restart-verify",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-watch", "verify_content": true }),
            true,
        ),
    );
    assert!(recovered.ok, "{:?}", recovered.error);
    assert_eq!(recovered.result.unwrap()["files_hashed"], 2);
    let stopped = call(
        &state,
        request("watch-stop", "system.shutdown", json!({}), false),
    );
    assert!(stopped.ok);
    assert!(second.0.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn os_notifications_remain_project_scoped_across_distinct_roots() {
    let dir = fixture_dir();
    let state_dir = dir.join("state");
    let root_a = dir.join("alpha");
    let root_b = dir.join("bravo");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();
    fs::write(root_a.join("base.txt"), b"alpha").unwrap();
    fs::write(root_b.join("base.txt"), b"bravo").unwrap();
    let (mut host, state) = spawn_host(&state_dir);
    for (project_id, root) in [("PRJ-alpha", &root_a), ("PRJ-bravo", &root_b)] {
        let imported = call(
            &state,
            request(
                &format!("import-{project_id}"),
                "project.import",
                json!({ "id": project_id, "name": project_id, "root_path": root.to_string_lossy() }),
                true,
            ),
        );
        assert!(imported.ok, "{:?}", imported.error);
        let built = call(
            &state,
            request(
                &format!("build-{project_id}"),
                "project.index.build",
                json!({ "project_id": project_id }),
                true,
            ),
        );
        assert!(built.ok, "{:?}", built.error);
    }
    thread::sleep(Duration::from_millis(500));
    for project_id in ["PRJ-alpha", "PRJ-bravo"] {
        let reconciled = call(
            &state,
            request(
                &format!("attach-{project_id}"),
                "project.index.reconcile",
                json!({ "project_id": project_id }),
                true,
            ),
        );
        assert!(reconciled.ok, "{:?}", reconciled.error);
    }
    fs::write(root_a.join("new.txt"), b"only alpha").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut alpha_stale = false;
    while Instant::now() < deadline {
        let caps = call(
            &state,
            request(
                "scope-alpha-caps",
                "project.capabilities",
                json!({ "project_id": "PRJ-alpha" }),
                false,
            ),
        );
        if caps.result.unwrap()["index_status"] == "stale" {
            alpha_stale = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(alpha_stale, "alpha OS event was not observed");
    let bravo = call(
        &state,
        request(
            "scope-bravo-caps",
            "project.capabilities",
            json!({ "project_id": "PRJ-bravo" }),
            false,
        ),
    );
    assert_eq!(bravo.result.unwrap()["index_status"], "ready");
    let alpha = call(
        &state,
        request(
            "scope-alpha-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-alpha" }),
            true,
        ),
    );
    assert!(alpha.ok, "{:?}", alpha.error);
    assert!(
        alpha.result.unwrap()["changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let stopped = call(
        &state,
        request("scope-stop", "system.shutdown", json!({}), false),
    );
    assert!(stopped.ok);
    assert!(host.0.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}
