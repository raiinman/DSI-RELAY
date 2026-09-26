#![cfg(windows)]

use relay::client;
use relay_contracts::{CommandRequest, CommandResponse, LocalHostState, RequestContext};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{FILETIME, HANDLE};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetProcessTimes;

struct Fixture {
    dir: PathBuf,
    host: Child,
    state: LocalHostState,
}

impl Fixture {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "relay-phase3-watcher-load-{}-{nanos}",
            std::process::id()
        ));
        let state_dir = dir.join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let host = Command::new(env!("CARGO_BIN_EXE_relayd"))
            .env("RELAY_STATE_DIR", &state_dir)
            .env("RELAY_INSTANCE", dir.file_name().unwrap())
            .env("RELAY_TEST_RECOVERY_IDLE_MS", "1")
            .env_remove("RELAY_TEST_DISABLE_WATCHER")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let state = loop {
            if let Ok(bytes) = fs::read(state_dir.join("host.json"))
                && let Ok(state) = serde_json::from_slice::<LocalHostState>(&bytes)
                && state.pid == host.id()
            {
                break state;
            }
            assert!(
                Instant::now() < deadline,
                "watcher host did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        };
        Self { dir, host, state }
    }

    fn call(&self, id: &str, command: &str, arguments: Value, keyed: bool) -> CommandResponse {
        let request = CommandRequest {
            request_id: id.to_string(),
            command: command.to_string(),
            command_version: Some(1),
            arguments,
            idempotency_key: keyed.then(|| format!("IDEMP-{id}")),
            context: RequestContext::default(),
        };
        client::call(&self.state, &request).unwrap()
    }

    fn checked(&self, id: &str, command: &str, arguments: Value, keyed: bool) -> Value {
        let response = self.call(id, command, arguments, keyed);
        assert!(response.ok, "{command}: {:?}", response.error);
        response.result.unwrap()
    }

    fn root(&self, name: &str) -> PathBuf {
        let root = self.dir.join(name);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn add_project(&self, id: &str, root: &Path) {
        self.checked(
            &format!("load-import-{id}"),
            "project.import",
            json!({ "id": id, "name": id, "root_path": root.to_string_lossy() }),
            true,
        );
        self.checked(
            &format!("load-build-{id}"),
            "project.index.build",
            json!({ "project_id": id }),
            true,
        );
        // Give the watcher time to attach, then clear its initial continuity marker
        // explicitly so the automatic recovery retry backoff has not started.
        thread::sleep(Duration::from_millis(500));
        self.checked(
            &format!("load-attach-{id}"),
            "project.index.reconcile",
            json!({ "project_id": id, "verify_content": true }),
            true,
        );
        assert_eq!(self.capabilities(id)["index_status"], "ready");
    }

    fn capabilities(&self, id: &str) -> Value {
        self.checked(
            &format!("load-capabilities-{id}"),
            "project.capabilities",
            json!({ "project_id": id }),
            false,
        )
    }

    fn wait_ready(&self, id: &str, timeout: Duration) -> Duration {
        let started = Instant::now();
        while started.elapsed() < timeout {
            // Read-only capability observations must not cancel background recovery.
            let caps = self.capabilities(id);
            if caps["index_status"] == "ready" && caps["content_verification_required"] == false {
                return started.elapsed();
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("{id} did not recover within {timeout:?}");
    }

    fn generation(&self, id: &str) -> i64 {
        let storage =
            relay_core::storage::RelayStorage::open(self.dir.join("state/relay.sqlite3")).unwrap();
        storage
            .get_project_index_state(id)
            .unwrap()
            .unwrap()
            .generation
    }

    fn stop(mut self) {
        self.checked("load-stop", "system.shutdown", json!({}), false);
        assert!(self.host.wait().unwrap().success());
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.host.try_wait().ok().flatten().is_none() {
            let _ = self.host.kill();
            let _ = self.host.wait();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn process_sample(host: &Child) -> (u64, u64) {
    let handle = host.as_raw_handle() as HANDLE;
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

#[test]
fn four_distinct_watched_roots_are_quiet_when_inactive() {
    let fixture = Fixture::new();
    let mut generations = Vec::new();
    for index in 0..4 {
        let id = format!("PRJ-idle-{index}");
        let root = fixture.root(&format!("idle-{index}"));
        fs::write(root.join("base.txt"), format!("project-{index}")).unwrap();
        fixture.add_project(&id, &root);
        generations.push((id.clone(), fixture.generation(&id)));
    }

    let sample_ms = 5_000_u64;
    let (rss_before, cpu_before) = process_sample(&fixture.host);
    thread::sleep(Duration::from_millis(sample_ms));
    let (rss_after, cpu_after) = process_sample(&fixture.host);
    for (id, generation) in &generations {
        assert_eq!(fixture.capabilities(id)["index_status"], "ready");
        assert_eq!(fixture.generation(id), *generation);
    }
    println!(
        "PHASE3_WATCHER_MULTI_ROOT_IDLE_METRICS={}",
        json!({
            "watched_projects": 4,
            "distinct_roots": 4,
            "sample_ms": sample_ms,
            "rss_before_bytes": rss_before,
            "rss_after_bytes": rss_after,
            "cpu_delta_ms": cpu_after.saturating_sub(cpu_before),
            "generations_unchanged": true
        })
    );
    fixture.stop();
}

#[test]
fn live_burst_uncertainty_recovers_without_cross_project_changes() {
    const BURST_FILES: usize = 96;
    let fixture = Fixture::new();
    let alpha = fixture.root("alpha");
    let bravo = fixture.root("bravo");
    fs::write(alpha.join("base.txt"), b"alpha").unwrap();
    fs::write(bravo.join("base.txt"), b"bravo").unwrap();
    fixture.add_project("PRJ-load-alpha", &alpha);
    fixture.add_project("PRJ-load-bravo", &bravo);
    let alpha_generation = fixture.generation("PRJ-load-alpha");
    let bravo_generation = fixture.generation("PRJ-load-bravo");

    let started = Instant::now();
    let burst_dir = alpha.join("burst");
    fs::create_dir(&burst_dir).unwrap();
    for index in 0..BURST_FILES {
        fs::write(
            burst_dir.join(format!("file-{index:03}.txt")),
            format!("burst-{index:03}"),
        )
        .unwrap();
    }
    let write_ms = started.elapsed().as_millis();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let caps = fixture.capabilities("PRJ-load-alpha");
        if caps["index_status"] == "stale" && caps["content_verification_required"] == true {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "directory uncertainty was not observed"
        );
        thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        fixture.capabilities("PRJ-load-bravo")["index_status"],
        "ready"
    );
    assert_eq!(fixture.generation("PRJ-load-bravo"), bravo_generation);
    let metadata_only = fixture.call(
        "load-metadata-rejected",
        "project.index.reconcile",
        json!({ "project_id": "PRJ-load-alpha" }),
        true,
    );
    assert_eq!(
        metadata_only.error.unwrap().code,
        "INDEX_CONTENT_VERIFICATION_REQUIRED"
    );

    let recovery_ms = fixture
        .wait_ready("PRJ-load-alpha", Duration::from_secs(40))
        .as_millis();
    assert_eq!(
        fixture.capabilities("PRJ-load-alpha")["index_status"],
        "ready"
    );
    let changes = fixture.checked(
        "load-burst-changes",
        "project.changes",
        json!({ "project_id": "PRJ-load-alpha", "after_generation": alpha_generation, "limit": 100 }),
        false,
    );
    let paths: BTreeSet<String> = changes["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["relative_path"].as_str().unwrap().to_string())
        .collect();
    let expected: BTreeSet<String> = (0..BURST_FILES)
        .map(|index| format!("burst/file-{index:03}.txt"))
        .collect();
    assert_eq!(paths, expected);
    assert_eq!(
        fixture.capabilities("PRJ-load-bravo")["index_status"],
        "ready"
    );
    assert_eq!(fixture.generation("PRJ-load-bravo"), bravo_generation);
    println!(
        "PHASE3_WATCHER_BURST_METRICS={}",
        json!({
            "burst_files": BURST_FILES,
            "burst_write_wall_ms": write_ms,
            "recovery_wait_after_stale_ms": recovery_ms,
            "alpha_change_count": paths.len(),
            "bravo_generation_unchanged": true
        })
    );
    fixture.stop();
}

#[test]
#[ignore = "manual one-minute live watcher soak; run with --ignored --nocapture"]
fn live_watcher_soak_recovers_after_prolonged_event_activity() {
    let fixture = Fixture::new();
    let root = fixture.root("soak");
    fs::write(root.join("base.txt"), b"baseline").unwrap();
    fixture.add_project("PRJ-load-soak", &root);
    let baseline_generation = fixture.generation("PRJ-load-soak");
    let (rss_before, cpu_before) = process_sample(&fixture.host);
    let duration = Duration::from_secs(60);
    let started = Instant::now();
    let mut writes = 0_u64;
    while started.elapsed() < duration {
        let path = root.join(format!("active-{:02}.txt", writes % 16));
        fs::write(path, format!("revision-{writes:08}")).unwrap();
        writes += 1;
        thread::sleep(Duration::from_millis(100));
    }
    let active_ms = started.elapsed().as_millis();
    let recovery_ms = fixture
        .wait_ready("PRJ-load-soak", Duration::from_secs(40))
        .as_millis();
    assert_eq!(
        fixture.capabilities("PRJ-load-soak")["index_status"],
        "ready"
    );
    let (rss_after, cpu_after) = process_sample(&fixture.host);
    assert!(fixture.generation("PRJ-load-soak") > baseline_generation);
    let storage =
        relay_core::storage::RelayStorage::open(fixture.dir.join("state/relay.sqlite3")).unwrap();
    let paths: BTreeSet<String> = storage
        .list_project_files("PRJ-load-soak")
        .unwrap()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    for index in 0..16 {
        assert!(paths.contains(&format!("active-{index:02}.txt")));
    }
    drop(storage);
    println!(
        "PHASE3_WATCHER_SOAK_METRICS={}",
        json!({
            "active_ms": active_ms,
            "writes": writes,
            "distinct_files": 16,
            "recovery_wait_after_activity_ms": recovery_ms,
            "rss_before_bytes": rss_before,
            "rss_after_bytes": rss_after,
            "cpu_delta_ms_including_recovery": cpu_after.saturating_sub(cpu_before)
        })
    );
    fixture.stop();
}

#[test]
#[ignore = "manual 1 GiB recovery race fixture; run separately from resource benchmarks"]
fn callback_during_full_recovery_keeps_verification_required() {
    let fixture = Fixture::new();
    let root = fixture.root("race");
    let target = root.join("000-target.txt");
    fs::write(&target, b"before").unwrap();
    let block = vec![b'p'; 64 * 1024 * 1024];
    for index in 0..16 {
        fs::write(root.join(format!("100-padding-{index:02}.bin")), &block).unwrap();
    }
    fixture.add_project("PRJ-load-race", &root);
    let generation = fixture.generation("PRJ-load-race");
    let original_modified = fs::metadata(&target).unwrap().modified().unwrap();

    // A folder notification requires full verification. The daemon is then
    // deliberately given enough content to keep its scan active while a second
    // callback arrives for the first file in lexical order.
    let uncertain_at = Instant::now();
    fs::create_dir(root.join("uncertain")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.capabilities("PRJ-load-race")["content_verification_required"] != true {
        assert!(
            Instant::now() < deadline,
            "uncertain folder event was not observed"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let (_, cpu_before) = process_sample(&fixture.host);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (_, cpu_now) = process_sample(&fixture.host);
        if cpu_now.saturating_sub(cpu_before) >= 20 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "full recovery did not start hashing"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let generation_before_edit = fixture.generation("PRJ-load-race");
    let status_before_edit = fixture.capabilities("PRJ-load-race")["index_status"].clone();
    println!(
        "PHASE3_WATCHER_RACE_PRE_EDIT={}",
        json!({
            "elapsed_since_uncertainty_ms": uncertain_at.elapsed().as_millis(),
            "generation": generation_before_edit,
            "status": status_before_edit
        })
    );
    assert_eq!(
        generation_before_edit, generation,
        "scan completed before test edit"
    );
    assert_eq!(
        status_before_edit, "stale",
        "scan completed before test edit"
    );
    fs::write(&target, b"after!").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(original_modified))
        .unwrap();

    let observation_started = Instant::now();
    while observation_started.elapsed() < Duration::from_secs(3) {
        let caps = fixture.capabilities("PRJ-load-race");
        assert_eq!(caps["index_status"], "stale");
        assert_eq!(caps["content_verification_required"], true);
        thread::sleep(Duration::from_millis(25));
    }
    let generation_after_edit = fixture.generation("PRJ-load-race");
    println!(
        "PHASE3_WATCHER_RACE_AFTER_EDIT={}",
        json!({
            "elapsed_since_uncertainty_ms": uncertain_at.elapsed().as_millis(),
            "generation": generation_after_edit,
            "status": fixture.capabilities("PRJ-load-race")["index_status"]
        })
    );
    assert_eq!(
        fixture.capabilities("PRJ-load-race")["index_status"],
        "stale"
    );
    let verified = fixture.checked(
        "load-race-verify",
        "project.index.reconcile",
        json!({ "project_id": "PRJ-load-race", "verify_content": true }),
        true,
    );
    assert_eq!(verified["files_hashed"], 17);
    let changes = fixture.checked(
        "load-race-changes",
        "project.changes",
        json!({ "project_id": "PRJ-load-race", "after_generation": generation }),
        false,
    );
    assert_eq!(changes["changes"].as_array().unwrap().len(), 1);
    assert_eq!(changes["changes"][0]["relative_path"], "000-target.txt");
    println!(
        "PHASE3_WATCHER_RACE_METRICS={}",
        json!({
            "padding_files": 16,
            "padding_bytes": 16_u64 * 64 * 1024 * 1024,
            "generation_before_edit": generation_before_edit,
            "generation_after_callback_hints": generation_after_edit,
            "verification_required_during_observation": true,
            "verified_files_hashed": verified["files_hashed"]
        })
    );
    fixture.stop();
}

#[test]
#[ignore = "manual foreground-write interruption and 30-second recovery retry"]
fn foreground_write_interrupts_scan_and_idle_retry_restores_ready() {
    let fixture = Fixture::new();
    let recovery_root = fixture.root("recovery");
    let foreground_root = fixture.root("foreground");
    fs::write(recovery_root.join("base.txt"), b"baseline").unwrap();
    let block = vec![b'r'; 64 * 1024 * 1024];
    for index in 0..4 {
        fs::write(
            recovery_root.join(format!("padding-{index:02}.bin")),
            &block,
        )
        .unwrap();
    }
    for index in 0..2 {
        fs::write(
            foreground_root.join(format!("padding-{index:02}.bin")),
            &block,
        )
        .unwrap();
    }
    fixture.add_project("PRJ-foreground-recovery", &recovery_root);
    fixture.add_project("PRJ-foreground-work", &foreground_root);
    let generation = fixture.generation("PRJ-foreground-recovery");
    let started = Instant::now();
    fs::create_dir(recovery_root.join("uncertain")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.capabilities("PRJ-foreground-recovery")["content_verification_required"] != true {
        assert!(
            Instant::now() < deadline,
            "uncertain event was not observed"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let (_, cpu_before) = process_sample(&fixture.host);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (_, cpu_now) = process_sample(&fixture.host);
        if cpu_now.saturating_sub(cpu_before) >= 100 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background recovery did not start hashing"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let scan_observed_ms = started.elapsed().as_millis();
    assert_eq!(fixture.generation("PRJ-foreground-recovery"), generation);
    let foreground_start = Instant::now();
    let foreground = fixture.checked(
        "foreground-verify",
        "project.index.reconcile",
        json!({ "project_id": "PRJ-foreground-work", "verify_content": true }),
        true,
    );
    let foreground_ms = foreground_start.elapsed().as_millis();
    assert_eq!(foreground["file_count"], 2);
    let after_write = fixture.capabilities("PRJ-foreground-recovery");
    assert_eq!(after_write["index_status"], "stale");
    assert_eq!(after_write["content_verification_required"], true);
    assert_eq!(fixture.generation("PRJ-foreground-recovery"), generation);
    let retry_ms = fixture
        .wait_ready("PRJ-foreground-recovery", Duration::from_secs(45))
        .as_millis();
    assert_eq!(
        fixture.capabilities("PRJ-foreground-recovery")["index_status"],
        "ready"
    );
    println!(
        "PHASE3_WATCHER_FOREGROUND_RETRY_METRICS={}",
        json!({
            "recovery_fixture_bytes": 4_u64 * 64 * 1024 * 1024,
            "foreground_fixture_bytes": 2_u64 * 64 * 1024 * 1024,
            "scan_observed_after_uncertainty_ms": scan_observed_ms,
            "foreground_write_ms": foreground_ms,
            "stale_after_foreground_write": true,
            "retry_wait_after_foreground_ms": retry_ms,
            "total_since_uncertainty_ms": started.elapsed().as_millis()
        })
    );
    fixture.stop();
}
