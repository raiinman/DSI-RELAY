use crate::diagnostics::{
    DiagnosticConfig, DiagnosticEvent, DiagnosticHealth,
    JsonlDiagnostics, Severity,
};
use crate::indexing;
use crate::policy::{
    CredentialHandle, DataClass, EgressDecision, EgressRequest,
    ExecutionAuthority,
};
use crate::storage::{
    CredentialHandleRecord, DependencyReplacement, EgressLedgerInput, NewTransaction,
    RelayStorage, StorageError, StorageHealth, UsageMetricInput,
};
use relay_contracts::registry::{self, ResolveError};
use relay_contracts::{
    CommandRequest, CommandResponse, Producer,
    PROTOCOL_MAX, PROTOCOL_MIN,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub data_dir: PathBuf,
}

impl CoreConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("relay.sqlite3")
    }

    pub fn diagnostics_dir(&self) -> PathBuf {
        self.data_dir.join("diagnostics")
    }
}
#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub pid: u32,
    pub runtime: String,
    pub uptime_ms: u64,
    pub ipc_healthy: bool,
    pub ipc_security: Value,
    pub capabilities: Vec<String>,
}

impl RuntimeContext {
    pub fn in_process() -> Self {
        Self {
            pid: std::process::id(),
            runtime: "in-process".to_string(),
            uptime_ms: 0,
            ipc_healthy: true,
            ipc_security: json!({
                "transport": "in-process",
                "scope": "current-process"
            }),
            capabilities: registry::capability_ids(),
        }
    }
}

pub struct RelayCore {
    storage: Option<Mutex<RelayStorage>>,
    storage_fallback: StorageHealth,
    diagnostics: Option<Mutex<JsonlDiagnostics>>,
    diagnostics_fallback: DiagnosticHealth,
    producer: Producer,
}
impl RelayCore {
    pub fn open(config: CoreConfig) -> Self {
        let _ = std::fs::create_dir_all(&config.data_dir);

        let (storage, storage_fallback) =
            match RelayStorage::open(config.database_path()) {
                Ok(storage) => {
                    let health = storage.integrity().unwrap_or_else(|error| {
                        StorageHealth::unavailable(error.to_string())
                    });
                    if health.ok {
                        (Some(Mutex::new(storage)), health)
                    } else {
                        (None, health)
                    }
                }
                Err(error) => (
                    None,
                    StorageHealth::unavailable(error.to_string()),
                ),
            };

        let (diagnostics, diagnostics_fallback) =
            match JsonlDiagnostics::open(
                DiagnosticConfig::new(config.diagnostics_dir()),
            ) {
                Ok(mut logger) => {
                    let event = DiagnosticEvent::new(
                        "relay.core.started",
                        Severity::Info,
                        "core",
                        "RELAY Core started",
                    );
                    let _ = logger.append(event);
                    let health = logger.health();
                    (Some(Mutex::new(logger)), health)
                }
                Err(error) => (
                    None,
                    DiagnosticHealth::unavailable(error.to_string()),
                ),
            };
        Self {
            storage,
            storage_fallback,
            diagnostics,
            diagnostics_fallback,
            producer: Producer {
                name: "relay-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    pub fn register_credential_handle_metadata(
        &self,
        handle: &CredentialHandle,
    ) -> Result<CredentialHandleRecord, StorageError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage is unavailable",
            )
        })?;
        let guard = storage.lock().map_err(|_| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage lock is unavailable",
            )
        })?;
        let scopes: Vec<String> =
            handle.scopes.iter().cloned().collect();
        guard.upsert_credential_handle(
            &handle.id,
            &handle.integration,
            &scopes,
            &handle.status,
        )
    }

    pub fn revoke_credential_handle_metadata(
        &self,
        handle_id: &str,
    ) -> Result<(), StorageError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage is unavailable",
            )
        })?;
        let guard = storage.lock().map_err(|_| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage lock is unavailable",
            )
        })?;
        guard.revoke_credential_handle(handle_id)
    }

    pub fn credential_handle_metadata(
        &self,
        handle_id: &str,
    ) -> Result<Option<CredentialHandleRecord>, StorageError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage is unavailable",
            )
        })?;
        let guard = storage.lock().map_err(|_| {
            StorageError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage lock is unavailable",
            )
        })?;
        guard.get_credential_handle(handle_id)
    }

    pub fn execute(
        &self,
        request: CommandRequest,
        runtime: &RuntimeContext,
    ) -> CommandResponse {
        let mut authority = ExecutionAuthority::local_user(
            request
                .context
                .client_id
                .clone()
                .unwrap_or_else(|| "in-process".to_string()),
        );
        if let Some(actor) = &request.context.actor_id {
            authority.actor_id = actor.clone();
        }
        authority.delegator_id = request.context.delegator_id.clone();
        self.execute_authorized(request, runtime, &authority)
    }

    pub fn execute_authorized(
        &self,
        mut request: CommandRequest,
        runtime: &RuntimeContext,
        authority: &ExecutionAuthority,
    ) -> CommandResponse {
        let started = Instant::now();
        request.context.actor_id = Some(authority.actor_id.clone());
        request.context.client_id = Some(authority.client_id.clone());
        request.context.delegator_id = authority.delegator_id.clone();

        let spec = match registry::resolve_command(
            &request.command,
            request.command_version,
        ) {
            Ok(spec) => spec,
            Err(ResolveError::UnknownCommand) => {
                let response = self.failure(
                    &request,
                    request.command_version.unwrap_or(0),
                    "COMMAND_UNKNOWN",
                    format!("unknown command {}", request.command),
                );
                return self.finalize(
                    &request,
                    response,
                    "unknown",
                    "unknown",
                    authority,
                    started,
                );
            }
            Err(ResolveError::VersionIncompatible { supported }) => {
                let response = self.failure(
                    &request,
                    request.command_version.unwrap_or(0),
                    "COMMAND_VERSION_INCOMPATIBLE",
                    format!(
                        "unsupported version for {}; supported: {:?}",
                        request.command, supported
                    ),
                );
                return self.finalize(
                    &request,
                    response,
                    "unknown",
                    "unknown",
                    authority,
                    started,
                );
            }
        };

        if let Err(error) =
            registry::validate_value(&spec.arguments_schema, &request.arguments)
        {
            let response = self.failure(
                &request,
                spec.version,
                "VALIDATION_FAILED",
                format!("{} {}", error.path, error.message),
            );
            return self.finalize(
                &request,
                response,
                &spec.effect_class,
                &spec.permission,
                authority,
                started,
            );
        }

        if let Err(error) = self.authorize_preflight(
            &request,
            &spec.effect_class,
            &spec.permission,
            authority,
        ) {
            let response = self.failure(
                &request,
                spec.version,
                error.code,
                error.message,
            );
            return self.finalize(
                &request,
                response,
                &spec.effect_class,
                &spec.permission,
                authority,
                started,
            );
        }

        if spec.idempotency != "safe" {
            if let Some(replayed) = self.try_replay(&request, spec.version) {
                return self.finalize(
                    &request,
                    replayed,
                    &spec.effect_class,
                    &spec.permission,
                    authority,
                    started,
                );
            }
        }

        let transaction = match self.begin_transaction_if_needed(
            &request,
            &spec.effect_class,
            &spec.permission,
            authority,
        ) {
            Ok(transaction) => transaction,
            Err(error) => {
                let response = self.failure(
                    &request,
                    spec.version,
                    error.code,
                    error.message,
                );
                return self.finalize(
                    &request,
                    response,
                    &spec.effect_class,
                    &spec.permission,
                    authority,
                    started,
                );
            }
        };

        let business = self.dispatch(
            &request,
            spec.version,
            runtime,
            authority,
        );
        let mut response = match business {
            Ok(result) => {
                if let Err(error) =
                    registry::validate_value(&spec.result_schema, &result)
                {
                    self.failure(
                        &request,
                        spec.version,
                        "RESULT_SCHEMA_VIOLATION",
                        format!("{} {}", error.path, error.message),
                    )
                } else {
                    CommandResponse::success(
                        &request,
                        spec.version,
                        self.producer.clone(),
                        result,
                    )
                }
            }
            Err(error) => self.failure(
                &request,
                spec.version,
                error.code,
                error.message,
            ),
        };

        if response.ok
            && spec.idempotency != "safe"
            && request.idempotency_key.is_some()
        {
            response = self.persist_idempotency(
                &request,
                spec.version,
                response,
            );
        }

        response = self.finish_transaction_if_needed(
            &request,
            response,
            transaction.as_deref(),
        );
        response = self.enforce_error_contract(
            &request,
            spec,
            response,
        );

        self.finalize(
            &request,
            response,
            &spec.effect_class,
            &spec.permission,
            authority,
            started,
        )
    }
}

#[derive(Debug)]
struct CoreCommandError {
    code: &'static str,
    message: String,
}

impl CoreCommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl RelayCore {
    fn failure(
        &self,
        request: &CommandRequest,
        command_version: u32,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> CommandResponse {
        CommandResponse::failure(
            request,
            command_version,
            self.producer.clone(),
            code,
            message,
        )
    }

    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&RelayStorage) -> Result<T, StorageError>,
    ) -> Result<T, CoreCommandError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            CoreCommandError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage is unavailable",
            )
        })?;
        let guard = storage.lock().map_err(|_| {
            CoreCommandError::new(
                "STORAGE_UNAVAILABLE",
                "RELAY operational storage lock is unavailable",
            )
        })?;
        operation(&guard).map_err(|error| {
            CoreCommandError::new(error.code, error.message)
        })
    }

    fn storage_health(&self) -> StorageHealth {
        match &self.storage {
            Some(storage) => storage
                .lock()
                .map_err(|_| ())
                .and_then(|storage| storage.integrity().map_err(|_| ()))
                .unwrap_or_else(|_| {
                    StorageHealth::unavailable(
                        "storage health check unavailable",
                    )
                }),
            None => self.storage_fallback.clone(),
        }
    }

    fn diagnostics_health(&self) -> DiagnosticHealth {
        match &self.diagnostics {
            Some(logger) => logger
                .lock()
                .map(|logger| logger.health())
                .unwrap_or_else(|_| {
                    DiagnosticHealth::unavailable(
                        "diagnostics health check unavailable",
                    )
                }),
            None => self.diagnostics_fallback.clone(),
        }
    }
    fn provenance(&self, request: &CommandRequest) -> Value {
        json!({
            "source": "relay-core",
            "request_id": request.request_id,
            "command": request.command,
            "actor_id": request.context.actor_id,
            "client_id": request.context.client_id,
            "delegator_id": request.context.delegator_id
        })
    }

    fn trust_label(&self, request: &CommandRequest) -> &'static str {
        if request.context.actor_id.is_some() {
            "local-attributed"
        } else {
            "local-unattributed"
        }
    }

    fn authorize_preflight(
        &self,
        request: &CommandRequest,
        effect_class: &str,
        permission: &str,
        authority: &ExecutionAuthority,
    ) -> Result<(), CoreCommandError> {
        authority
            .require_permission(permission)
            .map_err(|error| {
                CoreCommandError::new(error.code, error.message)
            })?;
        authority
            .require_effect(effect_class)
            .map_err(|error| {
                CoreCommandError::new(error.code, error.message)
            })?;

        let project_id = match request.command.as_str() {
            "project.register" | "project.import" => {
                request.arguments.get("id").and_then(Value::as_str)
            }
            "result.put" | "job.checkpoint" => {
                request
                    .arguments
                    .get("project_id")
                    .and_then(Value::as_str)
            }
            _ => request
                .arguments
                .get("project_id")
                .and_then(Value::as_str),
        };

        if project_id.is_some()
            || (authority.project_ids.is_some()
                && matches!(
                    request.command.as_str(),
                    "project.register" | "project.import"
                ))
        {
            authority.require_project(project_id).map_err(|error| {
                CoreCommandError::new(error.code, error.message)
            })?;
        }
        Ok(())
    }

    fn transactional_effect(effect_class: &str) -> bool {
        matches!(
            effect_class,
            "relay_state_write"
                | "project_write"
                | "destructive"
                | "publish_external"
                | "credential_change"
        )
    }

    fn transaction_project_id<'a>(
        request: &'a CommandRequest,
    ) -> Option<&'a str> {
        if matches!(
            request.command.as_str(),
            "project.register" | "project.import"
        ) {
            return None;
        }
        request
            .arguments
            .get("project_id")
            .and_then(Value::as_str)
    }

    fn begin_transaction_if_needed(
        &self,
        request: &CommandRequest,
        effect_class: &str,
        permission: &str,
        authority: &ExecutionAuthority,
    ) -> Result<Option<String>, CoreCommandError> {
        if !Self::transactional_effect(effect_class) {
            return Ok(None);
        }
        let request_sha256 =
            request_fingerprint(request, request.command_version.unwrap_or(1));
        let intended_summary = format!(
            "{} requested through validated command contract",
            request.command
        );
        let record = self.with_storage(|storage| {
            storage.begin_transaction(NewTransaction {
                project_id: Self::transaction_project_id(request),
                request_id: &request.request_id,
                command: &request.command,
                effect_class,
                permission,
                actor_id: &authority.actor_id,
                client_id: &authority.client_id,
                delegator_id: authority.delegator_id.as_deref(),
                request_sha256: &request_sha256,
                intended_summary: &intended_summary,
                rollback_status: "not_available",
            })
        })?;
        Ok(Some(record.id))
    }

    fn finish_transaction_if_needed(
        &self,
        request: &CommandRequest,
        response: CommandResponse,
        transaction_id: Option<&str>,
    ) -> CommandResponse {
        let Some(transaction_id) = transaction_id else {
            return response;
        };

        let after_ref = response
            .result
            .as_ref()
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str);
        let result_id = if request.command == "result.put" && response.ok {
            after_ref
        } else {
            None
        };
        let job_id = if request.command == "job.checkpoint" && response.ok {
            after_ref
        } else {
            None
        };
        let state = if response.ok { "COMPLETED" } else { "FAILED" };
        let verification = if response.ok { "verified" } else { "failed" };

        let project_id = if matches!(
            request.command.as_str(),
            "project.register" | "project.import"
        ) && response.ok
        {
            after_ref
        } else {
            Self::transaction_project_id(request)
        };

        match self.with_storage(|storage| {
            storage.finish_transaction(
                transaction_id,
                state,
                after_ref,
                verification,
                result_id,
                job_id,
                project_id,
            )
        }) {
            Ok(_) => response,
            Err(error) => self.failure(
                request,
                response.command_version,
                "TRANSACTION_RECORD_INCOMPLETE",
                format!(
                    "command outcome could not be durably finalized: {}",
                    error.message
                ),
            ),
        }
    }

    fn enforce_error_contract(
        &self,
        request: &CommandRequest,
        spec: &registry::CommandSpec,
        response: CommandResponse,
    ) -> CommandResponse {
        if response.ok {
            return response;
        }
        let Some(error) = response.error.as_ref() else {
            return self.failure(
                request,
                response.command_version,
                "INTERNAL_CONTRACT_VIOLATION",
                "failed response did not contain an error envelope",
            );
        };
        let error_value = match serde_json::to_value(error) {
            Ok(value) => value,
            Err(serialize_error) => {
                return self.failure(
                    request,
                    response.command_version,
                    "INTERNAL_CONTRACT_VIOLATION",
                    format!(
                        "error envelope could not be serialized: {serialize_error}"
                    ),
                );
            }
        };
        if let Err(validation) = registry::validate_value(
            &registry::registry().error_schema,
            &error_value,
        ) {
            return self.failure(
                request,
                response.command_version,
                "INTERNAL_CONTRACT_VIOLATION",
                format!(
                    "error envelope violated registry schema: {} {}",
                    validation.path, validation.message
                ),
            );
        }
        if !registry::is_error_allowed(spec, &error.code) {
            return self.failure(
                request,
                response.command_version,
                "INTERNAL_CONTRACT_VIOLATION",
                format!(
                    "command {} emitted undeclared error {}",
                    spec.id, error.code
                ),
            );
        }
        response
    }

    fn usage_project_id(
        request: &CommandRequest,
        response: &CommandResponse,
    ) -> Option<String> {
        if let Some(project_id) = request
            .arguments
            .get("project_id")
            .and_then(Value::as_str)
        {
            return Some(project_id.to_string());
        }
        if matches!(
            request.command.as_str(),
            "project.register" | "project.import"
        ) && response.ok {
            return response
                .result
                .as_ref()
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        response
            .result
            .as_ref()
            .and_then(|value| value.get("project_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn finalize(
        &self,
        request: &CommandRequest,
        response: CommandResponse,
        effect_class: &str,
        permission: &str,
        authority: &ExecutionAuthority,
        started: Instant,
    ) -> CommandResponse {
        let request_bytes = serde_json::to_vec(request)
            .map(|value| value.len() as u64)
            .unwrap_or(0);
        let response_bytes = serde_json::to_vec(&response)
            .map(|value| value.len() as u64)
            .unwrap_or(0);
        let elapsed_us = started.elapsed().as_micros() as u64;
        let elapsed_ms = elapsed_us.saturating_add(999) / 1000;
        let project_id = Self::usage_project_id(request, &response);

        if let Some(storage) = &self.storage {
            if let Ok(storage) = storage.lock() {
                let _ = storage.record_usage(UsageMetricInput {
                    request_id: &request.request_id,
                    command: &request.command,
                    command_version: response.command_version,
                    actor_id: &authority.actor_id,
                    client_id: &authority.client_id,
                    delegator_id: authority.delegator_id.as_deref(),
                    project_id: project_id.as_deref(),
                    effect_class,
                    permission,
                    ok: response.ok,
                    replayed: response.replayed,
                    elapsed_ms,
                    request_bytes,
                    response_bytes,
                    remote_calls: 0,
                    model_tokens_in: 0,
                    model_tokens_out: 0,
                });
            }
        }
        self.finish(request, response)
    }

    fn capabilities(&self, runtime: &RuntimeContext) -> Vec<String> {
        let mut capabilities = registry::capability_ids();
        capabilities.extend(runtime.capabilities.iter().cloned());
        capabilities.sort();
        capabilities.dedup();
        capabilities
    }

    fn dispatch(
        &self,
        request: &CommandRequest,
        command_version: u32,
        runtime: &RuntimeContext,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        match request.command.as_str() {
            "system.status" => self.status(runtime),
            "system.doctor" => self.doctor(runtime),
            "system.echo" => Ok(json!({
                "echo": request.arguments
            })),
            "system.shutdown" => Ok(json!({
                "shutting_down": true
            })),
            "registry.list" => self.registry_list(&request.arguments),
            "registry.describe" => {
                self.registry_describe(&request.arguments)
            }
            "storage.integrity" => {
                let health = self.with_storage(|storage| {
                    storage.integrity()
                })?;
                serde_json::to_value(health).map_err(|error| {
                    CoreCommandError::new(
                        "STORAGE_ERROR",
                        format!("serialize storage health: {error}"),
                    )
                })
            }
            "project.register" => {
                self.project_register(&request.arguments)
            }
            "project.import" => {
                self.project_import(&request.arguments)
            }
            "project.list" => self.project_list(authority),
            "project.index.build" => {
                self.project_index_build(&request.arguments)
            }
            "project.index.reconcile" => {
                self.project_index_reconcile(&request.arguments)
            }
            "project.capabilities" => {
                self.project_capabilities(&request.arguments)
            }
            "project.changes" => {
                self.project_changes(&request.arguments)
            }
            "project.dependencies.replace" => {
                self.project_dependencies_replace(&request.arguments)
            }
            "project.dependencies.list" => {
                self.project_dependencies_list(&request.arguments)
            }
            "result.put" => self.result_put(request),
            "result.get" => self.result_get(&request.arguments, authority),
            "job.checkpoint" => self.job_checkpoint(request),
            "job.get" => self.job_get(&request.arguments, authority),
            "transaction.list" => {
                self.transaction_list(&request.arguments, authority)
            }
            "usage.summary" => self.usage_summary(),
            "policy.egress.check" => {
                self.policy_egress_check(request, authority)
            }
            other => Err(CoreCommandError::new(
                "COMMAND_UNKNOWN",
                format!(
                    "command {other}@{command_version} is not implemented"
                ),
            )),
        }
    }

    fn status(
        &self,
        runtime: &RuntimeContext,
    ) -> Result<Value, CoreCommandError> {
        let storage = self.storage_health();
        let diagnostics = self.diagnostics_health();
        let healthy = storage.ok && diagnostics.ok && runtime.ipc_healthy;
        Ok(json!({
            "process_health": "running",
            "recovery_state": if healthy { "Healthy" } else { "Degraded" },
            "pid": runtime.pid,
            "version": self.producer.version,
            "runtime": runtime.runtime,
            "protocol": {
                "min": PROTOCOL_MIN,
                "max": PROTOCOL_MAX,
                "negotiated": PROTOCOL_MAX
            },
            "capabilities": self.capabilities(runtime),
            "ipc_security": runtime.ipc_security,
            "storage": storage,
            "diagnostics": diagnostics,
            "uptime_ms": runtime.uptime_ms
        }))
    }
    fn doctor(
        &self,
        runtime: &RuntimeContext,
    ) -> Result<Value, CoreCommandError> {
        let storage = self.storage_health();
        let diagnostics = self.diagnostics_health();
        let healthy = storage.ok && diagnostics.ok && runtime.ipc_healthy;
        Ok(json!({
            "healthy": healthy,
            "summary": if healthy {
                "RELAY Core, storage, diagnostics, and local transport are healthy."
            } else if !storage.ok {
                "RELAY Core is reachable, but operational storage needs attention."
            } else if !diagnostics.ok {
                "RELAY Core is reachable, but diagnostics capture needs attention."
            } else {
                "RELAY Core is healthy, but local transport needs attention."
            },
            "checks": [
                {
                    "id": "core.process",
                    "status": "pass",
                    "detail": format!("PID {}", runtime.pid)
                },
                {
                    "id": "storage.integrity",
                    "status": if storage.ok { "pass" } else { "fail" },
                    "detail": if storage.ok {
                        format!("SQLite quick_check: {}", storage.check)
                    } else {
                        storage.error.clone().unwrap_or_else(|| {
                            "storage unavailable".to_string()
                        })
                    }
                },
                {
                    "id": "diagnostics.capture",
                    "status": if diagnostics.ok { "pass" } else { "fail" },
                    "detail": if diagnostics.ok {
                        "bounded structured diagnostics available".to_string()
                    } else {
                        diagnostics.last_error.clone().unwrap_or_else(|| {
                            "diagnostics unavailable".to_string()
                        })
                    }
                },
                {
                    "id": "transport.local",
                    "status": if runtime.ipc_healthy { "pass" } else { "fail" },
                    "detail": if runtime.ipc_healthy {
                        "local transport healthy".to_string()
                    } else {
                        "local transport degraded".to_string()
                    }
                }
            ],
            "next_action": if healthy {
                Value::Null
            } else if !storage.ok {
                Value::String(
                    "Protect and inspect operational storage before writes."
                        .to_string()
                )
            } else if !diagnostics.ok {
                Value::String(
                    "Restore diagnostics capture before treating support evidence as complete."
                        .to_string()
                )
            } else {
                Value::String(
                    "Inspect local transport security/connectivity."
                        .to_string()
                )
            }
        }))
    }

    fn registry_list(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let surface = arguments.get("surface").and_then(Value::as_str);
        let prefix = arguments.get("prefix").and_then(Value::as_str);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(100)
            .clamp(1, 200) as usize;
        Ok(registry::compact_list(surface, prefix, limit))
    }

    fn registry_describe(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreCommandError::new(
                    "VALIDATION_FAILED",
                    "command is required",
                )
            })?;
        let version = arguments
            .get("version")
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        registry::describe(command, version).map_err(|error| {
            match error {
                ResolveError::UnknownCommand => CoreCommandError::new(
                    "COMMAND_UNKNOWN",
                    format!("unknown command {command}"),
                ),
                ResolveError::VersionIncompatible { supported } => {
                    CoreCommandError::new(
                        "COMMAND_VERSION_INCOMPATIBLE",
                        format!("supported versions: {supported:?}"),
                    )
                }
            }
        })
    }
    fn project_register(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let name = arguments["name"]
            .as_str()
            .expect("registry validation requires name");
        let root_uri = arguments["root_uri"]
            .as_str()
            .expect("registry validation requires root_uri");
        let id = arguments.get("id").and_then(Value::as_str);
        let project = self.with_storage(|storage| {
            storage.register_project(id, name, root_uri)
        })?;
        serde_json::to_value(project).map_err(|error| {
            CoreCommandError::new(
                "STORAGE_ERROR",
                format!("serialize project: {error}"),
            )
        })
    }

    fn project_import(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let name = arguments["name"]
            .as_str()
            .expect("registry validation requires name");
        let root_path = arguments["root_path"]
            .as_str()
            .expect("registry validation requires root_path");
        let id = arguments.get("id").and_then(Value::as_str);
        let root = indexing::canonical_project_root(root_path)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let root_uri = root.to_string_lossy().to_string();
        let project = self.with_storage(|storage| {
            storage.register_project(id, name, &root_uri)
        })?;
        Ok(json!({
            "id": project.id,
            "name": project.name,
            "root_canonicalized": true,
            "baseline_state": "missing"
        }))
    }

    fn project_list(
        &self,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let mut projects = self.with_storage(|storage| {
            storage.list_projects()
        })?;
        if let Some(allowed) = &authority.project_ids {
            projects.retain(|project| allowed.contains(&project.id));
        }
        for project in &mut projects {
            if Path::new(&project.root_uri).is_absolute() {
                project.root_uri = format!("local-project:{}", project.id);
            }
        }
        Ok(json!({ "projects": projects }))
    }

    fn project_index_build(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let project = self.with_storage(|storage| {
            storage.get_project(project_id)
        })?.ok_or_else(|| {
            CoreCommandError::new("PROJECT_NOT_FOUND", "project not found")
        })?;
        let root = indexing::canonical_project_root(&project.root_uri)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let plan = indexing::build_baseline(&root)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let state = self.with_storage(|storage| {
            storage.replace_project_baseline(
                project_id,
                &plan.files,
                plan.stats.total_bytes,
            )
        })?;
        Ok(json!({
            "project_id": project_id,
            "generation": state.generation,
            "file_count": state.file_count,
            "total_bytes": state.total_bytes,
            "files_hashed": plan.stats.files_hashed,
            "symlinks_skipped": plan.stats.symlinks_skipped,
            "elapsed_ms": plan.stats.elapsed_ms,
            "mode": "baseline"
        }))
    }

    fn project_index_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let hints: Vec<String> = arguments
            .get("hints")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let (project, index_state, previous) = self.with_storage(|storage| {
            let project = storage
                .get_project(project_id)?
                .ok_or_else(|| {
                    StorageError::new(
                        "PROJECT_NOT_FOUND",
                        "project not found",
                    )
                })?;
            let state = storage
                .get_project_index_state(project_id)?
                .ok_or_else(|| {
                    StorageError::new(
                        "INDEX_BASELINE_MISSING",
                        "project baseline is missing",
                    )
                })?;
            let files = storage.list_project_files(project_id)?;
            Ok((project, state, files))
        })?;

        let root = indexing::canonical_project_root(&project.root_uri)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let plan = indexing::reconcile(&root, &previous, &hints)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let state = self.with_storage(|storage| {
            storage.apply_project_reconciliation(
                project_id,
                index_state.generation,
                &plan.touched_files,
                &plan.changes,
                plan.files.len(),
                plan.stats.total_bytes,
            )
        })?;

        Ok(json!({
            "project_id": project_id,
            "generation": state.generation,
            "file_count": state.file_count,
            "total_bytes": state.total_bytes,
            "files_hashed": plan.stats.files_hashed,
            "files_unchanged": plan.stats.files_unchanged,
            "hint_count": plan.stats.hint_count,
            "hint_hits": plan.stats.hint_hits,
            "changes": plan.changes,
            "elapsed_ms": plan.stats.elapsed_ms,
            "mode": "reconcile"
        }))
    }

    fn project_capabilities(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let (project, state) = self.with_storage(|storage| {
            let project = storage
                .get_project(project_id)?
                .ok_or_else(|| {
                    StorageError::new(
                        "PROJECT_NOT_FOUND",
                        "project not found",
                    )
                })?;
            let state = storage.get_project_index_state(project_id)?;
            Ok((project, state))
        })?;

        let root_available =
            indexing::canonical_project_root(&project.root_uri).is_ok();
        let baseline_ready = state
            .as_ref()
            .map(|state| state.status == "ready")
            .unwrap_or(false);
        let baseline_state = if !root_available {
            "unavailable"
        } else if baseline_ready {
            "ready"
        } else {
            "missing"
        };

        let mut capabilities = Vec::new();
        capabilities.push(json!({
            "id": "filesystem.read",
            "state": if root_available { "available" } else { "unavailable" },
            "detail": if root_available {
                "canonical project root is readable"
            } else {
                "canonical project root is unavailable"
            }
        }));
        capabilities.push(json!({
            "id": "index.baseline",
            "state": if root_available { "available" } else { "unavailable" },
            "detail": if !root_available {
                "project root is unavailable"
            } else if baseline_ready {
                "durable baseline exists"
            } else {
                "baseline can be built when the project root is available"
            }
        }));
        capabilities.push(json!({
            "id": "index.changed_only",
            "state": if !root_available { "unavailable" } else if baseline_ready { "available" } else { "unknown" },
            "detail": if !root_available {
                "project root is unavailable"
            } else if baseline_ready {
                "metadata reconciliation hashes only new or metadata-changed files"
            } else {
                "requires a healthy baseline"
            }
        }));
        capabilities.push(json!({
            "id": "watcher.hints",
            "state": "available",
            "detail": "watcher paths are hints; authoritative reconciliation remains required"
        }));
        capabilities.push(json!({
            "id": "reconciliation",
            "state": if root_available { "available" } else { "unavailable" },
            "detail": "filesystem metadata reconciliation is authoritative"
        }));
        capabilities.push(json!({
            "id": "dependency_edges",
            "state": if !root_available { "unavailable" } else if baseline_ready { "available" } else { "unknown" },
            "detail": "project-scoped derived edges can be supplied for indexed files with generation and source-hash checks"
        }));
        capabilities.push(json!({
            "id": "dependency_graph",
            "state": "unknown",
            "detail": "automatic dependency extraction requires a compatible parser or adapter"
        }));

        Ok(json!({
            "project_id": project_id,
            "baseline_state": baseline_state,
            "capabilities": capabilities
        }))
    }

    fn project_changes(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
        let (state, changes) = self.with_storage(|storage| {
            if storage.get_project(project_id)?.is_none() {
                return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
            }
            let state = storage.get_project_index_state(project_id)?.ok_or_else(|| {
                StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
            })?;
            let after_generation = arguments
                .get("after_generation")
                .and_then(Value::as_i64)
                .unwrap_or(state.baseline_generation);
            if after_generation < state.baseline_generation {
                return Err(StorageError::new(
                    "INDEX_CONTINUITY_LOST",
                    "requested generation precedes the current baseline",
                ));
            }
            if after_generation > state.generation {
                return Err(StorageError::new(
                    "INDEX_GENERATION_CONFLICT",
                    "requested generation is newer than the current index",
                ));
            }
            let changes = storage.list_project_changes_since(
                project_id, after_generation, limit + 1,
            )?;
            Ok((state, changes))
        })?;
        if changes.len() > limit {
            return Err(CoreCommandError::new(
                "INDEX_DELTA_TOO_LARGE",
                "change delta exceeds the bounded result; rebuild or request a closer generation",
            ));
        }
        Ok(json!({
            "project_id": project_id,
            "baseline_generation": state.baseline_generation,
            "current_generation": state.generation,
            "changes": changes
        }))
    }

    fn project_dependencies_replace(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let expected_generation = arguments["expected_generation"]
            .as_i64()
            .expect("registry validation requires expected_generation");
        let raw_source = arguments["source_path"]
            .as_str()
            .expect("registry validation requires source_path");
        let source_path = indexing::normalize_project_relative_path(raw_source)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let source_sha256 = arguments["source_sha256"]
            .as_str()
            .expect("registry validation requires source_sha256");
        let producer_id = arguments["producer_id"]
            .as_str()
            .expect("registry validation requires producer_id");
        let producer_version = arguments["producer_version"]
            .as_str()
            .expect("registry validation requires producer_version");
        let raw_targets = arguments["targets"]
            .as_array()
            .expect("registry validation requires targets");
        if source_path.len() > 4096
            || source_sha256.len() != 64
            || !source_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || producer_id.len() > 128
            || producer_version.len() > 64
            || raw_targets.len() > 500
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "dependency replacement exceeds bounded field limits",
            ));
        }
        let mut targets = BTreeSet::new();
        for raw_target in raw_targets {
            let target = indexing::normalize_project_relative_path(
                raw_target.as_str().expect("registry validates targets"),
            ).map_err(|error| CoreCommandError::new(error.code, error.message))?;
            if target.len() > 4096 || target == source_path || !targets.insert(target) {
                return Err(CoreCommandError::new(
                    "VALIDATION_FAILED",
                    "dependency targets must be distinct project-relative files other than the source",
                ));
            }
        }
        let targets: Vec<String> = targets.into_iter().collect();
        let edge_count = self.with_storage(|storage| {
            storage.replace_project_dependencies(DependencyReplacement {
                project_id,
                expected_generation,
                source_path: &source_path,
                expected_source_sha256: source_sha256,
                producer_id,
                producer_version,
                targets: &targets,
            })
        })?;
        Ok(json!({
            "project_id": project_id,
            "generation": expected_generation,
            "source_path": source_path,
            "producer_id": producer_id,
            "edge_count": edge_count
        }))
    }

    fn project_dependencies_list(
        &self,
        arguments: &Value,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let source_path = arguments
            .get("source_path")
            .and_then(Value::as_str)
            .map(indexing::normalize_project_relative_path)
            .transpose()
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
        let (state, edges) = self.with_storage(|storage| {
            if storage.get_project(project_id)?.is_none() {
                return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
            }
            let state = storage.get_project_index_state(project_id)?.ok_or_else(|| {
                StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
            })?;
            let edges = storage.list_project_dependency_edges(
                project_id, source_path.as_deref(), limit + 1,
            )?;
            Ok((state, edges))
        })?;
        if edges.len() > limit {
            return Err(CoreCommandError::new(
                "INDEX_EDGE_LIMIT_EXCEEDED",
                "dependency edge result exceeds the bounded limit; filter by source path",
            ));
        }
        Ok(json!({
            "project_id": project_id,
            "generation": state.generation,
            "edges": edges
        }))
    }

    fn result_put(
        &self,
        request: &CommandRequest,
    ) -> Result<Value, CoreCommandError> {
        let project_id =
            request.arguments.get("project_id").and_then(Value::as_str);
        let kind = request
            .arguments
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("GENERIC");
        let payload = request
            .arguments
            .get("payload")
            .expect("registry validation requires payload");
        let provenance = self.provenance(request);
        let trust = self.trust_label(request);
        let result = self.with_storage(|storage| {
            storage.put_result(
                project_id,
                kind,
                payload,
                &self.producer.version,
                &provenance,
                trust,
            )
        })?;
        serde_json::to_value(result).map_err(|error| {
            CoreCommandError::new(
                "STORAGE_ERROR",
                format!("serialize result: {error}"),
            )
        })
    }

    fn result_get(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let id = arguments["result_id"]
            .as_str()
            .expect("registry validation requires result_id");
        let result = self.with_storage(|storage| {
            storage.get_result(id)
        })?;
        match result {
            Some(result) => {
                authority
                    .require_project(result.project_id.as_deref())
                    .map_err(|error| {
                        CoreCommandError::new(error.code, error.message)
                    })?;
                serde_json::to_value(result).map_err(|error| {
                    CoreCommandError::new(
                        "STORAGE_ERROR",
                        format!("serialize result: {error}"),
                    )
                })
            }
            None => Err(CoreCommandError::new(
                "RESULT_NOT_FOUND",
                "result not found",
            )),
        }
    }
    fn job_checkpoint(
        &self,
        request: &CommandRequest,
    ) -> Result<Value, CoreCommandError> {
        let arguments = &request.arguments;
        let id = arguments.get("id").and_then(Value::as_str);
        let project_id =
            arguments.get("project_id").and_then(Value::as_str);
        let command = arguments["command"]
            .as_str()
            .expect("registry validation requires command");
        let state = arguments["state"]
            .as_str()
            .expect("registry validation requires state");
        let checkpoint = arguments
            .get("checkpoint")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let result_id =
            arguments.get("result_id").and_then(Value::as_str);
        let provenance = self.provenance(request);
        let trust = self.trust_label(request);

        let job = self.with_storage(|storage| {
            storage.checkpoint_job(
                id,
                project_id,
                command,
                state,
                &checkpoint,
                result_id,
                &provenance,
                trust,
            )
        })?;
        serde_json::to_value(job).map_err(|error| {
            CoreCommandError::new(
                "STORAGE_ERROR",
                format!("serialize job: {error}"),
            )
        })
    }
    fn job_get(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let id = arguments["job_id"]
            .as_str()
            .expect("registry validation requires job_id");
        let job = self.with_storage(|storage| storage.get_job(id))?;
        match job {
            Some(job) => {
                authority
                    .require_project(job.project_id.as_deref())
                    .map_err(|error| {
                        CoreCommandError::new(error.code, error.message)
                    })?;
                serde_json::to_value(job).map_err(|error| {
                    CoreCommandError::new(
                        "STORAGE_ERROR",
                        format!("serialize job: {error}"),
                    )
                })
            }
            None => Err(CoreCommandError::new(
                "JOB_NOT_FOUND",
                "job not found",
            )),
        }
    }

    fn transaction_list(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let requested_project =
            arguments.get("project_id").and_then(Value::as_str);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .clamp(1, 200) as usize;

        if let Some(project_id) = requested_project {
            authority.require_project(Some(project_id)).map_err(|error| {
                CoreCommandError::new(error.code, error.message)
            })?;
        }

        let mut transactions = self.with_storage(|storage| {
            storage.list_transactions(requested_project, limit)
        })?;

        if requested_project.is_none() {
            if let Some(allowed) = &authority.project_ids {
                transactions.retain(|record| {
                    record
                        .project_id
                        .as_ref()
                        .map(|project| allowed.contains(project))
                        .unwrap_or(false)
                });
                transactions.truncate(limit);
            }
        }

        Ok(json!({ "transactions": transactions }))
    }

    fn usage_summary(&self) -> Result<Value, CoreCommandError> {
        let summary = self.with_storage(|storage| {
            storage.usage_summary()
        })?;
        serde_json::to_value(summary).map_err(|error| {
            CoreCommandError::new(
                "STORAGE_ERROR",
                format!("serialize usage summary: {error}"),
            )
        })
    }

    fn parse_data_class(value: &str) -> Result<DataClass, CoreCommandError> {
        match value {
            "public" => Ok(DataClass::Public),
            "project" => Ok(DataClass::Project),
            "sensitive" => Ok(DataClass::Sensitive),
            "credential" => Ok(DataClass::Credential),
            _ => Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                format!("unsupported data class {value}"),
            )),
        }
    }

    fn data_class_name(value: DataClass) -> &'static str {
        match value {
            DataClass::Public => "public",
            DataClass::Project => "project",
            DataClass::Sensitive => "sensitive",
            DataClass::Credential => "credential",
        }
    }

    fn policy_egress_check(
        &self,
        request: &CommandRequest,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let arguments = &request.arguments;
        let destination = arguments["destination"]
            .as_str()
            .expect("registry validation requires destination");
        let project_id =
            arguments.get("project_id").and_then(Value::as_str);
        if project_id.is_some() {
            authority.require_project(project_id).map_err(|error| {
                CoreCommandError::new(error.code, error.message)
            })?;
        }

        let data_classes: Vec<DataClass> = arguments["data_classes"]
            .as_array()
            .expect("registry validation requires data_classes array")
            .iter()
            .filter_map(Value::as_str)
            .map(Self::parse_data_class)
            .collect::<Result<_, _>>()?;
        if data_classes.is_empty() {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "data_classes must contain at least one class",
            ));
        }

        let modalities: Vec<String> = arguments
            .get("modalities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let source_refs: Vec<String> = arguments
            .get("source_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let approx_bytes = arguments
            .get("approx_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let approx_tokens = arguments
            .get("approx_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let credential_handle =
            arguments.get("credential_handle").and_then(Value::as_str);
        if let Some(handle_id) = credential_handle {
            authority
                .require_credential_handle(handle_id, Some("egress"))
                .map_err(|error| {
                    CoreCommandError::new(error.code, error.message)
                })?;
            let durable = self.with_storage(|storage| {
                storage.get_credential_handle(handle_id)
            })?;
            match durable {
                Some(handle) if handle.status == "active" => {}
                Some(_) => {
                    return Err(CoreCommandError::new(
                        "CREDENTIAL_HANDLE_REVOKED",
                        "credential handle is revoked",
                    ));
                }
                None => {
                    return Err(CoreCommandError::new(
                        "CREDENTIAL_HANDLE_DENIED",
                        "credential handle is not registered",
                    ));
                }
            }
        }
        let purpose = arguments["purpose"]
            .as_str()
            .expect("registry validation requires purpose");

        let egress_request = EgressRequest {
            destination: destination.to_string(),
            project_id: project_id.map(str::to_string),
            data_classes: data_classes.clone(),
            modalities: modalities.clone(),
            approx_bytes,
            source_refs: source_refs.clone(),
            purpose: purpose.to_string(),
            credential_handle: credential_handle.map(str::to_string),
        };
        let decision: EgressDecision =
            authority.evaluate_egress(&egress_request);
        let class_names: Vec<String> = data_classes
            .iter()
            .copied()
            .map(Self::data_class_name)
            .map(str::to_string)
            .collect();

        let ledger = self.with_storage(|storage| {
            storage.record_egress(EgressLedgerInput {
                project_id,
                request_id: &request.request_id,
                actor_id: &authority.actor_id,
                client_id: &authority.client_id,
                delegator_id: authority.delegator_id.as_deref(),
                destination,
                data_classes: &class_names,
                modalities: &modalities,
                source_refs: &source_refs,
                purpose,
                approx_bytes,
                approx_tokens,
                credential_handle,
                decision: if decision.allowed {
                    "allowed"
                } else {
                    "blocked"
                },
                reason: &decision.reason,
            })
        })?;

        Ok(json!({
            "allowed": decision.allowed,
            "reason": decision.reason,
            "strongest_class": Self::data_class_name(
                decision.strongest_class
            ),
            "destination": decision.destination,
            "ledger_id": ledger.id
        }))
    }

    fn try_replay(
        &self,
        request: &CommandRequest,
        command_version: u32,
    ) -> Option<CommandResponse> {
        let key = request.idempotency_key.as_deref()?;
        let fingerprint = request_fingerprint(request, command_version);

        let record = match self.with_storage(|storage| {
            storage.get_idempotency(key)
        }) {
            Ok(Some(record)) => record,
            Ok(None) => return None,
            Err(error) => {
                return Some(self.failure(
                    request,
                    command_version,
                    error.code,
                    error.message,
                ));
            }
        };

        if record.command != request.command
            || record.request_sha256 != fingerprint
        {
            return Some(self.failure(
                request,
                command_version,
                "IDEMPOTENCY_CONFLICT",
                "idempotency key was already used for a different request",
            ));
        }

        match serde_json::from_str::<CommandResponse>(
            &record.response_json,
        ) {
            Ok(mut response) => {
                response.request_id = request.request_id.clone();
                response.producer = self.producer.clone();
                response.replayed = true;
                Some(response)
            }
            Err(error) => Some(self.failure(
                request,
                command_version,
                "STORAGE_ERROR",
                format!(
                    "stored idempotency response is invalid: {error}"
                ),
            )),
        }
    }
    fn persist_idempotency(
        &self,
        request: &CommandRequest,
        command_version: u32,
        response: CommandResponse,
    ) -> CommandResponse {
        let Some(key) = request.idempotency_key.as_deref() else {
            return response;
        };
        let fingerprint = request_fingerprint(request, command_version);
        let response_json = match serde_json::to_string(&response) {
            Ok(value) => value,
            Err(error) => {
                return self.failure(
                    request,
                    command_version,
                    "IDEMPOTENCY_PERSIST_FAILED",
                    format!(
                        "serialize idempotent response: {error}"
                    ),
                );
            }
        };

        match self.with_storage(|storage| {
            storage.put_idempotency(
                key,
                &request.command,
                &fingerprint,
                &response_json,
            )
        }) {
            Ok(()) => response,
            Err(error) if error.code == "IDEMPOTENCY_CONFLICT" => {
                self.try_replay(request, command_version)
                    .unwrap_or_else(|| {
                        self.failure(
                            request,
                            command_version,
                            "IDEMPOTENCY_CONFLICT",
                            "idempotency race could not be resolved",
                        )
                    })
            }
            Err(error) => self.failure(
                request,
                command_version,
                "IDEMPOTENCY_PERSIST_FAILED",
                format!(
                    "write completed but replay metadata failed: {}",
                    error.message
                ),
            ),
        }
    }
    fn finish(
        &self,
        request: &CommandRequest,
        response: CommandResponse,
    ) -> CommandResponse {
        if let Some(logger) = &self.diagnostics {
            if let Ok(mut logger) = logger.lock() {
                let mut event = DiagnosticEvent::new(
                    "relay.command.completed",
                    if response.ok {
                        Severity::Info
                    } else {
                        Severity::Warn
                    },
                    "core.command",
                    "RELAY command completed",
                );
                event.correlation_id = Some(request.request_id.clone());
                event.attributes.insert(
                    "command".to_string(),
                    json!(request.command),
                );
                event.attributes.insert(
                    "command_version".to_string(),
                    json!(response.command_version),
                );
                event.attributes.insert(
                    "ok".to_string(),
                    json!(response.ok),
                );
                event.attributes.insert(
                    "replayed".to_string(),
                    json!(response.replayed),
                );
                if let Some(error) = &response.error {
                    event.attributes.insert(
                        "error_code".to_string(),
                        json!(error.code),
                    );
                }
                let _ = logger.append(event);
            }
        }
        response
    }

    pub fn flush_diagnostics(&self) {
        if let Some(logger) = &self.diagnostics {
            if let Ok(mut logger) = logger.lock() {
                let _ = logger.flush();
            }
        }
    }
}
fn request_fingerprint(
    request: &CommandRequest,
    command_version: u32,
) -> String {
    let canonical = canonical_json(&json!({
        "command": request.command,
        "command_version": command_version,
        "arguments": request.arguments,
        "context": request.context
    }));
    let encoded = serde_json::to_vec(&canonical)
        .expect("canonical JSON must serialize");
    let digest = Sha256::digest(encoded);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}")
            .expect("writing to String cannot fail");
    }
    output
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let mut output = Map::new();
            for key in keys {
                output.insert(
                    key.clone(),
                    canonical_json(&object[key]),
                );
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(
            items.iter().map(canonical_json).collect(),
        ),
        other => other.clone(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use relay_contracts::RequestContext;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "relay-core-service-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn request(
        id: &str,
        command: &str,
        arguments: Value,
    ) -> CommandRequest {
        CommandRequest {
            request_id: id.to_string(),
            command: command.to_string(),
            command_version: Some(1),
            arguments,
            idempotency_key: None,
            context: RequestContext {
                actor_id: Some("ACTOR-fixture".to_string()),
                client_id: Some("CLIENT-test".to_string()),
                delegator_id: Some("USER-fixture".to_string()),
            },
        }
    }

    fn runtime() -> RuntimeContext {
        RuntimeContext::in_process()
    }
    fn register_project(core: &RelayCore) -> String {
        let mut req = request(
            "REQ-project",
            "project.register",
            json!({
                "id": "PRJ-fixture",
                "name": "Fixture",
                "root_uri": "file:///fixture"
            }),
        );
        req.idempotency_key = Some("IDEMP-project".to_string());
        let response = core.execute(req, &runtime());
        assert!(response.ok, "{response:?}");
        response.result.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn validation_happens_before_business_logic() {
        let dir = temp_dir("validation");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let response = core.execute(
            request(
                "REQ-invalid",
                "project.register",
                json!({ "root_uri": "file:///fixture" }),
            ),
            &runtime(),
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().unwrap().code,
            "VALIDATION_FAILED"
        );

        let list = core.execute(
            request("REQ-list", "project.list", json!({})),
            &runtime(),
        );
        assert_eq!(
            list.result.unwrap()["projects"].as_array().unwrap().len(),
            0
        );
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn durable_result_and_job_include_provenance_and_trust() {
        let dir = temp_dir("provenance");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);

        let mut put = request(
            "REQ-result",
            "result.put",
            json!({
                "project_id": project_id,
                "kind": "TEST",
                "payload": { "value": 42 }
            }),
        );
        put.idempotency_key = Some("IDEMP-result".to_string());
        let result_response = core.execute(put, &runtime());
        assert!(result_response.ok);
        let result = result_response.result.unwrap();
        assert_eq!(
            result["provenance"]["actor_id"],
            "ACTOR-fixture"
        );
        assert_eq!(result["trust"], "local-attributed");
        let result_id = result["id"].as_str().unwrap().to_string();

        let mut checkpoint = request(
            "REQ-job",
            "job.checkpoint",
            json!({
                "project_id": "PRJ-fixture",
                "command": "fixture.work",
                "state": "CHECKPOINTED",
                "checkpoint": { "stage": 2 },
                "result_id": result_id
            }),
        );
        checkpoint.idempotency_key =
            Some("IDEMP-job".to_string());
        let job_response = core.execute(checkpoint, &runtime());
        assert!(job_response.ok);
        let job = job_response.result.unwrap();
        assert_eq!(
            job["provenance"]["client_id"],
            "CLIENT-test"
        );
        assert_eq!(job["trust"], "local-attributed");

        drop(core);
        let reopened = RelayCore::open(CoreConfig::new(&dir));
        let result_get = reopened.execute(
            request(
                "REQ-get-result",
                "result.get",
                json!({ "result_id": result_id }),
            ),
            &runtime(),
        );
        assert_eq!(
            result_get.result.unwrap()["payload"]["value"],
            42
        );
        let job_id = job["id"].as_str().unwrap();
        let job_get = reopened.execute(
            request(
                "REQ-get-job",
                "job.get",
                json!({ "job_id": job_id }),
            ),
            &runtime(),
        );
        assert_eq!(
            job_get.result.unwrap()["checkpoint"]["stage"],
            2
        );
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn same_idempotency_key_replays_same_result_id() {
        let dir = temp_dir("replay");
        let core = RelayCore::open(CoreConfig::new(&dir));
        register_project(&core);

        let make = |request_id: &str, value: i64| {
            let mut req = request(
                request_id,
                "result.put",
                json!({
                    "project_id": "PRJ-fixture",
                    "kind": "TEST",
                    "payload": { "value": value }
                }),
            );
            req.idempotency_key =
                Some("IDEMP-replay".to_string());
            req
        };

        let first = core.execute(make("REQ-first", 7), &runtime());
        assert!(first.ok);
        assert!(!first.replayed);
        let first_id = first.result.as_ref().unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let replay = core.execute(make("REQ-second", 7), &runtime());
        assert!(replay.ok);
        assert!(replay.replayed);
        assert_eq!(replay.request_id, "REQ-second");
        assert_eq!(
            replay.result.unwrap()["id"].as_str().unwrap(),
            first_id
        );

        let conflict = core.execute(
            make("REQ-conflict", 8),
            &runtime(),
        );
        assert!(!conflict.ok);
        assert_eq!(
            conflict.error.unwrap().code,
            "IDEMPOTENCY_CONFLICT"
        );
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn storage_and_diagnostics_health_degrade_independently() {
        let bad_storage_dir = temp_dir("bad-storage");
        fs::create_dir_all(&bad_storage_dir).unwrap();
        fs::write(
            bad_storage_dir.join("relay.sqlite3"),
            b"not sqlite",
        )
        .unwrap();
        let core = RelayCore::open(CoreConfig::new(&bad_storage_dir));
        let status = core.execute(
            request("REQ-status", "system.status", json!({})),
            &runtime(),
        );
        let result = status.result.unwrap();
        assert_eq!(result["recovery_state"], "Degraded");
        assert_eq!(result["storage"]["ok"], false);
        assert_eq!(result["diagnostics"]["ok"], true);
        let blocked = core.execute(
            request(
                "REQ-write",
                "project.register",
                json!({
                    "name": "Blocked",
                    "root_uri": "file:///blocked"
                }),
            ),
            &runtime(),
        );
        assert_eq!(
            blocked.error.unwrap().code,
            "STORAGE_UNAVAILABLE"
        );
        drop(core);
        fs::remove_dir_all(bad_storage_dir).unwrap();

        let bad_diag_dir = temp_dir("bad-diagnostics");
        fs::create_dir_all(&bad_diag_dir).unwrap();
        fs::write(
            bad_diag_dir.join("diagnostics"),
            b"blocks directory",
        )
        .unwrap();
        let core = RelayCore::open(CoreConfig::new(&bad_diag_dir));
        let status = core.execute(
            request("REQ-status-2", "system.status", json!({})),
            &runtime(),
        );
        let result = status.result.unwrap();
        assert_eq!(result["storage"]["ok"], true);
        assert_eq!(result["diagnostics"]["ok"], false);
        assert_eq!(result["recovery_state"], "Degraded");

        let project = core.execute(
            request(
                "REQ-project-2",
                "project.register",
                json!({
                    "id": "PRJ-still-works",
                    "name": "Still works",
                    "root_uri": "file:///fixture"
                }),
            ),
            &runtime(),
        );
        assert!(project.ok);
        drop(core);
        fs::remove_dir_all(bad_diag_dir).unwrap();
    }

    #[test]
    fn trusted_authority_blocks_ungranted_writes_and_identity_spoofing() {
        let dir = temp_dir("authority");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();

        let mut denied_authority = ExecutionAuthority::local_user("CLIENT-read");
        denied_authority.permissions.remove("state_write");
        denied_authority.actor_id = "AGENT-readonly".to_string();
        denied_authority.delegator_id = Some("USER-owner".to_string());

        let denied = core.execute_authorized(
            request(
                "REQ-denied",
                "project.register",
                json!({
                    "id": "PRJ-denied",
                    "name": "Denied",
                    "root_uri": "file:///denied"
                }),
            ),
            &runtime,
            &denied_authority,
        );
        assert!(!denied.ok);
        assert_eq!(denied.error.unwrap().code, "PERMISSION_DENIED");

        let mut effect_denied =
            ExecutionAuthority::local_user("CLIENT-effect");
        effect_denied.effect_classes.remove("relay_state_write");
        let denied = core.execute_authorized(
            request(
                "REQ-effect-denied",
                "project.register",
                json!({
                    "id": "PRJ-effect-denied",
                    "name": "Effect denied",
                    "root_uri": "file:///effect-denied"
                }),
            ),
            &runtime,
            &effect_denied,
        );
        assert!(!denied.ok);
        assert_eq!(denied.error.unwrap().code, "EFFECT_NOT_ALLOWED");

        let mut authority = ExecutionAuthority::local_user("CLIENT-trusted");
        authority.actor_id = "AGENT-trusted".to_string();
        authority.delegator_id = Some("USER-owner".to_string());

        let mut spoofed = request(
            "REQ-trusted",
            "project.register",
            json!({
                "id": "PRJ-trusted",
                "name": "Trusted",
                "root_uri": "file:///trusted"
            }),
        );
        spoofed.context.actor_id = Some("ATTACKER".to_string());
        spoofed.context.client_id = Some("CLIENT-attacker".to_string());
        spoofed.context.delegator_id = Some("ATTACKER-owner".to_string());
        spoofed.idempotency_key = Some("IDEMP-trusted".to_string());

        let response = core.execute_authorized(
            spoofed,
            &runtime,
            &authority,
        );
        assert!(response.ok);

        let tx = core.execute_authorized(
            request(
                "REQ-tx-list",
                "transaction.list",
                json!({ "project_id": "PRJ-trusted" }),
            ),
            &runtime,
            &authority,
        );
        let transactions = tx.result.unwrap()["transactions"]
            .as_array()
            .unwrap()
            .clone();
        let create = transactions
            .iter()
            .find(|item| item["command"] == "project.register")
            .unwrap();
        assert_eq!(create["actor_id"], "AGENT-trusted");
        assert_eq!(create["client_id"], "CLIENT-trusted");
        assert_eq!(create["delegator_id"], "USER-owner");

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn project_scope_filters_lists_and_blocks_opaque_cross_project_reads() {
        let dir = temp_dir("project-scope");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();

        for id in ["PRJ-a", "PRJ-b"] {
            let mut register = request(
                &format!("REQ-{id}"),
                "project.register",
                json!({
                    "id": id,
                    "name": id,
                    "root_uri": format!("file:///{id}")
                }),
            );
            register.idempotency_key = Some(format!("IDEMP-{id}"));
            assert!(core.execute(register, &runtime).ok);
        }

        let mut put = request(
            "REQ-b-result",
            "result.put",
            json!({
                "project_id": "PRJ-b",
                "kind": "TEST",
                "payload": { "private": "b" }
            }),
        );
        put.idempotency_key = Some("IDEMP-b-result".to_string());
        let result = core.execute(put, &runtime);
        assert!(result.ok);
        let result_id = result.result.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let mut authority =
            ExecutionAuthority::local_user("CLIENT-scoped");
        authority.project_ids =
            Some(["PRJ-a".to_string()].into_iter().collect());

        let projects = core.execute_authorized(
            request("REQ-list-scoped", "project.list", json!({})),
            &runtime,
            &authority,
        );
        let values = projects.result.unwrap()["projects"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["id"], "PRJ-a");

        let denied = core.execute_authorized(
            request(
                "REQ-cross-read",
                "result.get",
                json!({ "result_id": result_id }),
            ),
            &runtime,
            &authority,
        );
        assert!(!denied.ok);
        assert_eq!(
            denied.error.unwrap().code,
            "PROJECT_SCOPE_DENIED"
        );

        let mut cross_write = request(
            "REQ-cross-write",
            "result.put",
            json!({
                "project_id": "PRJ-b",
                "kind": "TEST",
                "payload": { "blocked": true }
            }),
        );
        cross_write.idempotency_key =
            Some("IDEMP-cross-write".to_string());
        let denied_write = core.execute_authorized(
            cross_write,
            &runtime,
            &authority,
        );
        assert!(!denied_write.ok);
        assert_eq!(
            denied_write.error.unwrap().code,
            "PROJECT_SCOPE_DENIED"
        );

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn project_scope_blocks_cross_project_index_commands() {
        let dir = temp_dir("phase3-index-scope");
        let root_a = dir.join("alpha");
        let root_b = dir.join("bravo");
        fs::create_dir_all(&root_a).unwrap();
        fs::create_dir_all(&root_b).unwrap();
        fs::write(root_a.join("same.txt"), b"alpha").unwrap();
        fs::write(root_b.join("same.txt"), b"bravo").unwrap();
        let core = RelayCore::open(CoreConfig::new(dir.join("state")));
        let runtime = runtime();

        for (id, root) in [("PRJ-a", &root_a), ("PRJ-b", &root_b)] {
            let mut import = request(
                &format!("REQ-import-{id}"),
                "project.import",
                json!({
                    "id": id,
                    "name": id,
                    "root_path": root.to_string_lossy()
                }),
            );
            import.idempotency_key = Some(format!("IDEMP-import-{id}"));
            assert!(core.execute(import, &runtime).ok);

            let mut build = request(
                &format!("REQ-build-{id}"),
                "project.index.build",
                json!({ "project_id": id }),
            );
            build.idempotency_key = Some(format!("IDEMP-build-{id}"));
            assert!(core.execute(build, &runtime).ok);
        }

        let mut authority = ExecutionAuthority::local_user("CLIENT-scoped");
        authority.project_ids = Some(["PRJ-a".to_string()].into_iter().collect());
        for command in [
            "project.capabilities",
            "project.changes",
            "project.dependencies.list",
            "project.index.build",
            "project.index.reconcile",
        ] {
            let mut attempt = request(
                &format!("REQ-deny-{command}"),
                command,
                json!({ "project_id": "PRJ-b" }),
            );
            if matches!(command, "project.index.build" | "project.index.reconcile") {
                attempt.idempotency_key = Some(format!("IDEMP-deny-{command}"));
            }
            let denied = core.execute_authorized(attempt, &runtime, &authority);
            assert!(!denied.ok, "{command} crossed project scope");
            assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        }
        let mut replace = request(
            "REQ-deny-dependency-replace",
            "project.dependencies.replace",
            json!({
                "project_id": "PRJ-b",
                "expected_generation": 1,
                "source_path": "same.txt",
                "source_sha256": "0".repeat(64),
                "producer_id": "fixture",
                "producer_version": "1",
                "targets": []
            }),
        );
        replace.idempotency_key = Some("IDEMP-deny-dependency-replace".to_string());
        let denied = core.execute_authorized(replace, &runtime, &authority);
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn change_deltas_and_dependency_edges_follow_index_generation() {
        let dir = temp_dir("phase3-dependencies");
        let root = dir.join("project");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.txt"), b"alpha").unwrap();
        fs::write(root.join("src/b.txt"), b"bravo").unwrap();
        let state_dir = dir.join("state");
        let core = RelayCore::open(CoreConfig::new(&state_dir));
        let runtime = runtime();

        let mut import = request(
            "REQ-dep-import",
            "project.import",
            json!({
                "id": "PRJ-dependencies",
                "name": "Dependencies",
                "root_path": root.to_string_lossy()
            }),
        );
        import.idempotency_key = Some("IDEMP-dep-import".to_string());
        assert!(core.execute(import, &runtime).ok);
        let mut build = request(
            "REQ-dep-build",
            "project.index.build",
            json!({ "project_id": "PRJ-dependencies" }),
        );
        build.idempotency_key = Some("IDEMP-dep-build".to_string());
        let built = core.execute(build, &runtime);
        assert!(built.ok);
        assert_eq!(built.result.unwrap()["generation"], 1);

        let storage = RelayStorage::open(state_dir.join("relay.sqlite3")).unwrap();
        let files = storage.list_project_files("PRJ-dependencies").unwrap();
        let source_sha = files.iter()
            .find(|file| file.relative_path == "src/a.txt")
            .unwrap()
            .content_sha256
            .clone();
        drop(storage);

        let make_replace = |id: &str, generation: i64, sha: &str, targets: Value| {
            let mut request = request(
                id,
                "project.dependencies.replace",
                json!({
                    "project_id": "PRJ-dependencies",
                    "expected_generation": generation,
                    "source_path": "src/a.txt",
                    "source_sha256": sha,
                    "producer_id": "fixture.parser",
                    "producer_version": "1",
                    "targets": targets
                }),
            );
            request.idempotency_key = Some(format!("IDEMP-{id}"));
            request
        };

        let wrong_hash = core.execute(
            make_replace("REQ-dep-wrong-hash", 1, &"0".repeat(64), json!(["src/b.txt"])),
            &runtime,
        );
        assert_eq!(wrong_hash.error.unwrap().code, "INDEX_SOURCE_CHANGED");
        let missing_target = core.execute(
            make_replace("REQ-dep-missing", 1, &source_sha, json!(["src/missing.txt"])),
            &runtime,
        );
        assert_eq!(missing_target.error.unwrap().code, "INDEX_FILE_NOT_FOUND");
        let escaped = core.execute(
            make_replace("REQ-dep-escaped", 1, &source_sha, json!(["../outside.txt"])),
            &runtime,
        );
        assert_eq!(escaped.error.unwrap().code, "PROJECT_PATH_ESCAPE");

        let replaced = core.execute(
            make_replace("REQ-dep-replace", 1, &source_sha, json!(["src/b.txt"])),
            &runtime,
        );
        assert!(replaced.ok, "{replaced:?}");
        assert_eq!(replaced.result.unwrap()["edge_count"], 1);
        let listed = core.execute(
            request(
                "REQ-dep-list",
                "project.dependencies.list",
                json!({ "project_id": "PRJ-dependencies" }),
            ),
            &runtime,
        );
        assert!(listed.ok);
        assert_eq!(listed.result.unwrap()["edges"][0]["target_path"], "src/b.txt");

        let stale_generation = core.execute(
            make_replace("REQ-dep-stale", 0, &source_sha, json!([])),
            &runtime,
        );
        assert_eq!(stale_generation.error.unwrap().code, "VALIDATION_FAILED");

        fs::write(root.join("src/b.txt"), b"bravo changed and longer").unwrap();
        fs::write(root.join("src/c.txt"), b"charlie").unwrap();
        let mut reconcile = request(
            "REQ-dep-reconcile",
            "project.index.reconcile",
            json!({ "project_id": "PRJ-dependencies" }),
        );
        reconcile.idempotency_key = Some("IDEMP-dep-reconcile".to_string());
        let reconciled = core.execute(reconcile, &runtime);
        assert!(reconciled.ok);
        assert_eq!(reconciled.result.unwrap()["generation"], 2);
        let listed = core.execute(
            request(
                "REQ-dep-list-after-change",
                "project.dependencies.list",
                json!({ "project_id": "PRJ-dependencies" }),
            ),
            &runtime,
        );
        assert!(listed.result.unwrap()["edges"].as_array().unwrap().is_empty());
        let delta = core.execute(
            request(
                "REQ-dep-delta",
                "project.changes",
                json!({ "project_id": "PRJ-dependencies", "after_generation": 1 }),
            ),
            &runtime,
        );
        assert!(delta.ok);
        let delta = delta.result.unwrap();
        assert_eq!(delta["changes"].as_array().unwrap().len(), 2);
        assert_eq!(delta["changes"][0]["relative_path"], "src/b.txt");
        let bounded = core.execute(
            request(
                "REQ-dep-bounded-delta",
                "project.changes",
                json!({
                    "project_id": "PRJ-dependencies",
                    "after_generation": 1,
                    "limit": 1
                }),
            ),
            &runtime,
        );
        assert_eq!(bounded.error.unwrap().code, "INDEX_DELTA_TOO_LARGE");

        let stale = core.execute(
            make_replace("REQ-dep-stale-after-change", 1, &source_sha, json!(["src/b.txt"])),
            &runtime,
        );
        assert_eq!(stale.error.unwrap().code, "INDEX_GENERATION_CONFLICT");

        let mut rebuild = request(
            "REQ-dep-rebuild",
            "project.index.build",
            json!({ "project_id": "PRJ-dependencies" }),
        );
        rebuild.idempotency_key = Some("IDEMP-dep-rebuild".to_string());
        assert!(core.execute(rebuild, &runtime).ok);
        let lost = core.execute(
            request(
                "REQ-dep-old-delta",
                "project.changes",
                json!({ "project_id": "PRJ-dependencies", "after_generation": 1 }),
            ),
            &runtime,
        );
        assert_eq!(lost.error.unwrap().code, "INDEX_CONTINUITY_LOST");

        drop(core);
        let reopened = RelayCore::open(CoreConfig::new(&state_dir));
        let current = reopened.execute(
            request(
                "REQ-dep-after-restart",
                "project.changes",
                json!({ "project_id": "PRJ-dependencies" }),
            ),
            &runtime,
        );
        assert!(current.ok);
        assert_eq!(current.result.unwrap()["baseline_generation"], 3);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn idempotent_replay_does_not_duplicate_transaction_and_usage_counts_replay() {
        let dir = temp_dir("transaction-replay");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();
        register_project(&core);

        let make = |request_id: &str| {
            let mut req = request(
                request_id,
                "result.put",
                json!({
                    "project_id": "PRJ-fixture",
                    "kind": "TEST",
                    "payload": { "value": 77 }
                }),
            );
            req.idempotency_key =
                Some("IDEMP-transaction-replay".to_string());
            req
        };

        let first = core.execute(make("REQ-first-tx"), &runtime);
        assert!(first.ok);
        assert!(!first.replayed);

        let replay = core.execute(make("REQ-second-tx"), &runtime);
        assert!(replay.ok);
        assert!(replay.replayed);

        let tx = core.execute(
            request(
                "REQ-tx-list-2",
                "transaction.list",
                json!({ "project_id": "PRJ-fixture", "limit": 50 }),
            ),
            &runtime,
        );
        assert!(tx.ok);
        let result_put_count = tx.result.unwrap()["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["command"] == "result.put")
            .count();
        assert_eq!(result_put_count, 1);

        let usage = core.execute(
            request("REQ-usage", "usage.summary", json!({})),
            &runtime,
        );
        assert!(usage.ok);
        let summary = usage.result.unwrap();
        assert!(summary["command_count"].as_u64().unwrap() >= 4);
        assert!(summary["replay_count"].as_u64().unwrap() >= 1);
        assert_eq!(summary["remote_calls"], 0);
        assert_eq!(summary["model_tokens_in"], 0);
        assert_eq!(summary["model_tokens_out"], 0);

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn egress_gate_is_local_only_by_default_and_revocation_is_durable() {
        use crate::policy::DestinationPolicy;

        let dir = temp_dir("egress");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();

        let mut authority =
            ExecutionAuthority::local_user("CLIENT-egress");
        authority.actor_id = "AGENT-egress".to_string();
        authority.delegator_id = Some("USER-owner".to_string());
        authority.project_ids =
            Some(["PRJ-egress".to_string()].into_iter().collect());
        authority.egress.destinations.insert(
            "remote-ai".to_string(),
            DestinationPolicy {
                remote: true,
                allowed_classes:
                    [DataClass::Project].into_iter().collect(),
                allowed_modalities:
                    ["text".to_string()].into_iter().collect(),
                allowed_projects:
                    Some(["PRJ-egress".to_string()].into_iter().collect()),
                max_bytes: Some(1024),
            },
        );

        let blocked = core.execute_authorized(
            request(
                "REQ-egress-blocked",
                "policy.egress.check",
                json!({
                    "destination": "remote-ai",
                    "project_id": "PRJ-egress",
                    "data_classes": ["project"],
                    "modalities": ["text"],
                    "approx_bytes": 100,
                    "approx_tokens": 25,
                    "source_refs": ["RES-fixture"],
                    "purpose": "diagnose"
                }),
            ),
            &runtime,
            &authority,
        );
        assert!(blocked.ok);
        assert_eq!(blocked.result.as_ref().unwrap()["allowed"], false);
        assert!(blocked.result.as_ref().unwrap()["reason"]
            .as_str()
            .unwrap()
            .contains("local-only"));

        authority.egress.local_only = false;
        let handle = CredentialHandle::active(
            "github.connection.fixture",
            "github",
            ["egress".to_string()],
        );
        core.register_credential_handle_metadata(&handle)
            .unwrap();
        authority
            .credential_handles
            .insert(handle.id.clone(), handle.clone());

        let allowed = core.execute_authorized(
            request(
                "REQ-egress-allowed",
                "policy.egress.check",
                json!({
                    "destination": "remote-ai",
                    "project_id": "PRJ-egress",
                    "data_classes": ["project"],
                    "modalities": ["text"],
                    "approx_bytes": 100,
                    "approx_tokens": 25,
                    "source_refs": ["RES-fixture"],
                    "purpose": "diagnose",
                    "credential_handle": "github.connection.fixture"
                }),
            ),
            &runtime,
            &authority,
        );
        assert!(allowed.ok);
        assert_eq!(allowed.result.as_ref().unwrap()["allowed"], true);
        assert!(allowed.result.as_ref().unwrap()["ledger_id"]
            .as_str()
            .unwrap()
            .starts_with("EGR-"));

        core.revoke_credential_handle_metadata(
            "github.connection.fixture",
        )
        .unwrap();

        let revoked = core.execute_authorized(
            request(
                "REQ-egress-revoked",
                "policy.egress.check",
                json!({
                    "destination": "remote-ai",
                    "project_id": "PRJ-egress",
                    "data_classes": ["project"],
                    "purpose": "diagnose",
                    "credential_handle": "github.connection.fixture"
                }),
            ),
            &runtime,
            &authority,
        );
        assert!(!revoked.ok);
        assert_eq!(
            revoked.error.unwrap().code,
            "CREDENTIAL_HANDLE_REVOKED"
        );

        let metadata = core
            .credential_handle_metadata("github.connection.fixture")
            .unwrap()
            .unwrap();
        assert_eq!(metadata.status, "revoked");
        let serialized = serde_json::to_string(&metadata).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("token"));

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn incompatible_command_version_is_explicit() {
        let dir = temp_dir("version");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let mut req =
            request("REQ-version", "system.status", json!({}));
        req.command_version = Some(999);
        let response = core.execute(req, &runtime());
        assert!(!response.ok);
        assert_eq!(
            response.error.unwrap().code,
            "COMMAND_VERSION_INCOMPATIBLE"
        );
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn command_diagnostics_do_not_persist_payload_values() {
        let dir = temp_dir("diagnostic-privacy");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let secret = "SHOULD-NOT-BE-IN-DIAGNOSTICS";
        let response = core.execute(
            request(
                "REQ-echo",
                "system.echo",
                json!({ "value": secret }),
            ),
            &runtime(),
        );
        assert!(response.ok);
        core.flush_diagnostics();
        drop(core);

        let diagnostics_dir = dir.join("diagnostics");
        let mut raw = String::new();
        for entry in fs::read_dir(&diagnostics_dir).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .ends_with(".jsonl")
            {
                raw.push_str(
                    &fs::read_to_string(entry.path()).unwrap()
                );
            }
        }
        assert!(raw.contains("relay.command.completed"));
        assert!(!raw.contains(secret));
        fs::remove_dir_all(dir).unwrap();
    }
}
