mod pipe;
mod security;
mod state;

use relay_contracts::registry;
use relay_contracts::{
    negotiate_protocol, CommandRequest, HelloRequest, HelloResponse,
    IpcSecurityState, LocalHostState, Producer, ProtocolRange,
    LOCAL_HOST_STATE_FORMAT, PROTOCOL_MAX, PROTOCOL_MIN,
};
use relay_core::policy::ExecutionAuthority;
use relay_core::service::{CoreConfig, RelayCore, RuntimeContext};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DAEMON_NAME: &str = "relayd";

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
                .filter(|ch| {
                    ch.is_ascii_alphanumeric()
                        || *ch == '-'
                        || *ch == '_'
                })
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
    }
}

fn hello_error(
    handle: windows_sys::Win32::Foundation::HANDLE,
    code: &str,
    message: &str,
) {
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
    registry::validate_embedded_registry()
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    let started_at_unix_ms = unix_ms();
    let state_dir = state::state_dir();
    let state_file = state::state_path();
    let security = security::current_user_pipe_security()?;
    let pipe_name = pipe_name(&security.sid);
    let auth_token = security::random_hex(32)?;
    let server = pipe::create_server(&pipe_name, &security)?;
    let verified =
        security::verify_pipe_security(server.raw(), &security.sid)?;

    if !verified.query_ok
        || !verified.protected_dacl
        || !verified.owner_is_current_user
        || !verified.current_user_only
        || !verified.current_user_full_control
        || verified.ace_count != 1
    {
        return Err(
            "kernel verification rejected local pipe security".to_string(),
        );
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

    let core = RelayCore::open(CoreConfig::new(&state_dir));
    let initial_runtime = runtime_context(started, &ipc_security);
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
    let storage_schema_version = initial_result["storage"]
        ["schema_version"]
        .as_i64();

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

    while !shutdown.load(Ordering::SeqCst) {
        if let Err(error) = pipe::wait_for_client(server.raw()) {
            core.flush_diagnostics();
            state::clear_state(&state_file);
            return Err(error);
        }

        let hello: HelloRequest = match pipe::read_json(server.raw()) {
            Ok(value) => value,
            Err(_) => {
                hello_error(
                    server.raw(),
                    "BAD_HANDSHAKE",
                    "hello request was invalid",
                );
                pipe::disconnect(server.raw());
                continue;
            }
        };

        if hello.message_type != "hello"
            || !secure_equal(&hello.auth_token, &auth_token)
        {
            hello_error(
                server.raw(),
                "UNAUTHORIZED",
                "local host authentication failed",
            );
            pipe::disconnect(server.raw());
            continue;
        }

        let Some(protocol) = negotiate_protocol(
            hello.protocol_min,
            hello.protocol_max,
        ) else {
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
            schema_version:
                relay_contracts::ENVELOPE_SCHEMA_VERSION,
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

        let request: CommandRequest =
            match pipe::read_json(server.raw()) {
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

        let authority =
            ExecutionAuthority::local_user(hello.client.name.clone());

        let should_shutdown = request.command == "system.shutdown";
        let runtime = runtime_context(started, &ipc_security);
        let response =
            core.execute_authorized(request, &runtime, &authority);
        let accepted_shutdown = should_shutdown && response.ok;
        let _ = pipe::write_json(server.raw(), &response);
        pipe::disconnect(server.raw());

        if accepted_shutdown {
            shutdown.store(true, Ordering::SeqCst);
        }
    }
    core.flush_diagnostics();
    state::clear_state(&state_file);
    Ok(())
}

fn main() {
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
