use relay_rust_challenger::diagnostics::{
    CompletenessMetadata, DiagnosticConfig, DiagnosticEvent, DiagnosticSource,
    JsonlDiagnostics, Severity,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Diagnostics::Etw::{
    EventProviderEnabled, EventRegister, EventUnregister, EventWriteString, REGHANDLE,
};

const ETW_PROVIDER: GUID =
    GUID::from_u128(0x9a421fa8_f21b_4f9e_99ab_1db6bb8b1470);

fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let index = (((p / 100.0) * sorted.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted[index]
}

fn summary(values: &[f64]) -> Value {
    if values.is_empty() {
        return json!({ "n": 0 });
    }
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

fn synthetic_event(index: usize, payload_bytes: usize) -> DiagnosticEvent {
    let mut event = DiagnosticEvent::new(
        "relay.fixture.operation",
        Severity::Info,
        "fixture.component",
        "Synthetic diagnostic operation completed",
    );
    event.refs.project_id = Some("PRJ-fixture".to_string());
    event.refs.job_id = Some(format!("JOB-{:04}", index % 128));
    event.refs.result_id = Some(format!("RES-{:04}", index % 64));
    event.correlation_id = Some(format!("CORR-{index:08}"));
    event.causation_id = if index == 0 {
        None
    } else {
        Some(format!("CORR-{:08}", index - 1))
    };
    event.source = DiagnosticSource::trusted_local("spike14-probe");
    event.completeness = CompletenessMetadata::default();
    event.attributes.insert("sequence".to_string(), json!(index));
    event
        .attributes
        .insert("operation".to_string(), json!("synthetic"));
    event.attributes.insert(
        "payload".to_string(),
        json!("x".repeat(payload_bytes)),
    );
    event
}

fn directory_bytes(directory: &Path) -> u64 {
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}


fn bench_jsonl(directory: &Path, events: usize, sync_every: u64) -> Value {
    let mut config = DiagnosticConfig::new(directory);
    config.max_file_bytes = 1024 * 1024;
    config.max_files = 4;
    config.sync_every = sync_every;
    let mut diagnostics = JsonlDiagnostics::open(config).expect("open JSONL diagnostics");

    let mut latencies = Vec::with_capacity(events);
    let mut sync_latencies = Vec::new();
    let mut rotation_latencies = Vec::new();
    let mut logical_bytes = 0u64;
    let started = Instant::now();
    for index in 0..events {
        let event = synthetic_event(index, 192);
        let before = Instant::now();
        let outcome = diagnostics.append(event).expect("append JSONL event");
        let elapsed = before.elapsed().as_secs_f64() * 1000.0;
        latencies.push(elapsed);
        if outcome.synced {
            sync_latencies.push(elapsed);
        }
        if outcome.rotated {
            rotation_latencies.push(elapsed);
        }
        logical_bytes += outcome.bytes_written as u64;
    }
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    let flush_started = Instant::now();
    diagnostics.flush().expect("flush JSONL");
    let flush_ms = flush_started.elapsed().as_secs_f64() * 1000.0;

    let aggregate_started = Instant::now();
    let aggregate = diagnostics.aggregate().expect("aggregate JSONL");
    let aggregate_ms = aggregate_started.elapsed().as_secs_f64() * 1000.0;
    let health = diagnostics.health();
    let retained_bytes = directory_bytes(directory);

    json!({
        "candidate": "jsonl",
        "sync_every": sync_every,
        "events_input": events,
        "write_latency": summary(&latencies),
        "total_write_ms": total_ms,
        "events_per_second": events as f64 / (total_ms / 1000.0),
        "durability_window_events": sync_every,
        "durability_sync_latency": summary(&sync_latencies),
        "logical_bytes_written": logical_bytes,
        "retained_bytes": retained_bytes,
        "logical_bytes_per_input_event": logical_bytes as f64 / events as f64,
        "retained_bytes_per_input_event": retained_bytes as f64 / events as f64,
        "explicit_flush_ms": flush_ms,
        "rotation": {
            "count": rotation_latencies.len(),
            "latency": summary(&rotation_latencies)
        },
        "aggregate_ms": aggregate_ms,
        "aggregate": aggregate,
        "health": health
    })
}

fn open_sqlite(path: &Path) -> Connection {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let connection = Connection::open(path).expect("open SQLite diagnostic candidate");
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA busy_timeout=3000;
             CREATE TABLE IF NOT EXISTS diagnostic_events (
               seq INTEGER PRIMARY KEY,
               event_json TEXT NOT NULL,
               severity TEXT NOT NULL,
               component TEXT NOT NULL,
               event_id TEXT NOT NULL
             );",
        )
        .expect("configure SQLite diagnostic candidate");
    connection
}


fn bench_sqlite(path: &Path, events: usize) -> Value {
    let connection = open_sqlite(path);
    let mut latencies = Vec::with_capacity(events);
    let mut sync_latencies = Vec::new();
    let mut logical_bytes = 0u64;
    let durability_window = 16usize;
    if events > 0 {
        connection
            .execute_batch("BEGIN IMMEDIATE;")
            .expect("begin SQLite durability window");
    }
    let started = Instant::now();

    for index in 0..events {
        let event = synthetic_event(index, 192);
        let encoded = serde_json::to_string(&event).expect("serialize fixture event");
        logical_bytes += encoded.len() as u64;
        let before = Instant::now();
        connection
            .execute(
                "INSERT INTO diagnostic_events(
                    seq, event_json, severity, component, event_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    index as i64,
                    encoded,
                    "info",
                    "fixture.component",
                    "relay.fixture.operation"
                ],
            )
            .expect("insert SQLite diagnostic event");
        let boundary =
            (index + 1) % durability_window == 0 || index + 1 == events;
        if boundary {
            let sync_started = Instant::now();
            connection
                .execute_batch("COMMIT;")
                .expect("commit SQLite durability window");
            sync_latencies.push(sync_started.elapsed().as_secs_f64() * 1000.0);
            if index + 1 < events {
                connection
                    .execute_batch("BEGIN IMMEDIATE;")
                    .expect("begin next SQLite durability window");
            }
        }
        latencies.push(before.elapsed().as_secs_f64() * 1000.0);
    }
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    let bytes_after_writes = file_family_bytes(path);
    let retention_target_rows = events.min(4096);
    let cutoff = events.saturating_sub(retention_target_rows);
    let prune_started = Instant::now();
    if cutoff > 0 {
        connection
            .execute(
                "DELETE FROM diagnostic_events WHERE seq < ?1",
                params![cutoff as i64],
            )
            .expect("prune diagnostic rows");
    }
    let prune_ms = prune_started.elapsed().as_secs_f64() * 1000.0;

    let checkpoint_started = Instant::now();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint diagnostic WAL");
    let checkpoint_ms = checkpoint_started.elapsed().as_secs_f64() * 1000.0;
    let bytes_after_checkpoint = file_family_bytes(path);

    let vacuum_started = Instant::now();
    connection
        .execute_batch("VACUUM;")
        .expect("vacuum diagnostic database");
    let vacuum_ms = vacuum_started.elapsed().as_secs_f64() * 1000.0;

    let post_vacuum_checkpoint_started = Instant::now();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint WAL after vacuum");
    let post_vacuum_checkpoint_ms =
        post_vacuum_checkpoint_started.elapsed().as_secs_f64() * 1000.0;

    let aggregate_started = Instant::now();
    let total: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM diagnostic_events",
            [],
            |row| row.get(0),
        )
        .expect("count diagnostic rows");
    let mut by_component = BTreeMap::<String, u64>::new();
    let mut statement = connection
        .prepare(
            "SELECT component, COUNT(*) FROM diagnostic_events GROUP BY component",
        )
        .expect("prepare aggregate");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("aggregate rows");
    for row in rows {
        let (component, count) = row.expect("decode aggregate row");
        by_component.insert(component, count as u64);
    }
    let aggregate_ms = aggregate_started.elapsed().as_secs_f64() * 1000.0;
    let retained_bytes = file_family_bytes(path);

    json!({
        "candidate": "sqlite",
        "events_input": events,
        "write_latency": summary(&latencies),
        "total_write_ms": total_ms,
        "events_per_second": events as f64 / (total_ms / 1000.0),
        "durability_window_events": durability_window,
        "durability_sync_latency": summary(&sync_latencies),
        "logical_bytes_written": logical_bytes,
        "bytes_after_writes": bytes_after_writes,
        "retention_target_rows": retention_target_rows,
        "prune_ms": prune_ms,
        "checkpoint_ms": checkpoint_ms,
        "bytes_after_checkpoint": bytes_after_checkpoint,
        "vacuum_ms": vacuum_ms,
        "post_vacuum_checkpoint_ms": post_vacuum_checkpoint_ms,
        "retained_bytes": retained_bytes,
        "logical_bytes_per_input_event": logical_bytes as f64 / events as f64,
        "retained_bytes_per_input_event": retained_bytes as f64 / events as f64,
        "aggregate_ms": aggregate_ms,
        "aggregate": {
            "total_events": total,
            "by_component": by_component
        },
        "sqlite_version": rusqlite::version()
    })
}

fn file_family_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
        total = total.saturating_add(
            fs::metadata(candidate).map(|metadata| metadata.len()).unwrap_or(0),
        );
    }
    total
}


fn register_etw() -> REGHANDLE {
    let mut handle: REGHANDLE = 0;
    let status = unsafe {
        EventRegister(
            &ETW_PROVIDER,
            None,
            std::ptr::null(),
            &mut handle,
        )
    };
    if status != 0 {
        panic!("EventRegister failed with status {status}");
    }
    handle
}

fn bench_etw(events: usize) -> Value {
    let handle = register_etw();
    let enabled = unsafe { EventProviderEnabled(handle, 4, 0) };
    let mut latencies = Vec::with_capacity(events);
    let mut logical_bytes = 0u64;
    let mut failed_writes = 0u64;
    let started = Instant::now();

    for index in 0..events {
        let event = synthetic_event(index, 192);
        let encoded = serde_json::to_string(&event).expect("serialize ETW fixture event");
        logical_bytes += encoded.len() as u64;
        let mut wide: Vec<u16> = encoded.encode_utf16().collect();
        wide.push(0);

        let before = Instant::now();
        let status = unsafe { EventWriteString(handle, 4, 0, wide.as_ptr()) };
        latencies.push(before.elapsed().as_secs_f64() * 1000.0);
        if status != 0 {
            failed_writes = failed_writes.saturating_add(1);
        }
    }
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    unsafe {
        EventUnregister(handle);
    }

    json!({
        "candidate": "etw",
        "provider_guid": "{9a421fa8-f21b-4f9e-99ab-1db6bb8b1470}",
        "provider_enabled_at_start": enabled,
        "events_input": events,
        "write_latency": summary(&latencies),
        "total_write_ms": total_ms,
        "events_per_second": events as f64 / (total_ms / 1000.0),
        "logical_bytes_written": logical_bytes,
        "logical_bytes_per_input_event": logical_bytes as f64 / events as f64,
        "failed_writes": failed_writes,
        "retained_bytes": 0,
        "aggregate_supported_by_probe": false,
        "durable_without_consumer_session": false
    })
}


fn bench_detail_jsonl(normal_dir: &Path, detail_dir: &Path, events: usize) -> Value {
    let mut normal = JsonlDiagnostics::open(DiagnosticConfig::new(normal_dir))
        .expect("open normal detail-comparison logger");
    let mut detail = JsonlDiagnostics::open(DiagnosticConfig::new(detail_dir))
        .expect("open detail logger");
    detail
        .enable_detail(60_000)
        .expect("enable temporary detail mode");

    let mut normal_latencies = Vec::with_capacity(events);
    let mut detail_latencies = Vec::with_capacity(events);
    let mut normal_bytes = 0u64;
    let mut detail_bytes = 0u64;

    for index in 0..events {
        let raw = synthetic_event(index, 3000);

        let started = Instant::now();
        let normal_outcome = normal.append(raw.clone()).expect("normal detail fixture");
        normal_latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        normal_bytes += normal_outcome.bytes_written as u64;

        let started = Instant::now();
        let detail_outcome = detail.append(raw).expect("detail fixture");
        detail_latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        detail_bytes += detail_outcome.bytes_written as u64;
    }
    normal.flush().expect("flush normal detail comparison");
    detail.flush().expect("flush detail comparison");

    json!({
        "events": events,
        "normal": {
            "write_latency": summary(&normal_latencies),
            "logical_bytes": normal_bytes,
            "bytes_per_event": normal_bytes as f64 / events as f64
        },
        "detail": {
            "write_latency": summary(&detail_latencies),
            "logical_bytes": detail_bytes,
            "bytes_per_event": detail_bytes as f64 / events as f64
        },
        "amplification": {
            "bytes_ratio": detail_bytes as f64 / normal_bytes as f64,
            "p50_latency_ratio":
                percentile(&detail_latencies, 50.0) / percentile(&normal_latencies, 50.0)
        }
    })
}

fn recovery_jsonl(directory: &Path) -> Value {
    let mut diagnostics =
        JsonlDiagnostics::open(DiagnosticConfig::new(directory))
            .expect("open recovery logger");
    diagnostics
        .append(synthetic_event(1, 192))
        .expect("write recovery fixture");
    diagnostics.flush().expect("flush recovery fixture");
    drop(diagnostics);

    let current = directory.join("relay-diagnostics.jsonl");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&current)
        .expect("open recovery tail");
    file.write_all(b"{\"partial\":").expect("append partial tail");
    file.flush().expect("flush partial tail");
    drop(file);

    let diagnostics =
        JsonlDiagnostics::open(DiagnosticConfig::new(directory))
            .expect("reopen recovery logger");
    let health = diagnostics.health();
    let aggregate = diagnostics.aggregate().expect("aggregate recovered log");
    json!({
        "recovered_partial_bytes": health.recovered_partial_bytes,
        "total_events_after_recovery": aggregate.total_events,
        "invalid_lines_after_recovery": aggregate.invalid_lines,
        "health_ok": health.ok
    })
}


fn idle_jsonl(directory: &Path, duration_ms: u64) {
    let diagnostics =
        JsonlDiagnostics::open(DiagnosticConfig::new(directory))
            .expect("open idle JSONL diagnostics");
    println!(
        "{}",
        serde_json::to_string(&json!({
            "candidate": "jsonl",
            "pid": std::process::id(),
            "health": diagnostics.health(),
            "duration_ms": duration_ms
        }))
        .unwrap()
    );
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(duration_ms));
}

fn idle_sqlite(path: &Path, duration_ms: u64) {
    let connection = open_sqlite(path);
    println!(
        "{}",
        serde_json::to_string(&json!({
            "candidate": "sqlite",
            "pid": std::process::id(),
            "sqlite_version": rusqlite::version(),
            "duration_ms": duration_ms
        }))
        .unwrap()
    );
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(duration_ms));
    drop(connection);
}

fn idle_etw(duration_ms: u64) {
    let handle = register_etw();
    let enabled = unsafe { EventProviderEnabled(handle, 4, 0) };
    println!(
        "{}",
        serde_json::to_string(&json!({
            "candidate": "etw",
            "pid": std::process::id(),
            "provider_guid": "{9a421fa8-f21b-4f9e-99ab-1db6bb8b1470}",
            "provider_enabled": enabled,
            "duration_ms": duration_ms
        }))
        .unwrap()
    );
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(duration_ms));
    unsafe {
        EventUnregister(handle);
    }
}

fn parse_usize(value: Option<&String>, name: &str) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| panic!("invalid {name}"))
}

fn parse_u64(value: Option<&String>, name: &str) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("invalid {name}"))
}

fn print_json(value: Value) {
    println!("{}", serde_json::to_string(&value).unwrap());
}


fn usage() -> ! {
    eprintln!(
        "usage: relay-diagnostics-probe \
         bench-jsonl <dir> <events> | \
         bench-jsonl-sync1 <dir> <events> | \
         bench-sqlite <db> <events> | \
         bench-etw <events> | \
         bench-detail-jsonl <normal_dir> <detail_dir> <events> | \
         recovery-jsonl <dir> | \
         idle-jsonl <dir> <duration_ms> | \
         idle-sqlite <db> <duration_ms> | \
         idle-etw <duration_ms>"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bench-jsonl") if args.len() == 3 => {
            let events = parse_usize(args.get(2), "events");
            print_json(bench_jsonl(Path::new(&args[1]), events, 16));
        }
        Some("bench-jsonl-sync1") if args.len() == 3 => {
            let events = parse_usize(args.get(2), "events");
            print_json(bench_jsonl(Path::new(&args[1]), events, 1));
        }
        Some("bench-sqlite") if args.len() == 3 => {
            let events = parse_usize(args.get(2), "events");
            print_json(bench_sqlite(Path::new(&args[1]), events));
        }
        Some("bench-etw") if args.len() == 2 => {
            let events = parse_usize(args.get(1), "events");
            print_json(bench_etw(events));
        }
        Some("bench-detail-jsonl") if args.len() == 4 => {
            let events = parse_usize(args.get(3), "events");
            print_json(bench_detail_jsonl(
                Path::new(&args[1]),
                Path::new(&args[2]),
                events,
            ));
        }
        Some("recovery-jsonl") if args.len() == 2 => {
            print_json(recovery_jsonl(Path::new(&args[1])));
        }
        Some("idle-jsonl") if args.len() == 3 => {
            idle_jsonl(
                Path::new(&args[1]),
                parse_u64(args.get(2), "duration_ms"),
            );
        }
        Some("idle-sqlite") if args.len() == 3 => {
            idle_sqlite(
                Path::new(&args[1]),
                parse_u64(args.get(2), "duration_ms"),
            );
        }
        Some("idle-etw") if args.len() == 2 => {
            idle_etw(parse_u64(args.get(1), "duration_ms"));
        }
        _ => usage(),
    }
}
