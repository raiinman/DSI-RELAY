mod assets;
mod dashboard;
mod parser;
mod pipe;
mod security;
mod state;
mod tool_discovery;
mod uefn;
mod verse;
mod watcher;

use relay_contracts::registry;
use relay_contracts::{
    CommandRequest, HelloRequest, HelloResponse, IpcSecurityState, LOCAL_HOST_STATE_FORMAT,
    LocalHostState, PROTOCOL_MAX, PROTOCOL_MIN, Producer, ProtocolRange, negotiate_protocol,
};
use relay_core::policy::ExecutionAuthority;
use relay_core::service::{CoreConfig, RelayCore, RuntimeContext};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DAEMON_NAME: &str = "relayd";

fn probe_storage_schema(state_dir: &Path) -> Result<i64, &'static str> {
    if !state_dir.is_absolute() {
        return Err("STORAGE_PROBE_PATH_INVALID");
    }
    match std::fs::symlink_metadata(state_dir) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("STORAGE_PROBE_PATH_INVALID");
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err("STORAGE_PROBE_UNREADABLE");
        }
        _ => {}
    }
    let db = state_dir.join("relay.sqlite3");
    let mut sidecar_present = false;
    for suffix in ["relay.sqlite3-wal", "relay.sqlite3-shm", "relay.sqlite3-journal"] {
        match std::fs::symlink_metadata(state_dir.join(suffix)) {
            Ok(_) => sidecar_present = true,
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err("STORAGE_PROBE_UNREADABLE");
            }
            _ => {}
        }
    }
    if sidecar_present {
        return Err("STORAGE_PROBE_INCOMPLETE");
    }
    let metadata = match std::fs::symlink_metadata(&db) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => return Err("STORAGE_PROBE_PATH_INVALID"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err("STORAGE_PROBE_UNREADABLE"),
    };
    if metadata.len() == 0 {
        return Err("STORAGE_PROBE_INVALID");
    }
    let path = db.to_str().ok_or("STORAGE_PROBE_PATH_INVALID")?;
    let mut uri = String::from("file:///");
    for byte in path.replace('\\', "/").bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri.push_str("?immutable=1");
    let conn = rusqlite::Connection::open_with_flags(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|_| "STORAGE_PROBE_UNREADABLE")?;
    conn.execute_batch("BEGIN")
        .map_err(|_| "STORAGE_PROBE_UNREADABLE")?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|_| "STORAGE_PROBE_INVALID")?;
    if integrity != "ok" {
        return Err("STORAGE_PROBE_INVALID");
    }
    let (min, max, count): (i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(MIN(version), 0), COALESCE(MAX(version), 0), COUNT(*) FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "STORAGE_PROBE_INVALID")?;
    if min != 1 || max < 1 || max > 10_000 || count != max {
        return Err("STORAGE_PROBE_INVALID");
    }
    let core_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('projects','results','jobs')",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "STORAGE_PROBE_INVALID")?;
    if core_tables != 3 {
        return Err("STORAGE_PROBE_INVALID");
    }
    Ok(max)
}

fn run_storage_probe(args: &[String]) -> Result<(), &'static str> {
    let [mode, option, path] = args else {
        return Err("STORAGE_PROBE_ARGUMENTS_INVALID");
    };
    if mode != "--probe-storage-schema" || option != "--state-dir" {
        return Err("STORAGE_PROBE_ARGUMENTS_INVALID");
    }
    let schema_version = probe_storage_schema(Path::new(path))?;
    println!("{}", json!({
        "ok": true,
        "schema_version": schema_version,
        "database": if schema_version == 0 { "absent" } else { "present" }
    }));
    Ok(())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn fnv1a64(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
fn pipe_name(sid: &str) -> String {
    let instance = std::env::var("RELAY_INSTANCE")
        .ok()
        .map(|value| {
            value
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
                .take(48)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{:016x}", fnv1a64(sid)));
    format!(r"\\.\pipe\dsi-relay-{instance}-v1")
}

fn secure_equal(actual: &str, expected: &str) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    actual
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn host_capabilities() -> Vec<String> {
    let mut values = registry::capability_ids();
    values.extend([
        "protocol.handshake@1".to_string(),
        "ipc.named-pipe.current-user@1".to_string(),
        "diagnostics.structured-jsonl@1".to_string(),
    ]);
    values.sort();
    values.dedup();
    values
}
fn runtime_context(
    started: Instant,
    ipc_security: &IpcSecurityState,
    core: &RelayCore,
    parser_health: &parser::ParserHealth,
    include_host_health: bool,
) -> RuntimeContext {
    RuntimeContext {
        pid: std::process::id(),
        runtime: "rust".to_string(),
        uptime_ms: started.elapsed().as_millis() as u64,
        ipc_healthy: true,
        ipc_security: serde_json::to_value(ipc_security)
            .unwrap_or_else(|_| json!({ "transport": "windows_named_pipe" })),
        capabilities: vec![
            "protocol.handshake@1".to_string(),
            "ipc.named-pipe.current-user@1".to_string(),
            "diagnostics.structured-jsonl@1".to_string(),
        ],
        host_components: if include_host_health {
            vec![parser_health.snapshot(core)]
        } else {
            Vec::new()
        },
    }
}

fn hello_error(handle: windows_sys::Win32::Foundation::HANDLE, code: &str, message: &str) {
    let _ = pipe::write_json(
        handle,
        &json!({
            "type": "hello_error",
            "code": code,
            "message": message
        }),
    );
}

fn run() -> Result<(), String> {
    registry::validate_embedded_registry().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let started_at_unix_ms = unix_ms();
    let state_dir = state::state_dir();
    let state_file = state::state_path();
    let security = security::current_user_pipe_security()?;
    let pipe_name = pipe_name(&security.sid);
    let auth_token = security::random_hex(32)?;
    let server = pipe::create_server(&pipe_name, &security)?;
    let verified = security::verify_pipe_security(server.raw(), &security.sid)?;

    if !verified.query_ok
        || !verified.protected_dacl
        || !verified.owner_is_current_user
        || !verified.current_user_only
        || !verified.current_user_full_control
        || verified.ace_count != 1
    {
        return Err(format!(
            "kernel verification rejected local pipe security: query_ok={}, protected_dacl={}, owner_is_current_user={}, current_user_only={}, current_user_full_control={}, ace_count={}",
            verified.query_ok,
            verified.protected_dacl,
            verified.owner_is_current_user,
            verified.current_user_only,
            verified.current_user_full_control,
            verified.ace_count,
        ));
    }

    let ipc_security = IpcSecurityState {
        transport: "windows_named_pipe".to_string(),
        explicit_dacl: true,
        kernel_acl_verified: true,
        owner_current_user: true,
        acl_ace_count: verified.ace_count,
        scope: "current_user".to_string(),
        auth_token: true,
    };

    let core = Arc::new(RelayCore::open(CoreConfig::new(&state_dir)));
    let (parser_installations, parser_load_failed) = match parser::load_installations(&state_dir) {
        Ok(installations) => (installations, false),
        Err(_) => (Default::default(), true),
    };
    let parser_health = parser::ParserHealth::new(&parser_installations, parser_load_failed);
    if parser_load_failed {
        core.record_host_component_event(
            "adapter.dependencies.parse",
            "PARSER_INSTALLATION_INVALID",
            false,
        );
    }
    let watcher_enabled = std::env::var("RELAY_TEST_DISABLE_WATCHER").as_deref() != Ok("1");
    if watcher_enabled {
        for (project_id, _) in core.index_watch_targets().unwrap_or_default() {
            let _ = core.mark_index_stale(&project_id);
        }
    }
    let initial_runtime = runtime_context(started, &ipc_security, &core, &parser_health, true);
    let initial_status = core.execute(
        CommandRequest {
            request_id: "BOOT-status".to_string(),
            command: "system.status".to_string(),
            command_version: Some(1),
            arguments: json!({}),
            idempotency_key: None,
            context: relay_contracts::RequestContext::default(),
        },
        &initial_runtime,
    );
    let initial_result = initial_status
        .result
        .as_ref()
        .ok_or_else(|| "initial Core status failed".to_string())?;
    let recovery_state = initial_result["recovery_state"]
        .as_str()
        .unwrap_or("Degraded")
        .to_string();
    let storage_schema_version = initial_result["storage"]["schema_version"].as_i64();

    let state = LocalHostState {
        state_format: LOCAL_HOST_STATE_FORMAT,
        pid: std::process::id(),
        pipe: pipe_name.clone(),
        auth_token: auth_token.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol: ProtocolRange {
            min: PROTOCOL_MIN,
            max: PROTOCOL_MAX,
        },
        capabilities: host_capabilities(),
        recovery_state,
        storage_schema_version,
        ipc_security: ipc_security.clone(),
        started_at_unix_ms,
    };
    state::write_state(&state_file, &state)?;
    let mut dashboard = match dashboard::DashboardServer::start(
        Arc::clone(&core),
        started,
        ipc_security.clone(),
        parser_health.clone(),
    ) {
        Ok(server) => server,
        Err(error) => {
            state::clear_state(&state_file);
            return Err(error);
        }
    };
    if let Err(error) = state::write_dashboard_url(&state_dir, &dashboard.url()) {
        dashboard.stop();
        state::clear_state(&state_file);
        return Err(error);
    }

    println!(
        "{}",
        serde_json::to_string(&json!({
            "component": DAEMON_NAME,
            "pid": state.pid,
            "version": state.version,
            "recovery_state": state.recovery_state,
            "storage_schema_version": state.storage_schema_version
        }))
        .map_err(|error| format!("serialize ready state: {error}"))?
    );

    let shutdown = AtomicBool::new(false);
    let foreground_active = Arc::new(AtomicBool::new(false));
    let mut parser = parser::ParserRuntime::start(
        Arc::clone(&core),
        initial_runtime.clone(),
        Arc::clone(&foreground_active),
        parser_installations,
        parser_health.clone(),
    );
    let mut watcher = watcher_enabled.then(|| {
        watcher::WatcherRuntime::start(
            Arc::clone(&core),
            initial_runtime.clone(),
            Arc::clone(&foreground_active),
            started_at_unix_ms,
        )
    });

    while !shutdown.load(Ordering::SeqCst) {
        if let Err(error) = pipe::wait_for_client(server.raw()) {
            core.flush_diagnostics();
            state::clear_dashboard_url(&state_dir);
            state::clear_state(&state_file);
            return Err(error);
        }

        let hello: HelloRequest = match pipe::read_json(server.raw()) {
            Ok(value) => value,
            Err(_) => {
                hello_error(server.raw(), "BAD_HANDSHAKE", "hello request was invalid");
                pipe::disconnect(server.raw());
                continue;
            }
        };

        if hello.message_type != "hello" || !secure_equal(&hello.auth_token, &auth_token) {
            hello_error(
                server.raw(),
                "UNAUTHORIZED",
                "local host authentication failed",
            );
            pipe::disconnect(server.raw());
            continue;
        }

        let Some(protocol) = negotiate_protocol(hello.protocol_min, hello.protocol_max) else {
            hello_error(
                server.raw(),
                "PROTOCOL_INCOMPATIBLE",
                "client/host protocol ranges do not overlap",
            );
            pipe::disconnect(server.raw());
            continue;
        };
        let greeting = HelloResponse {
            message_type: "hello_ok".to_string(),
            protocol,
            schema_version: relay_contracts::ENVELOPE_SCHEMA_VERSION,
            server: Producer {
                name: DAEMON_NAME.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: host_capabilities(),
        };
        if pipe::write_json(server.raw(), &greeting).is_err() {
            pipe::disconnect(server.raw());
            continue;
        }

        let request: CommandRequest = match pipe::read_json(server.raw()) {
            Ok(value) => value,
            Err(_) => {
                let _ = pipe::write_json(
                    server.raw(),
                    &json!({
                        "type": "transport_error",
                        "code": "INVALID_REQUEST",
                        "message": "command request was invalid"
                    }),
                );
                pipe::disconnect(server.raw());
                continue;
            }
        };

        let authority = ExecutionAuthority::local_user(hello.client.name.clone());

        let should_shutdown = request.command == "system.shutdown";
        let refresh_watches = matches!(
            request.command.as_str(),
            "project.register" | "project.import" | "project.index.build"
        );
        let foreground_work = relay_contracts::registry::registry()
            .commands
            .iter()
            .find(|command| command.id == request.command)
            .is_none_or(|command| command.effect_class != "observe");
        let include_host_health =
            matches!(request.command.as_str(), "system.status" | "system.doctor");
        let runtime = runtime_context(
            started,
            &ipc_security,
            &core,
            &parser_health,
            include_host_health,
        );
        if foreground_work {
            foreground_active.store(true, Ordering::SeqCst);
        }
        let response =
            core.execute_authorized_with_extension(request, &runtime, &authority, |request| {
                tool_discovery::execute(request)
                    .or_else(|| uefn::execute(&core, request))
                    .or_else(|| assets::execute_authorized(&core, request, &runtime, &authority))
                    .or_else(|| verse::execute(&core, request))
            });
        let accepted_shutdown = should_shutdown && response.ok;
        let _ = pipe::write_json(server.raw(), &response);
        pipe::disconnect(server.raw());
        if foreground_work {
            foreground_active.store(false, Ordering::SeqCst);
        }
        if refresh_watches
            && response.ok
            && let Some(watcher) = &watcher
        {
            watcher.refresh();
        }

        if accepted_shutdown {
            shutdown.store(true, Ordering::SeqCst);
        }
    }
    if let Some(watcher) = &mut watcher {
        watcher.stop();
    }
    if let Some(parser) = &mut parser {
        parser.stop();
    }
    dashboard.stop();
    state::clear_dashboard_url(&state_dir);
    core.flush_diagnostics();
    state::clear_state(&state_file);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(code) = run_storage_probe(&args) {
            eprintln!("{}", json!({"ok": false, "error": {"code": code}}));
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = run() {
        eprintln!(
            "{}",
            serde_json::to_string(&json!({
                "ok": false,
                "error": {
                    "code": "RELAYD_ERROR",
                    "message": error
                }
            }))
            .unwrap_or_else(|_| "{\"ok\":false}".to_string())
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod schema_probe_tests {
    use super::probe_storage_schema;
    use relay_core::storage::RelayStorage;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("relay-schema-probe-{label}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn absent_database_is_zero_without_creating_a_directory() {
        let dir = fixture_dir("absent");
        assert_eq!(probe_storage_schema(&dir), Ok(0));
        assert!(!dir.exists());
    }

    #[test]
    fn existing_database_is_probed_without_migration() {
        let dir = fixture_dir("existing space");
        let db = dir.join("relay.sqlite3");
        let storage = RelayStorage::open(&db).unwrap();
        let version = storage.schema_version().unwrap();
        drop(storage);
        let before: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(probe_storage_schema(&dir), Ok(version));
        assert_eq!(probe_storage_schema(&dir), Ok(version));
        let after: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(after, before);
        std::fs::write(dir.join("relay.sqlite3-wal"), b"uncheckpointed").unwrap();
        assert_eq!(probe_storage_schema(&dir), Err("STORAGE_PROBE_INCOMPLETE"));
        std::fs::remove_file(dir.join("relay.sqlite3-wal")).unwrap();
        let storage = RelayStorage::open(&db).unwrap();
        assert_eq!(storage.schema_version().unwrap(), version);
    }

    #[test]
    fn damaged_database_and_orphaned_journal_fail_closed() {
        let bad = fixture_dir("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("relay.sqlite3"), b"not sqlite").unwrap();
        assert!(probe_storage_schema(&bad).is_err());

        let orphaned = fixture_dir("orphaned");
        std::fs::create_dir_all(&orphaned).unwrap();
        std::fs::write(orphaned.join("relay.sqlite3-wal"), b"orphaned").unwrap();
        assert_eq!(probe_storage_schema(&orphaned), Err("STORAGE_PROBE_INCOMPLETE"));
    }
}
