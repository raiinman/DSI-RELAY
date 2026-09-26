use relay_rust_challenger::dashboard;
use relay_rust_challenger::diagnostics::{
    DiagnosticConfig, DiagnosticEvent, DiagnosticHealth, JsonlDiagnostics, Severity,
};
use relay_rust_challenger::pipe;
use relay_rust_challenger::protocol::{dispatch_command, hello_response, HostContext};
use relay_rust_challenger::registry;
use relay_rust_challenger::security::{
    current_user_pipe_security, random_hex, verify_pipe_security,
};
use relay_rust_challenger::state::{
    clear_state, db_path, read_state, state_dir, state_path, write_state, HostState,
    IpcSecurityState, ProtocolRange,
};
use relay_rust_challenger::storage::{RelayStorage, StorageHealth};
use relay_rust_challenger::{capabilities, PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn fnv1a64(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sanitize_instance(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(48)
        .collect()
}

fn pipe_name(sid: &str) -> String {
    let instance = std::env::var("RELAY_INSTANCE")
        .ok()
        .map(|value| sanitize_instance(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{:016x}", fnv1a64(sid)));
    format!(r"\\.\pipe\dsi-relay-rust-{instance}-v1")
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn record_command_event(
    context: &HostContext,
    request_id: &str,
    command: &str,
    response: &Value,
) {
    let Some(logger) = &context.diagnostics else {
        return;
    };
    let ok = response
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut event = DiagnosticEvent::new(
        "relay.command.completed",
        if ok { Severity::Info } else { Severity::Warn },
        "core.command",
        "RELAY command completed",
    );
    event.correlation_id = Some(request_id.to_string());
    event
        .attributes
        .insert("command".to_string(), json!(command));
    event.attributes.insert("ok".to_string(), json!(ok));
    if let Some(version) = response
        .get("command_version")
        .and_then(Value::as_u64)
    {
        event
            .attributes
            .insert("command_version".to_string(), json!(version));
    }
    if let Some(code) = response
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
    {
        event
            .attributes
            .insert("error_code".to_string(), json!(code));
    }
    if let Ok(mut logger) = logger.lock() {
        if let Err(error) = logger.append(event) {
            eprintln!("diagnostic command event failed: {error}");
        }
    }
}

fn flush_diagnostics(context: &HostContext) {
    if let Some(logger) = &context.diagnostics {
        if let Ok(mut logger) = logger.lock() {
            let event = DiagnosticEvent::new(
                "relay.host.stopping",
                Severity::Info,
                "core.host",
                "RELAY host stopping",
            );
            let _ = logger.append(event);
            let _ = logger.flush();
        }
    }
}

fn run_host(dashboard_enabled: bool) -> Result<(), String> {
    registry::validate_embedded_registry().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let security = current_user_pipe_security()?;
    let pipe_name = pipe_name(&security.sid);
    let auth_token = random_hex(32)?;
    let pipe_server = pipe::create_server(&pipe_name, &security)?;
    let acl_verification = verify_pipe_security(pipe_server.raw(), &security.sid)?;
    if !acl_verification.current_user_only
        || !acl_verification.current_user_full_control
        || !acl_verification.owner_is_current_user
        || !acl_verification.protected_dacl
        || acl_verification.ace_count != 1
    {
        return Err(format!(
            "kernel pipe DACL verification failed: protected={} current_user_only={} full_control={} owner_current_user={} ace_count={}",
            acl_verification.protected_dacl,
            acl_verification.current_user_only,
            acl_verification.current_user_full_control,
            acl_verification.owner_is_current_user,
            acl_verification.ace_count
        ));
    }
    let state_file = state_path();
    let db_file = db_path();
    let (storage, storage_health) = match RelayStorage::open(&db_file) {
        Ok(storage) => match storage.integrity() {
            Ok(health) if health.ok => (Some(Arc::new(Mutex::new(storage))), health),
            Ok(health) => (None, health),
            Err(error) => (
                None,
                StorageHealth::unavailable(format!("{}: {}", error.code, error.message)),
            ),
        },
        Err(error) => (
            None,
            StorageHealth::unavailable(format!("{}: {}", error.code, error.message)),
        ),
    };

    let diagnostics_dir = state_dir().join("diagnostics");
    let (diagnostics, diagnostics_fallback) =
        match JsonlDiagnostics::open(DiagnosticConfig::new(&diagnostics_dir)) {
            Ok(mut logger) => {
                let mut event = DiagnosticEvent::new(
                    "relay.host.started",
                    Severity::Info,
                    "core.host",
                    "RELAY host started",
                );
                event
                    .attributes
                    .insert("version".to_string(), json!(RELAY_VERSION));
                event
                    .attributes
                    .insert("storage_ok".to_string(), json!(storage_health.ok));
                if let Err(error) = logger.append(event) {
                    eprintln!("diagnostic startup event failed: {error}");
                }
                let health = logger.health();
                (Some(Arc::new(Mutex::new(logger))), health)
            }
            Err(error) => (
                None,
                DiagnosticHealth::unavailable(format!(
                    "{}: {}",
                    error.code, error.message
                )),
            ),
        };

    let shutdown = Arc::new(AtomicBool::new(false));
    let context = HostContext {
        started,
        shutdown: shutdown.clone(),
        explicit_dacl: true,
        kernel_acl_verified: acl_verification.query_ok
            && acl_verification.current_user_only
            && acl_verification.owner_is_current_user,
        acl_owner_current_user: acl_verification.owner_is_current_user,
        acl_ace_count: acl_verification.ace_count,
        storage,
        storage_health: storage_health.clone(),
        diagnostics,
        diagnostics_fallback: diagnostics_fallback.clone(),
    };

    let dashboard_server = if dashboard_enabled {
        Some(dashboard::start(context.clone())?)
    } else {
        None
    };
    let dashboard_state = dashboard_server.as_ref().map(|server| server.state.clone());

    let state = HostState {
        pid: std::process::id(),
        pipe: pipe_name.clone(),
        auth_token: auth_token.clone(),
        version: RELAY_VERSION.to_string(),
        protocol: ProtocolRange {
            min: PROTOCOL_MIN,
            max: PROTOCOL_MAX,
        },
        capabilities: capabilities(),
        recovery_state: if storage_health.ok && diagnostics_fallback.ok {
            "Healthy".to_string()
        } else {
            "Degraded".to_string()
        },
        storage_schema_version: storage_health.schema_version,
        ipc_security: IpcSecurityState {
            explicit_dacl: true,
            kernel_acl_verified: context.kernel_acl_verified,
            acl_ace_count: context.acl_ace_count,
            owner_current_user: context.acl_owner_current_user,
            scope: "current_user".to_string(),
            auth_token: true,
        },
        dashboard: dashboard_state,
        started_at_unix_ms: unix_ms(),
    };
    write_state(&state_file, &state)?;

    println!(
        "{}",
        serde_json::to_string(&json!({
            "component": "relayd-rust-challenger",
            "pid": std::process::id(),
            "version": RELAY_VERSION,
            "pipe": pipe_name,
            "explicit_dacl": true,
            "kernel_acl_verified": context.kernel_acl_verified,
            "owner_current_user": context.acl_owner_current_user,
            "acl_ace_count": context.acl_ace_count,
            "recovery_state": state.recovery_state,
            "storage_schema_version": state.storage_schema_version,
            "dashboard": state.dashboard
        }))
        .map_err(|error| format!("serialize ready: {error}"))?
    );

    while !shutdown.load(Ordering::SeqCst) {
        if let Err(error) = pipe::wait_for_client(pipe_server.raw()) {
            clear_state(&state_file);
            return Err(error);
        }

        let hello = match pipe::read_json(pipe_server.raw()) {
            Ok(value) => value,
            Err(_) => {
                pipe::disconnect(pipe_server.raw());
                continue;
            }
        };
        let greeting = hello_response(&hello, &auth_token);
        pipe::write_json(pipe_server.raw(), &greeting)?;
        if greeting.get("type").and_then(Value::as_str) != Some("hello_ok") {
            pipe::disconnect(pipe_server.raw());
            continue;
        }

        let message = match pipe::read_json(pipe_server.raw()) {
            Ok(value) => value,
            Err(error) => {
                let _ = pipe::write_json(
                    pipe_server.raw(),
                    &json!({ "type": "error", "code": "INVALID_JSON", "message": error }),
                );
                pipe::disconnect(pipe_server.raw());
                continue;
            }
        };

        if message.get("type").and_then(Value::as_str) != Some("command") {
            pipe::write_json(
                pipe_server.raw(),
                &json!({ "type": "error", "code": "BAD_MESSAGE" }),
            )?;
            pipe::disconnect(pipe_server.raw());
            continue;
        }

        let request_id = message
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("missing-request-id");
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("");
        let command_version = message
            .get("command_version")
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        let arguments = message
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let response = dispatch_command(
            request_id,
            command,
            command_version,
            arguments,
            &context,
        );
        record_command_event(&context, request_id, command, &response);
        pipe::write_json(pipe_server.raw(), &response)?;
        pipe::disconnect(pipe_server.raw());
    }

    flush_diagnostics(&context);
    clear_state(&state_file);
    if let Some(server) = dashboard_server {
        server.join();
    }
    Ok(())
}

fn call_command(command: &str, arguments: Value) -> Result<Value, String> {
    let state = read_state(&state_path()).map_err(|_| "HOST_UNAVAILABLE".to_string())?;
    pipe::call(&state.pipe, &state.auth_token, command, arguments, 3000)
}

fn print_machine(value: Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(&value).map_err(|error| format!("serialize output: {error}"))?
    );
    Ok(())
}

fn run_cli(args: &[String]) -> Result<(), String> {
    let command = args.first().map(String::as_str).unwrap_or("host");
    match command {
        "host" => run_host(
            args.iter().any(|arg| arg == "--dashboard")
                || std::env::var("RELAY_DASHBOARD_MODE").ok().as_deref() == Some("embedded"),
        ),
        "status" => print_machine(call_command("system.status", json!({}))?),
        "doctor" => print_machine(call_command("system.doctor", json!({}))?),
        "shutdown" => print_machine(call_command("system.shutdown", json!({}))?),
        "echo" => {
            let value = args.get(1).cloned().unwrap_or_default();
            print_machine(call_command("system.echo", json!({ "value": value }))?)
        }
        "commands" => print_machine(registry::compact_list(Some("cli"), None, 200)),
        "describe" => {
            let id = args.get(1).ok_or_else(|| "describe requires a command ID".to_string())?;
            let version = args.get(2).and_then(|value| value.parse::<u32>().ok());
            match registry::describe(id, version) {
                Ok(value) => print_machine(value),
                Err(error) => Err(format!("command description unavailable: {error:?}")),
            }
        }
        "help" => {
            println!("{}", registry::render_cli_catalog());
            Ok(())
        }
        "version" => print_machine(json!({
            "version": RELAY_VERSION,
            "schema_version": SCHEMA_VERSION,
            "protocol": { "min": PROTOCOL_MIN, "max": PROTOCOL_MAX }
        })),
        other => Err(format!("unknown CLI command: {other}")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run_cli(&args) {
        let code = if error == "HOST_UNAVAILABLE" {
            "HOST_UNAVAILABLE"
        } else {
            "RUST_CHALLENGER_ERROR"
        };
        eprintln!(
            "{}",
            serde_json::to_string(&json!({
                "ok": false,
                "error": { "code": code, "message": error }
            }))
            .unwrap_or_else(|_| "{\"ok\":false}".to_string())
        );
        std::process::exit(1);
    }
}
