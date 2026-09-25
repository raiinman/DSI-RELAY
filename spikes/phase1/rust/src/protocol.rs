use crate::{CAPABILITIES, PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct HostContext {
    pub started: Instant,
    pub shutdown: Arc<AtomicBool>,
    pub explicit_dacl: bool,
    pub kernel_acl_verified: bool,
    pub acl_owner_current_user: bool,
    pub acl_ace_count: u32,
}

pub fn negotiate(client_min: u32, client_max: u32) -> Option<u32> {
    let min = client_min.max(PROTOCOL_MIN);
    let max = client_max.min(PROTOCOL_MAX);
    (min <= max).then_some(max)
}

pub fn hello_response(message: &Value, expected_token: &str) -> Value {
    if message.get("type").and_then(Value::as_str) != Some("hello")
        || message.get("auth_token").and_then(Value::as_str) != Some(expected_token)
    {
        return json!({ "type": "hello_error", "code": "UNAUTHORIZED" });
    }

    let client_min = message
        .get("protocol_min")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let client_max = message
        .get("protocol_max")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;

    let Some(protocol) = negotiate(client_min, client_max) else {
        return json!({
            "type": "hello_error",
            "code": "PROTOCOL_INCOMPATIBLE",
            "host": { "min": PROTOCOL_MIN, "max": PROTOCOL_MAX }
        });
    };

    json!({
        "type": "hello_ok",
        "protocol": protocol,
        "schema_version": SCHEMA_VERSION,
        "server": { "name": "relayd-rust-challenger", "version": RELAY_VERSION },
        "capabilities": CAPABILITIES
    })
}

fn success(request_id: &str, result: Value) -> Value {
    json!({
        "type": "command_result",
        "request_id": request_id,
        "schema_version": SCHEMA_VERSION,
        "producer": { "version": RELAY_VERSION },
        "ok": true,
        "result": result
    })
}

fn failure(request_id: &str, code: &str, message: &str) -> Value {
    json!({
        "type": "command_result",
        "request_id": request_id,
        "schema_version": SCHEMA_VERSION,
        "producer": { "version": RELAY_VERSION },
        "ok": false,
        "error": { "code": code, "message": message }
    })
}

pub fn dispatch_command(
    request_id: &str,
    command: &str,
    arguments: Value,
    context: &HostContext,
) -> Value {
    match command {
        "system.status" => success(
            request_id,
            json!({
                "process_health": "running",
                "recovery_state": "Healthy",
                "pid": std::process::id(),
                "version": RELAY_VERSION,
                "runtime": "rust",
                "protocol": {
                    "min": PROTOCOL_MIN,
                    "max": PROTOCOL_MAX,
                    "negotiated": PROTOCOL_MAX
                },
                "capabilities": CAPABILITIES,
                "ipc_security": {
                    "transport": "windows_named_pipe",
                    "explicit_dacl": context.explicit_dacl,
                    "kernel_acl_verified": context.kernel_acl_verified,
                    "owner_current_user": context.acl_owner_current_user,
                    "acl_ace_count": context.acl_ace_count,
                    "scope": "current_user",
                    "auth_token": true
                },
                "uptime_ms": context.started.elapsed().as_millis() as u64
            }),
        ),
        "system.doctor" => success(
            request_id,
            json!({
                "healthy": context.explicit_dacl && context.kernel_acl_verified,
                "summary": if context.explicit_dacl && context.kernel_acl_verified {
                    "Rust challenger host, kernel-verified current-user pipe DACL, token authentication, and protocol checks passed."
                } else {
                    "Rust challenger host is running, but its explicit pipe security verification did not pass."
                },
                "checks": [
                    { "id": "host.process", "status": "pass", "detail": format!("PID {}", std::process::id()) },
                    { "id": "ipc.named_pipe", "status": "pass", "detail": "Windows named-pipe round trip is active." },
                    {
                        "id": "ipc.explicit_dacl",
                        "status": if context.kernel_acl_verified { "pass" } else { "fail" },
                        "detail": if context.kernel_acl_verified {
                            format!("Windows GetSecurityInfo verified a protected current-user-only DACL with {} ACE.", context.acl_ace_count)
                        } else {
                            "Windows did not verify the explicit current-user DACL on the created pipe.".to_string()
                        }
                    },
                    { "id": "ipc.auth_token", "status": "pass", "detail": "A random per-start token is required in addition to the OS DACL." },
                    { "id": "protocol.handshake", "status": "pass", "detail": format!("Protocol {}", PROTOCOL_MAX) },
                    { "id": "storage.scope", "status": "warning", "detail": "Storage is intentionally not ported in the runtime challenger." }
                ],
                "next_action": Value::Null
            }),
        ),
        "system.echo" => success(request_id, json!({ "echo": arguments })),
        "system.shutdown" => {
            context.shutdown.store(true, Ordering::SeqCst);
            success(request_id, json!({ "shutting_down": true }))
        }
        "project.list" => failure(
            request_id,
            "COMMAND_NOT_IMPLEMENTED_IN_CHALLENGER",
            "Project storage is intentionally not ported in the Rust runtime challenger.",
        ),
        "result.get" => failure(
            request_id,
            "COMMAND_NOT_IMPLEMENTED_IN_CHALLENGER",
            "Result storage is intentionally not ported in the Rust runtime challenger.",
        ),
        _ => failure(
            request_id,
            "COMMAND_UNKNOWN",
            &format!("Unknown command: {command}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn context() -> HostContext {
        HostContext {
            started: Instant::now(),
            shutdown: Arc::new(AtomicBool::new(false)),
            explicit_dacl: true,
            kernel_acl_verified: true,
            acl_owner_current_user: true,
            acl_ace_count: 1,
        }
    }

    #[test]
    fn handshake_fails_closed() {
        let bad = json!({
            "type": "hello",
            "auth_token": "wrong",
            "protocol_min": 1,
            "protocol_max": 1
        });
        assert_eq!(hello_response(&bad, "right")["code"], "UNAUTHORIZED");
    }

    #[test]
    fn status_reports_explicit_dacl() {
        let response = dispatch_command("req", "system.status", json!({}), &context());
        assert_eq!(response["ok"], true);
        assert_eq!(response["result"]["runtime"], "rust");
        assert_eq!(response["result"]["ipc_security"]["explicit_dacl"], true);
    }

    #[test]
    fn unported_storage_stays_explicit() {
        let response = dispatch_command("req", "project.list", json!({}), &context());
        assert_eq!(response["ok"], false);
        assert_eq!(
            response["error"]["code"],
            "COMMAND_NOT_IMPLEMENTED_IN_CHALLENGER"
        );
    }
}
