use crate::diagnostics::{DiagnosticHealth, JsonlDiagnostics};
use crate::registry::{self, ResolveError};
use crate::storage::{RelayStorage, StorageError, StorageHealth};
use crate::{capabilities, PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct HostContext {
    pub started: Instant,
    pub shutdown: Arc<AtomicBool>,
    pub explicit_dacl: bool,
    pub kernel_acl_verified: bool,
    pub acl_owner_current_user: bool,
    pub acl_ace_count: u32,
    pub storage: Option<Arc<Mutex<RelayStorage>>>,
    pub storage_health: StorageHealth,
    pub diagnostics: Option<Arc<Mutex<JsonlDiagnostics>>>,
    pub diagnostics_fallback: DiagnosticHealth,
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
        "capabilities": capabilities()
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

fn with_command_version(mut response: Value, version: u32) -> Value {
    if let Some(object) = response.as_object_mut() {
        object.insert("command_version".to_string(), json!(version));
    }
    response
}

fn with_storage<F>(context: &HostContext, operation: F) -> Result<Value, StorageError>
where
    F: FnOnce(&RelayStorage) -> Result<Value, StorageError>,
{
    let storage = context.storage.as_ref().ok_or_else(|| {
        StorageError::new(
            "STORAGE_UNAVAILABLE",
            "RELAY storage is unavailable; run relay doctor for details.",
        )
    })?;
    let guard = storage.lock().map_err(|_| {
        StorageError::new("STORAGE_UNAVAILABLE", "RELAY storage lock is unavailable.")
    })?;
    operation(&guard)
}

fn storage_result(request_id: &str, result: Result<Value, StorageError>) -> Value {
    match result {
        Ok(value) => success(request_id, value),
        Err(error) => failure(request_id, error.code, &error.message),
    }
}

fn diagnostics_health(context: &HostContext) -> DiagnosticHealth {
    match &context.diagnostics {
        Some(logger) => logger
            .lock()
            .map(|logger| logger.health())
            .unwrap_or_else(|_| {
                DiagnosticHealth::unavailable("diagnostics lock is unavailable")
            }),
        None => context.diagnostics_fallback.clone(),
    }
}

pub fn dispatch_command(
    request_id: &str,
    command: &str,
    requested_version: Option<u32>,
    arguments: Value,
    context: &HostContext,
) -> Value {
    let spec = match registry::resolve_command(command, requested_version) {
        Ok(spec) => spec,
        Err(ResolveError::UnknownCommand) => {
            return failure(
                request_id,
                "COMMAND_UNKNOWN",
                &format!("Unknown command: {command}"),
            );
        }
        Err(ResolveError::VersionIncompatible { supported }) => {
            return failure(
                request_id,
                "COMMAND_VERSION_INCOMPATIBLE",
                &format!(
                    "Command {command} does not support requested version {}; supported versions: {:?}",
                    requested_version.unwrap_or(0),
                    supported
                ),
            );
        }
    };

    if let Err(error) = registry::validate_value(&spec.arguments_schema, &arguments) {
        return with_command_version(
            failure(
                request_id,
                "VALIDATION_FAILED",
                &format!("{} {}", error.path, error.message),
            ),
            spec.version,
        );
    }

    let response = match command {
        "system.status" => {
            let diagnostic_health = diagnostics_health(context);
            success(
                request_id,
                json!({
                    "process_health": "running",
                    "recovery_state": if context.storage_health.ok && diagnostic_health.ok { "Healthy" } else { "Degraded" },
                    "pid": std::process::id(),
                    "version": RELAY_VERSION,
                    "runtime": "rust",
                    "protocol": {
                        "min": PROTOCOL_MIN,
                        "max": PROTOCOL_MAX,
                        "negotiated": PROTOCOL_MAX
                    },
                    "capabilities": capabilities(),
                    "ipc_security": {
                        "transport": "windows_named_pipe",
                        "explicit_dacl": context.explicit_dacl,
                        "kernel_acl_verified": context.kernel_acl_verified,
                        "owner_current_user": context.acl_owner_current_user,
                        "acl_ace_count": context.acl_ace_count,
                        "scope": "current_user",
                        "auth_token": true
                    },
                    "storage": &context.storage_health,
                    "diagnostics": diagnostic_health,
                    "uptime_ms": context.started.elapsed().as_millis() as u64
                }),
            )
        },
        "system.doctor" => {
            let security_ok = context.explicit_dacl && context.kernel_acl_verified;
            let storage_ok = context.storage_health.ok;
            let diagnostic_health = diagnostics_health(context);
            let diagnostics_ok = diagnostic_health.ok;
            success(
                request_id,
                json!({
                    "healthy": security_ok && storage_ok && diagnostics_ok,
                    "summary": if security_ok && storage_ok && diagnostics_ok {
                        "RELAY host, IPC security, durable storage, and diagnostics checks passed."
                    } else if !storage_ok {
                        "RELAY host is reachable, but durable storage needs attention."
                    } else if !diagnostics_ok {
                        "RELAY host is reachable, but diagnostics capture needs attention."
                    } else {
                        "RELAY host is running, but its explicit pipe security verification did not pass."
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
                        {
                            "id": "storage.integrity",
                            "status": if storage_ok { "pass" } else { "fail" },
                            "detail": if storage_ok {
                                format!("SQLite quick_check: {}", context.storage_health.check)
                            } else {
                                context.storage_health.error.clone().unwrap_or_else(|| "Storage unavailable".to_string())
                            }
                        },
                        {
                            "id": "diagnostics.capture",
                            "status": if diagnostics_ok { "pass" } else { "fail" },
                            "detail": if diagnostics_ok {
                                format!(
                                    "Structured diagnostics active; {} current bytes, {} rotated file(s), {} evicted event(s).",
                                    diagnostic_health.current_bytes,
                                    diagnostic_health.rotated_files,
                                    diagnostic_health.evicted_events
                                )
                            } else {
                                diagnostic_health
                                    .last_error
                                    .clone()
                                    .unwrap_or_else(|| "Diagnostics unavailable".to_string())
                            }
                        }
                    ],
                    "next_action": if !storage_ok {
                        Value::String("Protect the damaged store, inspect recovery options, and avoid writes until storage is repaired or restored.".to_string())
                    } else if !diagnostics_ok {
                        Value::String("Inspect diagnostics storage/permissions and restore structured capture before treating support evidence as complete.".to_string())
                    } else {
                        Value::Null
                    }
                }),
            )
        },
        "system.echo" => success(request_id, json!({ "echo": arguments })),
        "system.shutdown" => {
            context.shutdown.store(true, Ordering::SeqCst);
            success(request_id, json!({ "shutting_down": true }))
        }
        "storage.integrity" => storage_result(
            request_id,
            with_storage(context, |storage| {
                Ok(serde_json::to_value(storage.integrity()?).map_err(|error| {
                    StorageError::new("STORAGE_ERROR", format!("serialize storage health: {error}"))
                })?)
            }),
        ),
        "project.register" => storage_result(
            request_id,
            with_storage(context, |storage| storage.register_project(&arguments)),
        ),
        "project.list" => storage_result(
            request_id,
            with_storage(context, |storage| {
                Ok(json!({ "projects": storage.list_projects()? }))
            }),
        ),
        "result.put" => storage_result(
            request_id,
            with_storage(context, |storage| storage.put_result(&arguments)),
        ),
        "result.get" => storage_result(
            request_id,
            with_storage(context, |storage| {
                let id = arguments
                    .get("result_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                storage.get_result(id)?.ok_or_else(|| {
                    StorageError::new("RESULT_NOT_FOUND", "Result not found")
                })
            }),
        ),
        "job.checkpoint" => storage_result(
            request_id,
            with_storage(context, |storage| storage.checkpoint_job(&arguments)),
        ),
        "job.get" => storage_result(
            request_id,
            with_storage(context, |storage| {
                let id = arguments
                    .get("job_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                storage.get_job(id)?.ok_or_else(|| {
                    StorageError::new("JOB_NOT_FOUND", "Job not found")
                })
            }),
        ),
        "registry.list" => {
            let surface = arguments.get("surface").and_then(Value::as_str);
            let prefix = arguments.get("prefix").and_then(Value::as_str);
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(50) as usize;
            success(
                request_id,
                registry::compact_list(surface, prefix, limit),
            )
        }
        "registry.describe" => {
            let target = arguments
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("");
            let version = arguments
                .get("version")
                .and_then(Value::as_u64)
                .map(|value| value as u32);
            match registry::describe(target, version) {
                Ok(value) => success(request_id, value),
                Err(ResolveError::UnknownCommand) => failure(
                    request_id,
                    "COMMAND_UNKNOWN",
                    &format!("Unknown command: {target}"),
                ),
                Err(ResolveError::VersionIncompatible { supported }) => failure(
                    request_id,
                    "COMMAND_VERSION_INCOMPATIBLE",
                    &format!(
                        "Command {target} does not support requested version {}; supported versions: {:?}",
                        version.unwrap_or(0),
                        supported
                    ),
                ),
            }
        }
        _ => failure(
            request_id,
            "COMMAND_UNKNOWN",
            &format!("Unknown command: {command}"),
        ),
    };

    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        if let Some(result) = response.get("result") {
            if let Err(error) = registry::validate_value(&spec.result_schema, result) {
                return with_command_version(
                    failure(
                        request_id,
                        "RESULT_SCHEMA_VIOLATION",
                        &format!("{} {}", error.path, error.message),
                    ),
                    spec.version,
                );
            }
        }
    } else if let Some(error_value) = response.get("error") {
        if let Err(error) =
            registry::validate_value(&registry::registry().error_schema, error_value)
        {
            return with_command_version(
                failure(
                    request_id,
                    "ERROR_SCHEMA_VIOLATION",
                    &format!("{} {}", error.path, error.message),
                ),
                spec.version,
            );
        }
        if let Some(code) = error_value.get("code").and_then(Value::as_str) {
            if !registry::is_error_allowed(spec, code) {
                return with_command_version(
                    failure(
                        request_id,
                        "UNDECLARED_COMMAND_ERROR",
                        &format!("Command {command}@{} returned undeclared error {code}", spec.version),
                    ),
                    spec.version,
                );
            }
        }
    }

    with_command_version(response, spec.version)
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
            storage: None,
            storage_health: StorageHealth::unavailable("fixture storage unavailable"),
            diagnostics: None,
            diagnostics_fallback: DiagnosticHealth::unavailable(
                "fixture diagnostics unavailable",
            ),
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
        let response = dispatch_command("req", "system.status", None, json!({}), &context());
        assert_eq!(response["ok"], true);
        assert_eq!(response["result"]["runtime"], "rust");
        assert_eq!(response["result"]["ipc_security"]["explicit_dacl"], true);
    }

    #[test]
    fn unavailable_storage_stays_explicit() {
        let response = dispatch_command("req", "project.list", None, json!({}), &context());
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "STORAGE_UNAVAILABLE");
    }

    #[test]
    fn command_version_mismatch_fails_explicitly() {
        let response = dispatch_command(
            "req",
            "system.status",
            Some(999),
            json!({}),
            &context(),
        );
        assert_eq!(response["ok"], false);
        assert_eq!(
            response["error"]["code"],
            "COMMAND_VERSION_INCOMPATIBLE"
        );
    }

    #[test]
    fn registry_validation_rejects_bad_arguments_before_business_logic() {
        let response = dispatch_command(
            "req",
            "project.register",
            Some(1),
            json!({ "root_uri": "file:///fixture" }),
            &context(),
        );
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "VALIDATION_FAILED");
        assert_eq!(response["command_version"], 1);
    }

    #[test]
    fn registry_discovery_is_available_without_storage() {
        let response = dispatch_command(
            "req",
            "registry.list",
            Some(1),
            json!({ "surface": "ai", "prefix": "result." }),
            &context(),
        );
        assert_eq!(response["ok"], true);
        assert_eq!(response["command_version"], 1);
        assert_eq!(response["result"]["commands"].as_array().unwrap().len(), 2);
    }
}
