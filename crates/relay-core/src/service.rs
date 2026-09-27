use crate::context;
use crate::diagnostics::{
    DiagnosticConfig, DiagnosticEvent, DiagnosticHealth, JsonlDiagnostics, Severity,
};
use crate::indexing;
use crate::planner;
use crate::policy::{
    CredentialHandle, DataClass, EgressDecision, EgressRequest, ExecutionAuthority,
};
use crate::storage::{
    CredentialHandleRecord, DependencyReplacement, EgressLedgerInput, IndexCommitMode,
    NewTransaction, ProjectConfiguration, ProjectIndexCommit, RelayStorage, StorageError,
    StorageHealth, UsageMetricInput,
};
use relay_contracts::registry::{self, ResolveError};
use relay_contracts::{CommandRequest, CommandResponse, PROTOCOL_MAX, PROTOCOL_MIN, Producer};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};
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
    pub host_components: Vec<HostComponentHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostComponentState {
    NotConfigured,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostComponentHealth {
    pub id: String,
    pub state: HostComponentState,
    pub error_code: Option<String>,
    pub installed_count: u32,
    pub declared_count: u32,
    pub unavailable_count: u32,
    pub quarantined_count: u32,
    pub failure_count: u64,
    pub success_count: u64,
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
            host_components: Vec::new(),
        }
    }
}

pub struct RelayCore {
    storage: Option<Mutex<RelayStorage>>,
    storage_fallback: StorageHealth,
    diagnostics: Option<Mutex<JsonlDiagnostics>>,
    diagnostics_fallback: DiagnosticHealth,
    producer: Producer,
    automation_available: std::sync::atomic::AtomicBool,
    automation_paused: std::sync::atomic::AtomicBool,
    context_cache: Mutex<ContextCache>,
}

const MAX_CONTEXT_CACHE_ENTRIES: usize = 8;
const MAX_CONTEXT_CACHE_VALUE_BYTES: usize = 32 * 1024;

#[derive(PartialEq, Eq)]
struct ContextSourceKey {
    id: String,
    payload_sha256: String,
    kind: String,
    created_at: String,
    trust: String,
    producer_version: String,
    schema_version: i64,
}

#[derive(PartialEq, Eq)]
struct ContextCacheKey {
    project_id: String,
    sources: Vec<ContextSourceKey>,
    required_pointers: Vec<(String, String)>,
    focus_terms: Vec<String>,
    max_bytes: usize,
}

#[derive(Default)]
struct ContextCache {
    entries: VecDeque<(ContextCacheKey, Value)>,
    hits: u64,
    misses: u64,
    source_fact_collections_avoided: u64,
}

impl ContextCache {
    fn summary(&self) -> Value {
        json!({
            "available": true,
            "scope": "current_process",
            "entries": self.entries.len(),
            "hits": self.hits,
            "misses": self.misses,
            "source_fact_collections_avoided": self.source_fact_collections_avoided,
        })
    }
}

/// Trusted daemon input only. Canonical roots and indexed source identities never enter
/// public command results or adapter mailboxes.
pub struct ParserProjectSnapshot {
    pub project_id: String,
    pub root_uri: String,
    pub configuration: ProjectConfiguration,
    pub generation: i64,
    pub files: Vec<indexing::IndexedFileSnapshot>,
}

/// Trusted host input for read-only integrations. Paths are project-relative and
/// are returned only from a complete, current index after command authorization.
pub struct ReadyIndexSnapshot {
    pub generation: i64,
    pub relative_paths: Vec<String>,
}

pub struct ExtensionError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ExtensionError {
    pub fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl RelayCore {
    fn context_cache_summary(&self) -> Value {
        self.context_cache
            .lock()
            .map(|cache| cache.summary())
            .unwrap_or_else(|_| json!({ "available": false, "scope": "current_process" }))
    }

    fn context_cache_lookup(&self, key: &ContextCacheKey) -> Option<Value> {
        let mut cache = self.context_cache.lock().ok()?;
        if let Some((_, value)) = cache.entries.iter().find(|(cached, _)| cached == key) {
            let value = value.clone();
            cache.hits = cache.hits.saturating_add(1);
            cache.source_fact_collections_avoided = cache
                .source_fact_collections_avoided
                .saturating_add(key.sources.len() as u64);
            Some(value)
        } else {
            cache.misses = cache.misses.saturating_add(1);
            None
        }
    }

    fn context_cache_insert(&self, key: ContextCacheKey, value: Value) {
        if !serde_json::to_vec(&value)
            .is_ok_and(|bytes| bytes.len() <= MAX_CONTEXT_CACHE_VALUE_BYTES)
        {
            return;
        }
        let Ok(mut cache) = self.context_cache.lock() else {
            return;
        };
        if cache.entries.iter().any(|(cached, _)| cached == &key) {
            return;
        }
        if cache.entries.len() == MAX_CONTEXT_CACHE_ENTRIES {
            cache.entries.pop_front();
        }
        cache.entries.push_back((key, value));
    }

    pub fn ready_index_snapshot(
        &self,
        project_id: &str,
    ) -> Result<ReadyIndexSnapshot, ExtensionError> {
        self.with_storage(|storage| {
            if !storage
                .get_project(project_id)?
                .is_some_and(|project| project.lifecycle_state == "active")
            {
                return Ok(None);
            }
            let state = storage.get_project_index_state(project_id)?;
            let files = if state.as_ref().is_some_and(|state| {
                state.status == "ready" && !state.content_verification_required
            }) {
                Some(storage.list_project_files(project_id)?)
            } else {
                None
            };
            Ok(Some((state, files)))
        })
        .map_err(|_| ExtensionError::new("STORAGE_UNAVAILABLE", "project index is unavailable"))?
        .ok_or_else(|| ExtensionError::new("PROJECT_NOT_FOUND", "project not found"))
        .and_then(|(state, files)| {
            let (Some(state), Some(files)) = (state, files) else {
                return Err(ExtensionError::new(
                    "INDEX_NOT_READY",
                    "project index is not ready",
                ));
            };
            Ok(ReadyIndexSnapshot {
                generation: state.generation,
                relative_paths: files.into_iter().map(|file| file.relative_path).collect(),
            })
        })
    }

    /// Trusted host path lookup after shared command authorization. This path
    /// is never returned in a public command result or diagnostic event.
    pub fn trusted_project_root(&self, project_id: &str) -> Result<PathBuf, ExtensionError> {
        let project = self
            .with_storage(|storage| storage.get_project(project_id))
            .map_err(|_| {
                ExtensionError::new("STORAGE_UNAVAILABLE", "project registry is unavailable")
            })?
            .ok_or_else(|| ExtensionError::new("PROJECT_NOT_FOUND", "project not found"))?;
        if project.lifecycle_state != "active" {
            return Err(ExtensionError::new(
                "PROJECT_NOT_FOUND",
                "project is not active",
            ));
        }
        indexing::canonical_project_root(&project.root_uri).map_err(|_| {
            ExtensionError::new("PROJECT_ROOT_UNAVAILABLE", "project root is unavailable")
        })
    }

    pub fn require_registered_project(&self, project_id: &str) -> Result<(), ExtensionError> {
        let project = self
            .with_storage(|storage| storage.get_project(project_id))
            .map_err(|_| {
                ExtensionError::new("STORAGE_UNAVAILABLE", "project registry is unavailable")
            })?
            .ok_or_else(|| ExtensionError::new("PROJECT_NOT_FOUND", "project not found"))?;
        if project.lifecycle_state != "active" {
            return Err(ExtensionError::new(
                "PROJECT_NOT_FOUND",
                "project is not active",
            ));
        }
        Ok(())
    }

    /// Local host health input; exposes no project roots or source paths.
    pub fn parser_declared_bindings(&self) -> Result<Vec<(String, String, String)>, String> {
        self.with_storage(|storage| {
            let mut bindings = Vec::new();
            for project in storage.list_projects()? {
                if let Some(configuration) = storage.get_project_configuration(&project.id)?
                    && let (Some(adapter_id), Some(adapter_version)) =
                        (configuration.adapter_id, configuration.adapter_version)
                {
                    bindings.push((project.id, adapter_id, adapter_version));
                }
            }
            Ok(bindings)
        })
        .map_err(|error| error.message)
    }

    /// Append only fixed daemon event labels and codes; never source or worker text.
    pub fn record_host_component_event(
        &self,
        component: &'static str,
        code: &'static str,
        recovered: bool,
    ) {
        if let Some(logger) = &self.diagnostics
            && let Ok(mut logger) = logger.lock()
        {
            let mut event = DiagnosticEvent::new(
                if recovered {
                    "relay.host.component.recovered"
                } else {
                    "relay.host.component.failed"
                },
                if recovered {
                    Severity::Info
                } else {
                    Severity::Warn
                },
                "host.component",
                if recovered {
                    "Host component recovered"
                } else {
                    "Host component needs attention"
                },
            );
            event
                .attributes
                .insert("component".to_string(), json!(component));
            event
                .attributes
                .insert("error_code".to_string(), json!(code));
            let _ = logger.append(event);
        }
    }

    pub fn parser_ready_projects(&self) -> Result<Vec<ParserProjectSnapshot>, String> {
        self.with_storage(|storage| {
            let mut ready = Vec::new();
            for project in storage.list_projects()? {
                let Some(configuration) = storage.get_project_configuration(&project.id)? else {
                    continue;
                };
                if configuration.adapter_id.is_none() {
                    continue;
                }
                let Some(state) = storage.get_project_index_state(&project.id)? else {
                    continue;
                };
                if state.status != "ready" || state.content_verification_required {
                    continue;
                }
                ready.push(ParserProjectSnapshot {
                    project_id: project.id.clone(),
                    root_uri: project.root_uri,
                    configuration,
                    generation: state.generation,
                    files: Vec::new(),
                });
            }
            Ok(ready)
        })
        .map_err(|error| error.message)
    }

    /// Fetch source identities only when a daemon-observed ready generation changes.
    pub fn parser_ready_files(
        &self,
        project_id: &str,
        expected_generation: i64,
    ) -> Result<Option<Vec<indexing::IndexedFileSnapshot>>, String> {
        self.with_storage(|storage| {
            storage.get_active_project(project_id)?;
            let Some(state) = storage.get_project_index_state(project_id)? else {
                return Ok(None);
            };
            if state.status != "ready"
                || state.content_verification_required
                || state.generation != expected_generation
            {
                return Ok(None);
            }
            storage.list_project_files(project_id).map(Some)
        })
        .map_err(|error| error.message)
    }

    /// A bounded invalidation hint for daemon parser scheduling. None requires a full
    /// parser-cache reset after a rebuild or an oversized change burst.
    pub fn parser_changed_paths(
        &self,
        project_id: &str,
        after_generation: i64,
        expected_generation: i64,
    ) -> Result<Option<BTreeSet<String>>, String> {
        self.with_storage(|storage| {
            storage.get_active_project(project_id)?;
            let Some(state) = storage.get_project_index_state(project_id)? else {
                return Ok(None);
            };
            if state.status != "ready"
                || state.generation != expected_generation
                || after_generation < state.baseline_generation
            {
                return Ok(None);
            }
            let changes = storage.list_project_changes_since(project_id, after_generation, 501)?;
            if changes.len() > 500 {
                return Ok(None);
            }
            let mut paths = BTreeSet::new();
            for change in changes {
                paths.insert(change.relative_path);
                if let Some(previous_path) = change.previous_path {
                    paths.insert(previous_path);
                }
            }
            Ok(Some(paths))
        })
        .map_err(|error| error.message)
    }

    /// Recheck the selected parser and indexed source immediately before source delivery.
    pub fn parser_source_selected(
        &self,
        project_id: &str,
        generation: i64,
        configuration_revision: i64,
        adapter_id: &str,
        adapter_version: &str,
        source_path: &str,
        source_sha256: &str,
    ) -> bool {
        self.with_storage(|storage| {
            storage.get_active_project(project_id)?;
            let state = storage.get_project_index_state(project_id)?;
            let configuration = storage.get_project_configuration(project_id)?;
            let indexed_sha256 = storage.project_file_sha256(project_id, source_path)?;
            Ok(state.is_some_and(|state| {
                state.status == "ready"
                    && !state.content_verification_required
                    && state.generation == generation
            }) && configuration.is_some_and(|config| {
                config.revision == configuration_revision
                    && config.adapter_id.as_deref() == Some(adapter_id)
                    && config.adapter_version.as_deref() == Some(adapter_version)
            }) && indexed_sha256.as_deref() == Some(source_sha256))
        })
        .unwrap_or(false)
    }
    /// Local daemon input only; never return canonical roots through command results.
    pub fn index_watch_targets(&self) -> Result<Vec<(String, String)>, String> {
        self.with_storage(|storage| {
            let mut targets = Vec::new();
            for project in storage.list_projects()? {
                if storage.get_project_index_state(&project.id)?.is_some() {
                    targets.push((project.id, project.root_uri));
                }
            }
            Ok(targets)
        })
        .map_err(|error| error.message)
    }

    /// Record loss of notification continuity without changing a project generation.
    pub fn mark_index_stale(&self, project_id: &str) -> Result<(), String> {
        self.with_storage(|storage| storage.mark_project_index_stale(project_id))
            .map_err(|error| error.message)
    }

    /// Local daemon recovery input; exposes only index state, never a canonical root.
    pub fn index_recovery_state(
        &self,
        project_id: &str,
    ) -> Result<Option<crate::storage::ProjectIndexState>, String> {
        self.with_storage(|storage| {
            storage.get_active_project(project_id)?;
            storage.get_project_index_state(project_id)
        })
        .map_err(|error| error.message)
    }

    /// Trusted local watcher recovery uses the same reconciliation path with cooperative deferral.
    pub fn reconcile_index_background(
        &self,
        project_id: &str,
        verify_content: bool,
        should_continue: &impl Fn() -> bool,
    ) -> Result<(), String> {
        self.project_index_reconcile_with_guard(
            &json!({ "project_id": project_id, "verify_content": verify_content }),
            should_continue,
        )
        .map(|_| ())
        .map_err(|error| format!("{}: {}", error.code, error.message))
    }

    pub fn open(config: CoreConfig) -> Self {
        let _ = std::fs::create_dir_all(&config.data_dir);

        let (storage, storage_fallback) = match RelayStorage::open(config.database_path()) {
            Ok(storage) => {
                let health = storage
                    .integrity()
                    .unwrap_or_else(|error| StorageHealth::unavailable(error.to_string()));
                if health.ok {
                    (Some(Mutex::new(storage)), health)
                } else {
                    (None, health)
                }
            }
            Err(error) => (None, StorageHealth::unavailable(error.to_string())),
        };

        let (diagnostics, diagnostics_fallback) =
            match JsonlDiagnostics::open(DiagnosticConfig::new(config.diagnostics_dir())) {
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
                Err(error) => (None, DiagnosticHealth::unavailable(error.to_string())),
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
            automation_available: std::sync::atomic::AtomicBool::new(false),
            automation_paused: std::sync::atomic::AtomicBool::new(false),
            context_cache: Mutex::new(ContextCache::default()),
        }
    }

    /// Trusted host signal; a failed or stopped watcher cannot claim it is running.
    pub fn set_automation_available(&self, available: bool) {
        self.automation_available
            .store(available, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn automation_paused(&self) -> bool {
        self.automation_paused
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn automation_mode(&self) -> &'static str {
        if !self
            .automation_available
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            "unavailable"
        } else if self.automation_paused() {
            "paused"
        } else {
            "running"
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
        let scopes: Vec<String> = handle.scopes.iter().cloned().collect();
        guard.upsert_credential_handle(&handle.id, &handle.integration, &scopes, &handle.status)
    }

    pub fn revoke_credential_handle_metadata(&self, handle_id: &str) -> Result<(), StorageError> {
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

    pub fn execute(&self, request: CommandRequest, runtime: &RuntimeContext) -> CommandResponse {
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
        request: CommandRequest,
        runtime: &RuntimeContext,
        authority: &ExecutionAuthority,
    ) -> CommandResponse {
        self.execute_authorized_with_extension(request, runtime, authority, |_| None)
    }

    /// Extensions run only after the shared registry validates arguments and
    /// the Core authority checks pass. Core validates their returned schema.
    pub fn execute_authorized_with_extension(
        &self,
        mut request: CommandRequest,
        runtime: &RuntimeContext,
        authority: &ExecutionAuthority,
        extension: impl Fn(&CommandRequest) -> Option<Result<Value, ExtensionError>>,
    ) -> CommandResponse {
        let started = Instant::now();
        request.context.actor_id = Some(authority.actor_id.clone());
        request.context.client_id = Some(authority.client_id.clone());
        request.context.delegator_id = authority.delegator_id.clone();

        let spec = match registry::resolve_command(&request.command, request.command_version) {
            Ok(spec) => spec,
            Err(ResolveError::UnknownCommand) => {
                let response = self.failure(
                    &request,
                    request.command_version.unwrap_or(0),
                    "COMMAND_UNKNOWN",
                    format!("unknown command {}", request.command),
                );
                return self.finalize(&request, response, "unknown", "unknown", authority, started);
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
                return self.finalize(&request, response, "unknown", "unknown", authority, started);
            }
        };

        if let Err(error) = registry::validate_value(&spec.arguments_schema, &request.arguments) {
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

        if let Err(error) =
            self.authorize_preflight(&request, &spec.effect_class, &spec.permission, authority)
        {
            let response = self.failure(&request, spec.version, error.code, error.message);
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
                let response = self.failure(&request, spec.version, error.code, error.message);
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

        let business = if request.command.starts_with("uefn.")
            || request.command.starts_with("assets.")
            || request.command.starts_with("runtime.")
            || request.command == "tools.local.discover"
        {
            extension(&request)
                .map(|result| {
                    result.map_err(|error| CoreCommandError::new(error.code, error.message))
                })
                .unwrap_or_else(|| {
                    Err(CoreCommandError::new(
                        "INTEGRATION_UNAVAILABLE",
                        "integration command is unavailable on this host",
                    ))
                })
        } else {
            self.dispatch(&request, spec.version, runtime, authority)
        };
        let mut response = match business {
            Ok(result) => {
                if let Err(error) = registry::validate_value(&spec.result_schema, &result) {
                    self.failure(
                        &request,
                        spec.version,
                        "RESULT_SCHEMA_VIOLATION",
                        format!("{} {}", error.path, error.message),
                    )
                } else {
                    CommandResponse::success(&request, spec.version, self.producer.clone(), result)
                }
            }
            Err(error) => self.failure(&request, spec.version, error.code, error.message),
        };

        if response.ok && spec.idempotency != "safe" && request.idempotency_key.is_some() {
            response = self.persist_idempotency(&request, spec.version, response);
        }

        response = self.finish_transaction_if_needed(&request, response, transaction.as_deref());
        response = self.enforce_error_contract(&request, spec, response);

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
        operation(&guard).map_err(|error| CoreCommandError::new(error.code, error.message))
    }

    fn storage_health(&self) -> StorageHealth {
        match &self.storage {
            Some(storage) => storage
                .lock()
                .map_err(|_| ())
                .and_then(|storage| storage.integrity().map_err(|_| ()))
                .unwrap_or_else(|_| StorageHealth::unavailable("storage health check unavailable")),
            None => self.storage_fallback.clone(),
        }
    }

    fn diagnostics_health(&self) -> DiagnosticHealth {
        match &self.diagnostics {
            Some(logger) => logger
                .lock()
                .map(|logger| logger.health())
                .unwrap_or_else(|_| {
                    DiagnosticHealth::unavailable("diagnostics health check unavailable")
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
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        authority
            .require_effect(effect_class)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;

        let project_id = match request.command.as_str() {
            "project.register" | "project.import" => {
                request.arguments.get("id").and_then(Value::as_str)
            }
            "result.put" | "job.checkpoint" => {
                request.arguments.get("project_id").and_then(Value::as_str)
            }
            _ => request.arguments.get("project_id").and_then(Value::as_str),
        };

        if project_id.is_some()
            || (authority.project_ids.is_some()
                && matches!(
                    request.command.as_str(),
                    "project.register" | "project.import"
                ))
        {
            authority
                .require_project(project_id)
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
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

    fn transaction_project_id<'a>(request: &'a CommandRequest) -> Option<&'a str> {
        if matches!(
            request.command.as_str(),
            "project.register" | "project.import"
        ) {
            return None;
        }
        request.arguments.get("project_id").and_then(Value::as_str)
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
        let request_sha256 = request_fingerprint(request, request.command_version.unwrap_or(1));
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
                    format!("error envelope could not be serialized: {serialize_error}"),
                );
            }
        };
        if let Err(validation) =
            registry::validate_value(&registry::registry().error_schema, &error_value)
        {
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

    fn usage_project_id(request: &CommandRequest, response: &CommandResponse) -> Option<String> {
        if let Some(project_id) = request.arguments.get("project_id").and_then(Value::as_str) {
            return Some(project_id.to_string());
        }
        if matches!(
            request.command.as_str(),
            "project.register" | "project.import"
        ) && response.ok
        {
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
            "diagnostics.summary" => self.diagnostics_summary(),
            "automation.pause" => self.set_automation_paused(true),
            "automation.resume" => self.set_automation_paused(false),
            "system.echo" => Ok(json!({
                "echo": request.arguments
            })),
            "system.shutdown" => Ok(json!({
                "shutting_down": true
            })),
            "registry.list" => self.registry_list(&request.arguments),
            "registry.describe" => self.registry_describe(&request.arguments),
            "storage.integrity" => {
                let health = self.with_storage(|storage| storage.integrity())?;
                serde_json::to_value(health).map_err(|error| {
                    CoreCommandError::new(
                        "STORAGE_ERROR",
                        format!("serialize storage health: {error}"),
                    )
                })
            }
            "project.register" => self.project_register(&request.arguments),
            "project.import" => self.project_import(&request.arguments),
            "project.list" => self.project_list(&request.arguments, authority),
            "project.archive" => self.project_lifecycle(&request.arguments, "archived"),
            "project.restore" => self.project_lifecycle(&request.arguments, "active"),
            "project.remove" => {
                if command_version == 1 {
                    Err(CoreCommandError::new(
                        "APPROVAL_REQUIRED",
                        "project.remove@1 is disabled; plan, approve, and execute project.remove@2",
                    ))
                } else {
                    self.project_removal_execute(&request.arguments, authority)
                }
            }
            "project.removal.plan" => self.project_removal_plan(&request.arguments, authority),
            "project.removal.get" => self.project_removal_get(&request.arguments),
            "project.removal.list" => self.project_removal_list(&request.arguments),
            "project.removal.decide" => self.project_removal_decide(&request.arguments, authority),
            "project.configuration.put" => self.project_configuration_put(&request.arguments),
            "project.configuration.get" => self.project_configuration_get(&request.arguments),
            "project.index.build" => self.project_index_build(&request.arguments),
            "project.index.apply_hints" => self.project_index_apply_hints(&request.arguments),
            "project.index.reconcile" => self.project_index_reconcile(&request.arguments),
            "project.capabilities" => self.project_capabilities(&request.arguments),
            "project.changes" => self.project_changes(&request.arguments),
            "project.dependencies.replace" => self.project_dependencies_replace(&request.arguments),
            "project.dependencies.list" => self.project_dependencies_list(&request.arguments),
            "project.check_catalog.put" => self.project_check_catalog_put(&request.arguments),
            "project.check_catalog.get" => self.project_check_catalog_get(&request.arguments),
            "automation.checks.plan" => self.automation_checks_plan(&request.arguments),
            "automation.checks.execute" => self.automation_checks_execute(request),
            "result.put" => self.result_put(request),
            "result.get" => self.result_get(&request.arguments, authority),
            "result.list" => self.result_list(&request.arguments, authority),
            "result.describe" => self.result_describe(&request.arguments, authority),
            "result.context" => self.result_context(&request.arguments, authority),
            "context.compile" => self.context_compile(&request.arguments, authority),
            "context.task.compile" => self.context_task_compile(&request.arguments, authority),
            "job.checkpoint" => self.job_checkpoint(request),
            "job.get" => self.job_get(&request.arguments, authority),
            "job.list" => self.job_list(&request.arguments, authority),
            "transaction.list" => self.transaction_list(&request.arguments, authority),
            "usage.summary" => self.usage_summary(),
            "policy.egress.check" => self.policy_egress_check(request, authority),
            other => Err(CoreCommandError::new(
                "COMMAND_UNKNOWN",
                format!("command {other}@{command_version} is not implemented"),
            )),
        }
    }

    fn set_automation_paused(&self, paused: bool) -> Result<Value, CoreCommandError> {
        if !self
            .automation_available
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(CoreCommandError::new(
                "AUTOMATION_UNAVAILABLE",
                "background reconciliation is unavailable",
            ));
        }
        self.automation_paused
            .store(paused, std::sync::atomic::Ordering::SeqCst);
        Ok(json!({ "mode": self.automation_mode() }))
    }

    fn diagnostics_summary(&self) -> Result<Value, CoreCommandError> {
        let health = self.diagnostics_health();
        let (available, events, recent, error_code) = match &self.diagnostics {
            None => (false, Value::Null, Value::Null, Some("DIAGNOSTICS_UNAVAILABLE")),
            Some(logger) => match logger.lock() {
                Err(_) => (false, Value::Null, Value::Null, Some("DIAGNOSTICS_LOCK_UNAVAILABLE")),
                Ok(logger) => match logger.support_summary() {
                    Err(_) => (false, Value::Null, Value::Null, Some("DIAGNOSTICS_READ_FAILED")),
                    Ok(summary) => (
                        true,
                        json!({
                            "total": summary.aggregate.total_events,
                            "incomplete": summary.aggregate.incomplete_events,
                            "sampled": summary.aggregate.sampled_events,
                            "invalid_lines": summary.aggregate.invalid_lines,
                            "by_severity": summary.aggregate.by_severity,
                        }),
                        json!(summary.aggregate.recent),
                        None,
                    ),
                },
            },
        };
        Ok(json!({
            "schema_version": 1,
            "relay_version": crate::CORE_VERSION,
            "generated_unix_ms": crate::diagnostics::unix_ms(),
            "available": available,
            "health": {
                "ok": health.ok,
                "retention_evicted_events": health.evicted_events,
                "retention_evicted_files": health.evicted_files,
            },
            "events": events,
            "recent": recent,
            "error_code": error_code,
            "automation_mode": self.automation_mode(),
        }))
    }

    fn status(&self, runtime: &RuntimeContext) -> Result<Value, CoreCommandError> {
        let storage = self.storage_health();
        let diagnostics = self.diagnostics_health();
        let host_healthy = runtime
            .host_components
            .iter()
            .all(|component| component.state != HostComponentState::Degraded);
        let healthy = storage.ok && diagnostics.ok && runtime.ipc_healthy && host_healthy;
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
            "host_components": runtime.host_components,
            "automation_mode": self.automation_mode(),
            "context_cache": self.context_cache_summary(),
            "uptime_ms": runtime.uptime_ms
        }))
    }
    fn doctor(&self, runtime: &RuntimeContext) -> Result<Value, CoreCommandError> {
        let storage = self.storage_health();
        let diagnostics = self.diagnostics_health();
        let host_healthy = runtime
            .host_components
            .iter()
            .all(|component| component.state != HostComponentState::Degraded);
        let healthy = storage.ok && diagnostics.ok && runtime.ipc_healthy && host_healthy;
        let mut report = json!({
            "healthy": healthy,
            "summary": if healthy {
                "RELAY Core, storage, diagnostics, and local transport are healthy."
            } else if !storage.ok {
                "RELAY Core is reachable, but operational storage needs attention."
            } else if !diagnostics.ok {
                "RELAY Core is reachable, but diagnostics capture needs attention."
            } else if !host_healthy {
                "A local host component needs attention."
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
            } else if !host_healthy {
                Value::String("Inspect the host component error code and repair its installation or input.".to_string())
            } else {
                Value::String(
                    "Inspect local transport security/connectivity."
                        .to_string()
                )
            }
        });
        if let Some(checks) = report["checks"].as_array_mut() {
            for component in &runtime.host_components {
                checks.push(json!({
                    "id": component.id,
                    "status": if component.state == HostComponentState::Degraded { "fail" } else { "pass" },
                    "detail": component.error_code.as_deref().unwrap_or("component available")
                }));
            }
        }
        Ok(report)
    }

    fn registry_list(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let surface = arguments.get("surface").and_then(Value::as_str);
        let prefix = arguments.get("prefix").and_then(Value::as_str);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(100)
            .clamp(1, 200) as usize;
        Ok(registry::compact_list(surface, prefix, limit))
    }

    fn registry_describe(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreCommandError::new("VALIDATION_FAILED", "command is required"))?;
        let version = arguments
            .get("version")
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        registry::describe(command, version).map_err(|error| match error {
            ResolveError::UnknownCommand => {
                CoreCommandError::new("COMMAND_UNKNOWN", format!("unknown command {command}"))
            }
            ResolveError::VersionIncompatible { supported } => CoreCommandError::new(
                "COMMAND_VERSION_INCOMPATIBLE",
                format!("supported versions: {supported:?}"),
            ),
        })
    }
    fn project_register(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let name = arguments["name"]
            .as_str()
            .expect("registry validation requires name");
        let root_uri = arguments["root_uri"]
            .as_str()
            .expect("registry validation requires root_uri");
        let id = arguments.get("id").and_then(Value::as_str);
        let project = self.with_storage(|storage| storage.register_project(id, name, root_uri))?;
        serde_json::to_value(project).map_err(|error| {
            CoreCommandError::new("STORAGE_ERROR", format!("serialize project: {error}"))
        })
    }

    fn project_import(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
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
        let project = self.with_storage(|storage| storage.register_project(id, name, &root_uri))?;
        Ok(json!({
            "id": project.id,
            "name": project.name,
            "root_canonicalized": true,
            "baseline_state": "missing"
        }))
    }

    fn project_list(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let include_inactive = arguments
            .get("include_inactive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut projects =
            self.with_storage(|storage| storage.list_projects_with_inactive(include_inactive))?;
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

    fn project_lifecycle(
        &self,
        arguments: &Value,
        target: &str,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let (project, changed) =
            self.with_storage(|storage| storage.set_project_lifecycle(project_id, target))?;
        Ok(json!({
            "project_id": project.id,
            "lifecycle_state": project.lifecycle_state,
            "changed": changed
        }))
    }

    fn project_removal_plan(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let actor = safe_approval_ref(&authority.actor_id);
        let client = safe_approval_ref(&authority.client_id);
        let delegator = authority.delegator_id.as_deref().map(safe_approval_ref);
        let approval = self.with_storage(|storage| {
            storage.plan_project_removal(project_id, &actor, &client, delegator.as_deref())
        })?;
        Ok(removal_approval_json(&approval))
    }

    fn project_removal_get(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let approval_id = arguments["approval_id"].as_str().unwrap();
        let approval = self.with_storage(|storage| {
            storage.get_project_removal_approval(project_id, approval_id)
        })?;
        Ok(removal_approval_json(&approval))
    }

    fn project_removal_list(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .min(100) as usize;
        let approvals =
            self.with_storage(|storage| storage.list_project_removal_approvals(project_id, limit))?;
        Ok(
            json!({ "project_id": safe_approval_ref(project_id), "approvals": approvals.iter().map(removal_approval_json).collect::<Vec<_>>() }),
        )
    }

    fn project_removal_decide(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let approval_id = arguments["approval_id"].as_str().unwrap();
        let approve = arguments["decision"].as_str() == Some("approve");
        let actor = safe_approval_ref(&authority.actor_id);
        let client = safe_approval_ref(&authority.client_id);
        let approval = self.with_storage(|storage| {
            storage.decide_project_removal(project_id, approval_id, approve, &actor, &client)
        })?;
        Ok(removal_approval_json(&approval))
    }

    fn project_removal_execute(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let approval_id = arguments["approval_id"].as_str().unwrap();
        let actor = safe_approval_ref(&authority.actor_id);
        let client = safe_approval_ref(&authority.client_id);
        let (approval, executed) = self.with_storage(|storage| {
            storage.execute_project_removal(project_id, approval_id, &actor, &client)
        })?;
        Ok(
            json!({ "project_id": safe_approval_ref(project_id), "approval_id": approval.id, "state": approval.state, "executed": executed }),
        )
    }

    fn project_configuration_put(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let expected_revision = arguments["expected_revision"].as_i64().unwrap();
        let project_type = arguments["project_type"].as_str().unwrap();
        let adapter_id = arguments.get("adapter_id").and_then(Value::as_str);
        let adapter_version = arguments.get("adapter_version").and_then(Value::as_str);
        let valid_token = |value: &str, max: usize| {
            !value.is_empty()
                && value.len() <= max
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
                })
        };
        if !valid_token(project_type, 128)
            || adapter_id.is_some() != adapter_version.is_some()
            || adapter_id.is_some_and(|value| !valid_token(value, 128))
            || adapter_version.is_some_and(|value| !valid_token(value, 64))
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "project type and optional adapter binding must be bounded identifiers",
            ));
        }
        let config = self.with_storage(|storage| {
            storage.put_project_configuration(
                project_id,
                expected_revision,
                project_type,
                adapter_id,
                adapter_version,
            )
        })?;
        Ok(json!({
            "project_id": config.project_id,
            "configured": true,
            "revision": config.revision,
            "format_version": config.format_version,
            "project_type": config.project_type,
            "adapter_id": config.adapter_id,
            "adapter_version": config.adapter_version,
            "updated_at": config.updated_at
        }))
    }

    fn project_configuration_get(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"].as_str().unwrap();
        let config = self.with_storage(|storage| {
            if storage.get_project(project_id)?.is_none() {
                return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
            }
            storage.get_project_configuration(project_id)
        })?;
        match config {
            Some(config) => Ok(json!({
                "project_id": config.project_id,
                "configured": true,
                "revision": config.revision,
                "format_version": config.format_version,
                "project_type": config.project_type,
                "adapter_id": config.adapter_id,
                "adapter_version": config.adapter_version,
                "updated_at": config.updated_at
            })),
            None => Ok(json!({
                "project_id": project_id,
                "configured": false,
                "revision": 0,
                "format_version": 1,
                "project_type": null,
                "adapter_id": null,
                "adapter_version": null,
                "updated_at": null
            })),
        }
    }

    fn project_index_build(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let project = self.with_storage(|storage| storage.get_active_project(project_id))?;
        let root = indexing::canonical_project_root(&project.root_uri)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let plan = indexing::build_baseline(&root)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let state = self.with_storage(|storage| {
            storage.replace_project_baseline(project_id, &plan.files, plan.stats.total_bytes)
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

    fn project_index_apply_hints(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let raw_hints = arguments["hints"]
            .as_array()
            .expect("registry validation requires hints");
        if raw_hints.is_empty() || raw_hints.len() > 500 {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "hint-only updates require 1 to 500 paths",
            ));
        }
        let raw_hints: Vec<String> = raw_hints
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("registry validation requires string hints")
                    .to_string()
            })
            .collect();
        let hints = indexing::validate_hints(&raw_hints)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let (project, index_state, previous) = self.with_storage(|storage| {
            let project = storage.get_active_project(project_id)?;
            let state = storage
                .get_project_index_state(project_id)?
                .ok_or_else(|| {
                    StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
                })?;
            let files = storage.project_files_for_paths(project_id, &hints)?;
            Ok((project, state, files))
        })?;
        let root = indexing::canonical_project_root(&project.root_uri)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let plan = indexing::apply_hints(
            &root,
            &previous,
            &hints,
            index_state.file_count,
            index_state.total_bytes,
        )
        .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let file_count = usize::try_from(plan.file_count).map_err(|_| {
            CoreCommandError::new(
                "INDEX_METADATA_OVERFLOW",
                "file count exceeds platform limits",
            )
        })?;
        let state = self.with_storage(|storage| {
            storage.apply_project_reconciliation(ProjectIndexCommit {
                project_id,
                expected_generation: index_state.generation,
                touched_files: &plan.touched_files,
                changes: &plan.changes,
                file_count,
                total_bytes: plan.stats.total_bytes,
                mode: IndexCommitMode::HintsOnly,
                content_verified: false,
            })
        })?;
        Ok(json!({
            "project_id": project_id,
            "generation": state.generation,
            "file_count": state.file_count,
            "files_hashed": plan.stats.files_hashed,
            "hint_count": plan.stats.hint_count,
            "changes": plan.changes,
            "elapsed_ms": plan.stats.elapsed_ms,
            "index_status": "stale"
        }))
    }

    fn project_index_reconcile(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        self.project_index_reconcile_with_guard(arguments, &|| true)
    }

    fn project_index_reconcile_with_guard(
        &self,
        arguments: &Value,
        should_continue: &impl Fn() -> bool,
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
        let verify_content = arguments
            .get("verify_content")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let (project, index_state, previous) = self.with_storage(|storage| {
            let project = storage.get_active_project(project_id)?;
            let state = storage
                .get_project_index_state(project_id)?
                .ok_or_else(|| {
                    StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
                })?;
            let files = storage.list_project_files(project_id)?;
            Ok((project, state, files))
        })?;

        if index_state.content_verification_required && !verify_content {
            return Err(CoreCommandError::new(
                "INDEX_CONTENT_VERIFICATION_REQUIRED",
                "full content verification is required after index continuity loss",
            ));
        }

        let root = indexing::canonical_project_root(&project.root_uri)
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let plan = indexing::reconcile_with_guard(
            &root,
            &previous,
            &hints,
            verify_content,
            should_continue,
        )
        .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        if !should_continue() {
            return Err(CoreCommandError::new(
                "INDEX_RECOVERY_DEFERRED",
                "background index recovery was deferred",
            ));
        }
        let state = self.with_storage(|storage| {
            storage.apply_project_reconciliation(ProjectIndexCommit {
                project_id,
                expected_generation: index_state.generation,
                touched_files: &plan.touched_files,
                changes: &plan.changes,
                file_count: plan.files.len(),
                total_bytes: plan.stats.total_bytes,
                mode: IndexCommitMode::Authoritative,
                content_verified: verify_content,
            })
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
            "verify_content": verify_content,
            "mode": "reconcile"
        }))
    }

    fn project_capabilities(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let (project, state) = self.with_storage(|storage| {
            let project = storage.get_active_project(project_id)?;
            let state = storage.get_project_index_state(project_id)?;
            Ok((project, state))
        })?;

        let root_available = indexing::canonical_project_root(&project.root_uri).is_ok();
        let baseline_ready = state
            .as_ref()
            .map(|state| state.status == "ready")
            .unwrap_or(false);
        let index_status = if !root_available {
            "unavailable"
        } else {
            state
                .as_ref()
                .map(|state| state.status.as_str())
                .unwrap_or("missing")
        };
        let baseline_state = if !root_available {
            "unavailable"
        } else if state.is_some() {
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
            } else if state.is_some() {
                "durable baseline exists but requires authoritative reconciliation"
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
            } else if state.is_some() {
                "hint-only changes are provisional until authoritative reconciliation"
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
            "detail": "metadata reconciliation checks the full tree; verify_content also hashes every file after uncertain continuity"
        }));
        capabilities.push(json!({
            "id": "dependency_edges",
            "state": if !root_available { "unavailable" } else if baseline_ready { "available" } else { "unknown" },
            "detail": "project-scoped derived edges can be supplied for indexed files with generation and source-hash checks"
        }));
        capabilities.push(json!({
            "id": "index.continuity",
            "state": if !root_available { "unavailable" } else if baseline_ready { "available" } else { "unknown" },
            "detail": if index_status == "stale" {
                "hint-only update awaits authoritative reconciliation"
            } else {
                "full-tree metadata was reconciled for the stored generation; content verification is available separately"
            }
        }));
        capabilities.push(json!({
            "id": "dependency_graph",
            "state": "unknown",
            "detail": "automatic dependency extraction requires a compatible parser or adapter"
        }));

        Ok(json!({
            "project_id": project_id,
            "baseline_state": baseline_state,
            "index_status": index_status,
            "content_verification_required": state.as_ref()
                .map(|state| state.content_verification_required).unwrap_or(false),
            "capabilities": capabilities
        }))
    }

    fn project_check_catalog_put(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let expected_revision = arguments["expected_revision"]
            .as_i64()
            .expect("registry validation requires expected_revision");
        let canonical = planner::canonical_catalog(&arguments["catalog"])
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let record = self.with_storage(|storage| {
            storage.put_project_check_catalog(project_id, expected_revision, &canonical)
        })?;
        Ok(json!({
            "project_id": record.project_id,
            "revision": record.revision,
            "index_generation": record.index_generation,
            "configuration_revision": record.configuration_revision,
            "catalog_sha256": record.catalog_sha256,
            "check_count": record.catalog["checks"].as_array().map_or(0, Vec::len),
        }))
    }

    fn project_check_catalog_get(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let record = self.with_storage(|storage| {
            storage.get_active_project(project_id)?;
            storage
                .get_project_check_catalog(project_id)?
                .ok_or_else(|| {
                    StorageError::new("CHECK_CATALOG_MISSING", "project check catalog is missing")
                })
        })?;
        Ok(json!({
            "project_id": record.project_id,
            "revision": record.revision,
            "index_generation": record.index_generation,
            "configuration_revision": record.configuration_revision,
            "catalog_sha256": record.catalog_sha256,
            "catalog": record.catalog,
        }))
    }

    fn automation_checks_plan(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let after_generation = arguments["after_generation"]
            .as_i64()
            .expect("registry validation requires after_generation");
        let snapshot = self.with_storage(|storage| {
            storage.project_plan_snapshot(project_id, after_generation, 4096)
        })?;
        planner::plan(snapshot, after_generation)
            .map_err(|error| CoreCommandError::new(error.code, error.message))
    }

    fn automation_checks_execute(
        &self,
        request: &CommandRequest,
    ) -> Result<Value, CoreCommandError> {
        let arguments = &request.arguments;
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let after_generation = arguments["after_generation"]
            .as_i64()
            .expect("registry validation requires after_generation");
        let expected_plan_id = arguments["plan_id"]
            .as_str()
            .expect("registry validation requires plan_id");
        let provenance = self.provenance(request);
        let trust = self.trust_label(request);
        self.with_storage(|storage| {
            let snapshot = storage.project_plan_snapshot(project_id, after_generation, 4096)?;
            let planned = planner::plan(snapshot.clone(), after_generation)
                .map_err(|error| StorageError::new(error.code, error.message))?;
            if planned["plan_id"] != expected_plan_id {
                return Err(StorageError::new(
                    "CHECK_PLAN_CONFLICT",
                    "check plan identity changed",
                ));
            }
            if snapshot.bounds_exceeded
                || snapshot.state.status != "ready"
                || snapshot.state.content_verification_required
                || planned["mode"] != "selective"
            {
                return Err(StorageError::new(
                    "CHECK_EXECUTION_UNCERTAIN",
                    "check plan does not have a complete current index basis",
                ));
            }
            let prepared = planner::evaluate_declared_checks(&snapshot, &planned)
                .map_err(|error| StorageError::new(error.code, error.message))?;
            storage.commit_check_execution(
                &snapshot,
                &planned,
                after_generation,
                &prepared,
                &self.producer.version,
                &provenance,
                trust,
            )
        })
    }

    fn project_changes(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(100) as usize;
        let (state, changes) = self.with_storage(|storage| {
            if storage.get_project(project_id)?.is_none() {
                return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
            }
            let state = storage
                .get_project_index_state(project_id)?
                .ok_or_else(|| {
                    StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
                })?;
            if state.status != "ready" {
                return Err(StorageError::new(
                    "INDEX_RECONCILIATION_REQUIRED",
                    "hint-only index changes require authoritative reconciliation",
                ));
            }
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
            let changes =
                storage.list_project_changes_since(project_id, after_generation, limit + 1)?;
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

    fn project_dependencies_replace(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
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
        let configuration_revision = arguments
            .get("expected_configuration_revision")
            .and_then(Value::as_i64);
        let adapter_id = arguments.get("expected_adapter_id").and_then(Value::as_str);
        let adapter_version = arguments
            .get("expected_adapter_version")
            .and_then(Value::as_str);
        let configuration_guard = match (configuration_revision, adapter_id, adapter_version) {
            (None, None, None) => None,
            (Some(revision), Some(id), Some(version)) => Some((revision, id, version)),
            _ => {
                return Err(CoreCommandError::new(
                    "VALIDATION_FAILED",
                    "parser selection guard requires revision, adapter ID, and version together",
                ));
            }
        };
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
            )
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
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
                configuration_guard,
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

    fn project_dependencies_list(&self, arguments: &Value) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validation requires project_id");
        let source_path = arguments
            .get("source_path")
            .and_then(Value::as_str)
            .map(indexing::normalize_project_relative_path)
            .transpose()
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(100) as usize;
        let (state, edges) = self.with_storage(|storage| {
            if storage.get_project(project_id)?.is_none() {
                return Err(StorageError::new("PROJECT_NOT_FOUND", "project not found"));
            }
            let state = storage
                .get_project_index_state(project_id)?
                .ok_or_else(|| {
                    StorageError::new("INDEX_BASELINE_MISSING", "project baseline is missing")
                })?;
            if state.status != "ready" {
                return Err(StorageError::new(
                    "INDEX_RECONCILIATION_REQUIRED",
                    "dependency edges require authoritative reconciliation",
                ));
            }
            let edges = storage.list_project_dependency_edges(
                project_id,
                source_path.as_deref(),
                limit + 1,
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

    fn result_put(&self, request: &CommandRequest) -> Result<Value, CoreCommandError> {
        let project_id = request.arguments.get("project_id").and_then(Value::as_str);
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
            CoreCommandError::new("STORAGE_ERROR", format!("serialize result: {error}"))
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
        let result = self.with_storage(|storage| storage.get_result(id))?;
        match result {
            Some(result) => {
                authority
                    .require_project(result.project_id.as_deref())
                    .map_err(|error| CoreCommandError::new(error.code, error.message))?;
                serde_json::to_value(result).map_err(|error| {
                    CoreCommandError::new("STORAGE_ERROR", format!("serialize result: {error}"))
                })
            }
            None => Err(CoreCommandError::new(
                "RESULT_NOT_FOUND",
                "result not found",
            )),
        }
    }

    fn result_list(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments.get("project_id").and_then(Value::as_str);
        if let Some(project_id) = project_id {
            authority
                .require_project(Some(project_id))
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        } else if authority.project_ids.is_some() {
            return Err(CoreCommandError::new(
                "PROJECT_SCOPE_REQUIRED",
                "a project ID is required for scoped result listing",
            ));
        }
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 100) as usize;
        let results =
            self.with_storage(|storage| storage.list_result_descriptions(project_id, limit))?;
        Ok(json!({ "results": results }))
    }

    fn result_describe(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let id = arguments["result_id"]
            .as_str()
            .expect("registry validation requires result_id");
        let description = self.with_storage(|storage| storage.describe_result(id))?;
        match description {
            Some(description) => {
                authority
                    .require_project(description.project_id.as_deref())
                    .map_err(|error| CoreCommandError::new(error.code, error.message))?;
                serde_json::to_value(description).map_err(|error| {
                    CoreCommandError::new(
                        "STORAGE_ERROR",
                        format!("serialize result description: {error}"),
                    )
                })
            }
            None => Err(CoreCommandError::new(
                "RESULT_NOT_FOUND",
                "result not found",
            )),
        }
    }

    fn result_context(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let id = arguments["result_id"]
            .as_str()
            .expect("registry validation requires result_id");
        let max_bytes = arguments["max_bytes"]
            .as_u64()
            .expect("registry validation requires max_bytes") as usize;
        let required_pointers: Vec<&str> = arguments
            .get("required_pointers")
            .map(|value| {
                value
                    .as_array()
                    .expect("registry validation requires an array")
                    .iter()
                    .map(|pointer| {
                        pointer
                            .as_str()
                            .expect("registry validation requires strings")
                    })
                    .collect()
            })
            .unwrap_or_default();
        let focus_terms: Vec<&str> = arguments
            .get("focus_terms")
            .map(|value| {
                value
                    .as_array()
                    .expect("registry validation requires an array")
                    .iter()
                    .map(|term| term.as_str().expect("registry validation requires strings"))
                    .collect()
            })
            .unwrap_or_default();
        if focus_terms.len() > 8
            || focus_terms.iter().collect::<BTreeSet<_>>().len() != focus_terms.len()
            || focus_terms
                .iter()
                .any(|term| !context::valid_focus_term(term))
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "focus_terms must contain at most 8 unique bounded search terms",
            ));
        }
        if arguments.get("required_pointers").is_some()
            && (required_pointers.is_empty()
                || required_pointers.len() > 8
                || required_pointers.iter().collect::<BTreeSet<_>>().len()
                    != required_pointers.len()
                || required_pointers
                    .iter()
                    .any(|pointer| !context::valid_required_pointer(pointer)))
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "required_pointers must contain 1-8 unique bounded JSON Pointers",
            ));
        }
        let result = self.with_storage(|storage| storage.get_result(id))?;
        let result =
            result.ok_or_else(|| CoreCommandError::new("RESULT_NOT_FOUND", "result not found"))?;
        authority
            .require_project(result.project_id.as_deref())
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        match context::compile_result_focused(&result, max_bytes, &required_pointers, &focus_terms)
        {
            Ok(view) => Ok(view),
            Err(context::CompileError::BudgetTooSmall) => Err(CoreCommandError::new(
                "CONTEXT_BUDGET_TOO_SMALL",
                if required_pointers.is_empty() {
                    "the requested byte budget cannot hold the context envelope"
                } else {
                    "the requested byte budget cannot hold all required context facts"
                },
            )),
            Err(context::CompileError::RequiredFactUnavailable) => Err(CoreCommandError::new(
                "CONTEXT_FACT_UNAVAILABLE",
                "a required context fact is unavailable",
            )),
            Err(context::CompileError::SourceTooLarge) => Err(CoreCommandError::new(
                "CONTEXT_SOURCE_TOO_LARGE",
                "stored source exceeds the bounded context compiler input",
            )),
        }
    }
    fn context_compile(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validates project_id");
        authority
            .require_project(Some(project_id))
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let result_ids: Vec<&str> = arguments["result_ids"]
            .as_array()
            .expect("registry validates result_ids")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("registry validates result ID strings")
            })
            .collect();
        if result_ids.is_empty()
            || result_ids.len() > 8
            || result_ids.iter().collect::<BTreeSet<_>>().len() != result_ids.len()
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "result_ids must contain 1-8 unique IDs",
            ));
        }
        let required: Vec<(&str, &str)> = arguments
            .get("required_pointers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        (
                            item["result_id"]
                                .as_str()
                                .expect("registry validates result ID"),
                            item["pointer"]
                                .as_str()
                                .expect("registry validates pointer"),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if required.len() > 16
            || required.iter().collect::<BTreeSet<_>>().len() != required.len()
            || required.iter().any(|(id, pointer)| {
                !result_ids.contains(id) || !context::valid_required_pointer(pointer)
            })
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "required_pointers must name unique eligible source pointers",
            ));
        }
        let focus_terms: Vec<&str> = arguments
            .get("focus_terms")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().expect("registry validates focus terms"))
                    .collect()
            })
            .unwrap_or_default();
        if focus_terms.len() > 8
            || focus_terms.iter().collect::<BTreeSet<_>>().len() != focus_terms.len()
            || focus_terms
                .iter()
                .any(|term| !context::valid_focus_term(term))
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "focus_terms must contain at most 8 unique bounded terms",
            ));
        }
        let max_bytes = arguments["max_bytes"]
            .as_u64()
            .expect("registry validates max_bytes") as usize;
        let mut records = self.with_storage(|storage| {
            let mut records = Vec::with_capacity(result_ids.len());
            for id in &result_ids {
                let record = storage.get_result(id)?.ok_or_else(|| {
                    StorageError::new("RESULT_NOT_FOUND", "result not found in project")
                })?;
                if record.project_id.as_deref() != Some(project_id) {
                    return Err(StorageError::new(
                        "RESULT_NOT_FOUND",
                        "result not found in project",
                    ));
                }
                records.push(record);
            }
            Ok(records)
        })?;
        records.sort_by(|left, right| left.id.cmp(&right.id));
        let mut sources = Vec::with_capacity(records.len());
        for record in &records {
            sources.push(ContextSourceKey {
                id: record.id.clone(),
                payload_sha256: record.payload_sha256.clone(),
                kind: record.kind.clone(),
                created_at: record.created_at.clone(),
                trust: record.trust.clone(),
                producer_version: record.producer_version.clone(),
                schema_version: record.schema_version,
            });
        }
        let cache_key = ContextCacheKey {
            project_id: project_id.to_string(),
            sources,
            required_pointers: required
                .iter()
                .map(|(id, pointer)| ((*id).to_string(), (*pointer).to_string()))
                .collect(),
            focus_terms: focus_terms.iter().map(|term| (*term).to_string()).collect(),
            max_bytes,
        };
        if let Some(cached) = self.context_cache_lookup(&cache_key) {
            return Ok(cached);
        }
        match context::compile_project_results(
            project_id,
            &records,
            max_bytes,
            &required,
            &focus_terms,
        ) {
            Ok(view) => {
                self.context_cache_insert(cache_key, view.clone());
                Ok(view)
            }
            Err(context::CompileError::BudgetTooSmall) => Err(CoreCommandError::new(
                "CONTEXT_BUDGET_TOO_SMALL",
                "byte budget cannot hold source metadata, conflicts, and required facts",
            )),
            Err(context::CompileError::RequiredFactUnavailable) => Err(CoreCommandError::new(
                "CONTEXT_FACT_UNAVAILABLE",
                "a required exact source fact is unavailable",
            )),
            Err(context::CompileError::SourceTooLarge) => Err(CoreCommandError::new(
                "CONTEXT_SOURCE_TOO_LARGE",
                "selected stored results exceed the bounded compiler input",
            )),
        }
    }
    fn context_task_compile(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments["project_id"]
            .as_str()
            .expect("registry validates project_id");
        authority
            .require_project(Some(project_id))
            .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        let task_kind = arguments["task_kind"]
            .as_str()
            .expect("registry validates task_kind");
        let result_ids: Vec<&str> = arguments
            .get("result_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().expect("registry validates result ID"))
                    .collect()
            })
            .unwrap_or_default();
        let approval_ids: Vec<&str> = arguments
            .get("approval_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().expect("registry validates approval ID"))
                    .collect()
            })
            .unwrap_or_default();
        if result_ids.len() > 8
            || approval_ids.len() > 4
            || result_ids.iter().collect::<BTreeSet<_>>().len() != result_ids.len()
            || approval_ids.iter().collect::<BTreeSet<_>>().len() != approval_ids.len()
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "task context source IDs must be unique and bounded",
            ));
        }
        let required: Vec<(&str, &str)> = arguments
            .get("required_pointers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        (
                            item["result_id"]
                                .as_str()
                                .expect("registry validates result ID"),
                            item["pointer"]
                                .as_str()
                                .expect("registry validates pointer"),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if required.len() > 16
            || required.iter().collect::<BTreeSet<_>>().len() != required.len()
            || required.iter().any(|(id, pointer)| {
                !result_ids.contains(id) || !context::valid_required_pointer(pointer)
            })
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "required pointers must name unique eligible result facts",
            ));
        }
        let focus_terms: Vec<&str> = arguments
            .get("focus_terms")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().expect("registry validates focus term"))
                    .collect()
            })
            .unwrap_or_default();
        if focus_terms.len() > 8
            || focus_terms.iter().collect::<BTreeSet<_>>().len() != focus_terms.len()
            || focus_terms
                .iter()
                .any(|term| !context::valid_focus_term(term))
        {
            return Err(CoreCommandError::new(
                "VALIDATION_FAILED",
                "focus terms must be unique bounded literals",
            ));
        }
        let max_bytes = arguments["max_bytes"]
            .as_u64()
            .expect("registry validates max_bytes") as usize;
        let snapshot = self.with_storage(|storage| {
            storage.task_context_snapshot(project_id, &result_ids, &approval_ids)
        })?;
        match context::compile_task_project_view(
            &safe_approval_ref(project_id),
            &snapshot,
            task_kind,
            max_bytes,
            &required,
            &focus_terms,
        ) {
            Ok(view) => Ok(view),
            Err(context::CompileError::BudgetTooSmall) => Err(CoreCommandError::new(
                "CONTEXT_BUDGET_TOO_SMALL",
                "byte budget cannot hold current state, selected decision evidence, conflicts, and required facts",
            )),
            Err(context::CompileError::RequiredFactUnavailable) => Err(CoreCommandError::new(
                "CONTEXT_FACT_UNAVAILABLE",
                "a required exact source fact is unavailable",
            )),
            Err(context::CompileError::SourceTooLarge) => Err(CoreCommandError::new(
                "CONTEXT_SOURCE_TOO_LARGE",
                "selected stored results exceed bounded compiler input",
            )),
        }
    }
    fn job_checkpoint(&self, request: &CommandRequest) -> Result<Value, CoreCommandError> {
        let arguments = &request.arguments;
        let id = arguments.get("id").and_then(Value::as_str);
        let project_id = arguments.get("project_id").and_then(Value::as_str);
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
        let result_id = arguments.get("result_id").and_then(Value::as_str);
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
            CoreCommandError::new("STORAGE_ERROR", format!("serialize job: {error}"))
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
                    .map_err(|error| CoreCommandError::new(error.code, error.message))?;
                serde_json::to_value(job).map_err(|error| {
                    CoreCommandError::new("STORAGE_ERROR", format!("serialize job: {error}"))
                })
            }
            None => Err(CoreCommandError::new("JOB_NOT_FOUND", "job not found")),
        }
    }

    fn job_list(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let project_id = arguments.get("project_id").and_then(Value::as_str);
        if let Some(project_id) = project_id {
            authority
                .require_project(Some(project_id))
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        } else if authority.project_ids.is_some() {
            return Err(CoreCommandError::new(
                "PROJECT_SCOPE_REQUIRED",
                "a project ID is required for scoped job listing",
            ));
        }
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 100) as usize;
        let jobs = self.with_storage(|storage| storage.list_job_descriptions(project_id, limit))?;
        Ok(json!({ "jobs": jobs }))
    }

    fn transaction_list(
        &self,
        arguments: &Value,
        authority: &ExecutionAuthority,
    ) -> Result<Value, CoreCommandError> {
        let requested_project = arguments.get("project_id").and_then(Value::as_str);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .clamp(1, 200) as usize;

        if let Some(project_id) = requested_project {
            authority
                .require_project(Some(project_id))
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
        }

        let mut transactions =
            self.with_storage(|storage| storage.list_transactions(requested_project, limit))?;

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
        let summary = self.with_storage(|storage| storage.usage_summary())?;
        let mut value = serde_json::to_value(summary).map_err(|error| {
            CoreCommandError::new("STORAGE_ERROR", format!("serialize usage summary: {error}"))
        })?;
        value["context_cache"] = self.context_cache_summary();
        Ok(value)
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
        let project_id = arguments.get("project_id").and_then(Value::as_str);
        if project_id.is_some() {
            authority
                .require_project(project_id)
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
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
        let credential_handle = arguments.get("credential_handle").and_then(Value::as_str);
        if let Some(handle_id) = credential_handle {
            authority
                .require_credential_handle(handle_id, Some("egress"))
                .map_err(|error| CoreCommandError::new(error.code, error.message))?;
            let durable = self.with_storage(|storage| storage.get_credential_handle(handle_id))?;
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
        let decision: EgressDecision = authority.evaluate_egress(&egress_request);
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

        let record = match self.with_storage(|storage| storage.get_idempotency(key)) {
            Ok(Some(record)) => record,
            Ok(None) => return None,
            Err(error) => {
                return Some(self.failure(request, command_version, error.code, error.message));
            }
        };

        if record.command != request.command || record.request_sha256 != fingerprint {
            return Some(self.failure(
                request,
                command_version,
                "IDEMPOTENCY_CONFLICT",
                "idempotency key was already used for a different request",
            ));
        }

        match serde_json::from_str::<CommandResponse>(&record.response_json) {
            Ok(mut response) => {
                response.request_id = request.request_id.clone();
                response.producer = self.producer.clone();
                response.replayed = true;
                if request.command == "automation.checks.execute" && response.ok {
                    if let Some(result) = response.result.as_mut() {
                        result["replayed"] = json!(true);
                    }
                }
                Some(response)
            }
            Err(error) => Some(self.failure(
                request,
                command_version,
                "STORAGE_ERROR",
                format!("stored idempotency response is invalid: {error}"),
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
                    format!("serialize idempotent response: {error}"),
                );
            }
        };

        match self.with_storage(|storage| {
            storage.put_idempotency(key, &request.command, &fingerprint, &response_json)
        }) {
            Ok(()) => response,
            Err(error) if error.code == "IDEMPOTENCY_CONFLICT" => self
                .try_replay(request, command_version)
                .unwrap_or_else(|| {
                    self.failure(
                        request,
                        command_version,
                        "IDEMPOTENCY_CONFLICT",
                        "idempotency race could not be resolved",
                    )
                }),
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
    fn finish(&self, request: &CommandRequest, response: CommandResponse) -> CommandResponse {
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
                event
                    .attributes
                    .insert("command".to_string(), json!(request.command));
                event.attributes.insert(
                    "command_version".to_string(),
                    json!(response.command_version),
                );
                event
                    .attributes
                    .insert("ok".to_string(), json!(response.ok));
                event
                    .attributes
                    .insert("replayed".to_string(), json!(response.replayed));
                if let Some(error) = &response.error {
                    event
                        .attributes
                        .insert("error_code".to_string(), json!(error.code));
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
fn safe_approval_ref(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        let digest = Sha256::digest(value.as_bytes());
        let mut out = String::from("ref-sha256-");
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut out, "{byte:02x}").expect("String write");
        }
        out
    }
}

fn removal_approval_json(approval: &crate::storage::ProjectRemovalApprovalRecord) -> Value {
    json!({
        "approval_id": approval.id,
        "project_id": safe_approval_ref(&approval.project_id),
        "state": approval.state,
        "project_state_revision": approval.project_state_revision,
        "project_lifecycle_state": approval.project_lifecycle_state,
        "requester_actor": approval.requester_actor,
        "requester_client": approval.requester_client,
        "requester_delegator": approval.requester_delegator,
        "approver_actor": approval.approver_actor,
        "approver_client": approval.approver_client,
        "executor_actor": approval.executor_actor,
        "executor_client": approval.executor_client,
        "created_at_ms": approval.created_at_ms,
        "expires_at_ms": approval.expires_at_ms,
        "decided_at_ms": approval.decided_at_ms,
        "executed_at_ms": approval.executed_at_ms,
        "action": "remove_project_registration",
        "risk": "high",
        "effect": "Project is removed from active use; RELAY history and project files remain untouched.",
        "validation": "Project lifecycle and registration revision must match the plan.",
        "rollback": "Removal cannot be reversed through RELAY."
    })
}

fn request_fingerprint(request: &CommandRequest, command_version: u32) -> String {
    let canonical = canonical_json(&json!({
        "command": request.command,
        "command_version": command_version,
        "arguments": request.arguments,
        "context": request.context
    }));
    let encoded = serde_json::to_vec(&canonical).expect("canonical JSON must serialize");
    let digest = Sha256::digest(encoded);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
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
                output.insert(key.clone(), canonical_json(&object[key]));
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::{IndexChange, IndexedFileSnapshot};
    use crate::storage::{DependencyReplacement, IndexCommitMode, ProjectIndexCommit};
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

    fn request(id: &str, command: &str, arguments: Value) -> CommandRequest {
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

    #[test]
    fn check_catalog_commands_plan_without_running_checks() {
        let dir = temp_dir("check-plan-commands");
        fs::create_dir_all(&dir).unwrap();
        let core = RelayCore::open(CoreConfig::new(&dir));
        core.with_storage(|storage| {
            storage.register_project(Some("PRJ-plan"), "Plan", "file:///fixture")?;
            storage.replace_project_baseline("PRJ-plan", &[], 0)?;
            Ok(())
        })
        .unwrap();
        let mut put = request(
            "catalog-put",
            "project.check_catalog.put",
            json!({
                "project_id": "PRJ-plan", "expected_revision": 0,
                "catalog": {"format_version": 1, "checks": [
                    {"id": "check.a", "roots": ["source.txt"], "leaves": []}
                ]}
            }),
        );
        put.idempotency_key = Some("catalog-put-key".into());
        let saved = core.execute(put, &runtime());
        assert!(saved.ok, "{:?}", saved.error);
        assert_eq!(saved.result.unwrap()["revision"], 1);
        let fetched = core.execute(
            request(
                "catalog-get",
                "project.check_catalog.get",
                json!({"project_id":"PRJ-plan"}),
            ),
            &runtime(),
        );
        assert!(fetched.ok, "{:?}", fetched.error);
        let planned = core.execute(
            request(
                "check-plan",
                "automation.checks.plan",
                json!({"project_id":"PRJ-plan", "after_generation": 1}),
            ),
            &runtime(),
        );
        assert!(planned.ok, "{:?}", planned.error);
        let result = planned.result.unwrap();
        assert_eq!(result["mode"], "full_catalog_fallback");
        assert_eq!(result["checks"][0]["status"], "planned_not_run");
        assert!(result["checks"][0]["result_id"].is_null());
        let mut attempt = request(
            "fallback-run",
            "automation.checks.execute",
            json!({
                "project_id":"PRJ-plan", "after_generation":1, "plan_id":result["plan_id"]
            }),
        );
        attempt.idempotency_key = Some("fallback-run-key".into());
        let denied = core.execute(attempt, &runtime());
        assert_eq!(denied.error.unwrap().code, "CHECK_EXECUTION_UNCERTAIN");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn declared_index_checks_commit_atomically_replay_and_reject_changed_plan() {
        let dir = temp_dir("check-execute-restart");
        fs::create_dir_all(&dir).unwrap();
        let core = RelayCore::open(CoreConfig::new(&dir));
        core.with_storage(|storage| {
            storage.register_project(Some("PRJ-check-run"), "Checks", "file:///fixture")?;
            let first = IndexedFileSnapshot { relative_path: "source.txt".into(),
                size_bytes: 1, modified_unix_ns: 1, content_sha256: "a".repeat(64) };
            storage.replace_project_baseline("PRJ-check-run", &[first], 1)?;
            storage.put_project_configuration("PRJ-check-run", 0, "generic", Some("fixture"), Some("1"))?;
            let catalog = planner::canonical_catalog(&json!({
                "format_version": 1, "checks": [
                    {"id":"check.digest", "roots":["source.txt"], "leaves":[],
                     "assertion":{"kind":"indexed_file_digest", "path":"source.txt", "sha256":"b".repeat(64)}},
                    {"id":"check.native", "roots":["source.txt"], "leaves":[]}
                ]
            })).unwrap();
            storage.put_project_check_catalog("PRJ-check-run", 0, &catalog)?;
            let next = IndexedFileSnapshot { relative_path: "source.txt".into(),
                size_bytes: 2, modified_unix_ns: 2, content_sha256: "b".repeat(64) };
            let change = IndexChange { change_kind: "modified".into(), relative_path: "source.txt".into(),
                previous_path: None, before_sha256: Some("a".repeat(64)),
                after_sha256: Some("b".repeat(64)) };
            storage.apply_project_reconciliation(ProjectIndexCommit {
                project_id: "PRJ-check-run", expected_generation: 1,
                touched_files: &[next], changes: &[change], file_count: 1, total_bytes: 2,
                mode: IndexCommitMode::Authoritative, content_verified: false,
            })?;
            storage.replace_project_dependencies(DependencyReplacement {
                project_id: "PRJ-check-run", expected_generation: 2,
                source_path: "source.txt", expected_source_sha256: &"b".repeat(64),
                producer_id: "fixture", producer_version: "1",
                configuration_guard: Some((1, "fixture", "1")), targets: &[],
            })?;
            Ok(())
        }).unwrap();
        let planned = core.execute(
            request(
                "plan-run",
                "automation.checks.plan",
                json!({
                    "project_id":"PRJ-check-run", "after_generation":1
                }),
            ),
            &runtime(),
        );
        assert!(planned.ok, "{:?}", planned.error);
        let plan = planned.result.unwrap();
        assert_eq!(plan["mode"], "selective");
        assert_eq!(plan["checks"].as_array().unwrap().len(), 2);
        let args = json!({"project_id":"PRJ-check-run", "after_generation":1,
            "plan_id":plan["plan_id"]});
        let mut execute = request("run-first", "automation.checks.execute", args.clone());
        execute.idempotency_key = Some("run-first-key".into());
        let response = core.execute(execute, &runtime());
        assert!(response.ok, "{:?}", response.error);
        let first = response.result.unwrap();
        assert_eq!(first["replayed"], false);
        assert_eq!(first["passed_count"], 1);
        assert_eq!(first["untested_count"], 1);
        let result_id = first["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["check_id"] == "check.digest")
            .unwrap()["result_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            first["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|check| check["check_id"] == "check.native")
                .unwrap()["result_id"]
                .is_null()
        );
        core.with_storage(|storage| {
            let stored = storage.get_result(&result_id)?.unwrap();
            let payload = serde_json::to_string(&stored.payload).unwrap();
            assert!(!payload.contains("source.txt"));
            assert_eq!(stored.payload["native_workflow_status"], "untested");
            assert_eq!(
                storage
                    .list_result_descriptions(Some("PRJ-check-run"), 10)?
                    .len(),
                1
            );
            Ok(())
        })
        .unwrap();
        drop(core);

        let reopened = RelayCore::open(CoreConfig::new(&dir));
        let mut replay = request("run-replay", "automation.checks.execute", args.clone());
        replay.idempotency_key = Some("run-replay-key".into());
        let replayed = reopened.execute(replay, &runtime());
        assert!(replayed.ok, "{:?}", replayed.error);
        let replayed = replayed.result.unwrap();
        assert_eq!(replayed["replayed"], true);
        assert_eq!(
            replayed["checks"][0]["result_id"],
            first["checks"][0]["result_id"]
        );
        reopened
            .with_storage(|storage| {
                assert_eq!(
                    storage
                        .list_result_descriptions(Some("PRJ-check-run"), 10)?
                        .len(),
                    1
                );
                Ok(())
            })
            .unwrap();
        let mut replace = request(
            "replace-catalog",
            "project.check_catalog.put",
            json!({
                "project_id":"PRJ-check-run", "expected_revision":1,
                "catalog":{"format_version":1,"checks":[{"id":"check.changed","roots":["source.txt"],"leaves":[]}]}
            }),
        );
        replace.idempotency_key = Some("replace-catalog-key".into());
        assert!(reopened.execute(replace, &runtime()).ok);
        let mut same_logical_retry = request("run-first-retry", "automation.checks.execute", args.clone());
        same_logical_retry.idempotency_key = Some("run-first-key".into());
        let prior = reopened.execute(same_logical_retry, &runtime());
        assert!(prior.ok);
        assert_eq!(prior.result.unwrap()["replayed"], true);
        let mut stale = request("run-stale", "automation.checks.execute", args);
        stale.idempotency_key = Some("run-stale-key".into());
        let denied = reopened.execute(stale, &runtime());
        assert_eq!(denied.error.unwrap().code, "CHECK_PLAN_CONFLICT");
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn project_configuration_is_scoped_versioned_and_restart_safe() {
        let dir = temp_dir("project-configuration");
        fs::create_dir_all(&dir).unwrap();
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();
        for project_id in ["PRJ-config-a", "PRJ-config-b"] {
            let mut register = request(
                &format!("register-{project_id}"),
                "project.register",
                json!({ "id": project_id, "name": project_id, "root_uri": "file:///fixture" }),
            );
            register.idempotency_key = Some(format!("key-register-{project_id}"));
            assert!(core.execute(register, &runtime).ok);
        }
        let missing = core.execute(
            request(
                "config-missing",
                "project.configuration.get",
                json!({
                    "project_id": "PRJ-config-a"
                }),
            ),
            &runtime,
        );
        assert!(missing.ok);
        assert_eq!(missing.result.unwrap()["revision"], 0);

        let mut put = request(
            "config-put",
            "project.configuration.put",
            json!({
                "project_id": "PRJ-config-a", "expected_revision": 0,
                "format_version": 1, "project_type": "synthetic.project",
                "adapter_id": "fixture.parser", "adapter_version": "1.2.0+fixture"
            }),
        );
        put.idempotency_key = Some("key-config-put".to_string());
        let saved = core.execute(put, &runtime);
        assert!(saved.ok, "{:?}", saved.error);
        assert_eq!(saved.result.unwrap()["revision"], 1);

        let mut stale_put = request(
            "config-stale",
            "project.configuration.put",
            json!({
                "project_id": "PRJ-config-a", "expected_revision": 0,
                "format_version": 1, "project_type": "synthetic.other"
            }),
        );
        stale_put.idempotency_key = Some("key-config-stale".to_string());
        assert_eq!(
            core.execute(stale_put, &runtime).error.unwrap().code,
            "PROJECT_CONFIG_CONFLICT"
        );
        let mut invalid = request(
            "config-invalid",
            "project.configuration.put",
            json!({
                "project_id": "PRJ-config-a", "expected_revision": 1,
                "format_version": 1, "project_type": "synthetic.project",
                "adapter_id": "fixture.parser"
            }),
        );
        invalid.idempotency_key = Some("key-config-invalid".to_string());
        assert_eq!(
            core.execute(invalid, &runtime).error.unwrap().code,
            "VALIDATION_FAILED"
        );

        let mut scoped = ExecutionAuthority::local_user("scoped-config-reader");
        scoped.project_ids = Some(["PRJ-config-b".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "config-cross-project",
                "project.configuration.get",
                json!({
                    "project_id": "PRJ-config-a"
                }),
            ),
            &runtime,
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        drop(core);

        let reopened = RelayCore::open(CoreConfig::new(&dir));
        let read = reopened.execute(
            request(
                "config-restart",
                "project.configuration.get",
                json!({
                    "project_id": "PRJ-config-a"
                }),
            ),
            &runtime,
        );
        assert!(read.ok, "{:?}", read.error);
        let config = read.result.unwrap();
        assert_eq!(config["revision"], 1);
        assert_eq!(config["adapter_id"], "fixture.parser");
        assert_eq!(config["adapter_version"], "1.2.0+fixture");
        let other = reopened.execute(
            request(
                "config-other",
                "project.configuration.get",
                json!({
                    "project_id": "PRJ-config-b"
                }),
            ),
            &runtime,
        );
        assert_eq!(other.result.unwrap()["configured"], false);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
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
        response.result.unwrap()["id"].as_str().unwrap().to_string()
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
        assert_eq!(response.error.as_ref().unwrap().code, "VALIDATION_FAILED");

        let list = core.execute(request("REQ-list", "project.list", json!({})), &runtime());
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
        assert_eq!(result["provenance"]["actor_id"], "ACTOR-fixture");
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
        checkpoint.idempotency_key = Some("IDEMP-job".to_string());
        let job_response = core.execute(checkpoint, &runtime());
        assert!(job_response.ok);
        let job = job_response.result.unwrap();
        assert_eq!(job["provenance"]["client_id"], "CLIENT-test");
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
        assert_eq!(result_get.result.unwrap()["payload"]["value"], 42);
        let job_id = job["id"].as_str().unwrap();
        let job_get = reopened.execute(
            request("REQ-get-job", "job.get", json!({ "job_id": job_id })),
            &runtime(),
        );
        assert_eq!(job_get.result.unwrap()["checkpoint"]["stage"], 2);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn result_description_omits_payload_and_enforces_project_scope() {
        let dir = temp_dir("result-description");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);
        let payload = json!({ "text": "é".repeat(32_768) });
        let payload_bytes = serde_json::to_vec(&payload).unwrap().len();
        let mut put = request(
            "REQ-describe-put",
            "result.put",
            json!({
                "project_id": project_id,
                "kind": "TEST",
                "payload": payload
            }),
        );
        put.idempotency_key = Some("IDEMP-describe-put".to_string());
        let stored = core.execute(put, &runtime());
        assert!(stored.ok, "{:?}", stored.error);
        let stored = stored.result.unwrap();
        let result_id = stored["id"].as_str().unwrap();

        let described = core.execute(
            request(
                "REQ-describe",
                "result.describe",
                json!({ "result_id": result_id }),
            ),
            &runtime(),
        );
        assert!(described.ok, "{:?}", described.error);
        let described = described.result.unwrap();
        assert_eq!(described["payload_sha256"], stored["payload_sha256"]);
        assert_eq!(described["payload_bytes"], payload_bytes);
        assert_eq!(described["project_id"], project_id);
        assert!(described.get("payload").is_none());
        assert!(described.get("provenance").is_none());
        assert!(serde_json::to_vec(&described).unwrap().len() < 1_024);

        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "REQ-describe-denied",
                "result.describe",
                json!({ "result_id": result_id }),
            ),
            &runtime(),
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        drop(core);
        let reopened = RelayCore::open(CoreConfig::new(&dir));
        let described = reopened.execute(
            request(
                "REQ-describe-reopened",
                "result.describe",
                json!({ "result_id": result_id }),
            ),
            &runtime(),
        );
        assert!(described.ok);
        assert_eq!(described.result.unwrap()["payload_bytes"], payload_bytes);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn result_context_is_bounded_and_project_scoped() {
        let dir = temp_dir("result-context");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);
        let mut put = request(
            "REQ-context-put",
            "result.put",
            json!({
                "project_id": project_id,
                "kind": "TEST",
                "payload": {
                    "result_id": "RES-exact",
                    "failure_count": 7,
                    "private_path": "C:\\Users\\private\\project",
                    "api_token": "private-token",
                    "logs": vec!["repeated narrative"; 300]
                }
            }),
        );
        put.idempotency_key = Some("IDEMP-context-put".to_string());
        let stored = core.execute(put, &runtime());
        assert!(stored.ok, "{:?}", stored.error);
        let result_id = stored.result.unwrap()["id"].as_str().unwrap().to_string();

        let context_request = || {
            request(
                "REQ-context-read",
                "result.context",
                json!({ "result_id": result_id, "max_bytes": 512 }),
            )
        };
        let compact = core.execute(context_request(), &runtime());
        assert!(compact.ok, "{:?}", compact.error);
        let compact = compact.result.unwrap();
        let serialized = serde_json::to_string(&compact).unwrap();
        assert!(serialized.len() <= 512);
        assert_eq!(compact["result_id"], result_id);
        assert_eq!(compact["full_result_command"], "result.get");
        assert!(!serialized.contains("C:\\Users"));
        assert!(!serialized.contains("private-token"));
        let full = core.execute(
            request(
                "REQ-context-full",
                "result.get",
                json!({ "result_id": result_id }),
            ),
            &runtime(),
        );
        assert!(full.ok);
        assert_eq!(full.result.unwrap()["payload"]["failure_count"], 7);

        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(context_request(), &runtime(), &scoped);
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn required_context_facts_are_exact_scoped_and_fail_closed() {
        let dir = temp_dir("required-context");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);
        let mut entries: Vec<Value> = (0..401)
            .map(|index| json!({ "event_code": format!("EVENT-{index:03}") }))
            .collect();
        entries[200] = json!({ "target_code": "TARGET-EXACT-42" });
        let mut put = request(
            "REQ-required-put",
            "result.put",
            json!({
                "project_id": project_id,
                "payload": {
                    "entries": entries,
                    "api_token": "PRIVATE-TOKEN",
                    "coordinate": { "x": 12.125 }
                }
            }),
        );
        put.idempotency_key = Some("IDEMP-required-put".to_string());
        let stored = core.execute(put, &runtime());
        assert!(stored.ok, "{:?}", stored.error);
        let result_id = stored.result.unwrap()["id"].as_str().unwrap().to_string();
        let read = |pointers: Value, max_bytes: usize| {
            core.execute(
                request(
                    "REQ-required-read",
                    "result.context",
                    json!({
                        "result_id": result_id,
                        "max_bytes": max_bytes,
                        "required_pointers": pointers
                    }),
                ),
                &runtime(),
            )
        };

        let focused = read(json!(["/entries/200/target_code"]), 1024);
        assert!(focused.ok, "{:?}", focused.error);
        let focused = focused.result.unwrap();
        assert!(serde_json::to_vec(&focused).unwrap().len() <= 1024);
        assert!(focused["facts"].as_array().unwrap().iter().any(|fact| {
            fact["pointer"] == "/entries/200/target_code" && fact["value"] == "TARGET-EXACT-42"
        }));
        let pair = read(
            json!(["/entries/200/target_code", "/entries/0/event_code"]),
            1024,
        );
        assert!(pair.ok, "{:?}", pair.error);
        let pair = pair.result.unwrap();
        let pair_facts = pair["facts"].as_array().unwrap();
        assert_eq!(pair_facts[0]["pointer"], "/entries/200/target_code");
        assert_eq!(pair_facts[0]["value"], "TARGET-EXACT-42");
        assert_eq!(pair_facts[1]["pointer"], "/entries/0/event_code");
        assert_eq!(pair_facts[1]["value"], "EVENT-000");

        for pointer in ["/api_token", "/coordinate/x", "/missing"] {
            let failed = read(json!([pointer]), 1024);
            assert_eq!(
                failed.error.as_ref().unwrap().code,
                "CONTEXT_FACT_UNAVAILABLE"
            );
            let serialized = serde_json::to_string(&failed).unwrap();
            assert!(!serialized.contains("PRIVATE-TOKEN"));
            assert!(!serialized.contains("12.125"));
            assert!(!serialized.contains(pointer));
        }
        for pointer in ["entries/200/target_code", "/entries/~2", "/entries/~"] {
            let failed = read(json!([pointer]), 1024);
            assert_eq!(failed.error.unwrap().code, "VALIDATION_FAILED");
        }
        assert_eq!(
            read(json!(vec!["/entries/200/target_code"; 2]), 1024)
                .error
                .unwrap()
                .code,
            "VALIDATION_FAILED"
        );

        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "REQ-required-denied",
                "result.context",
                json!({
                    "result_id": result_id,
                    "max_bytes": 1024,
                    "required_pointers": ["/entries/200/target_code"]
                }),
            ),
            &runtime(),
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        let mut large = request(
            "REQ-required-large-put",
            "result.put",
            json!({
                "project_id": project_id,
                "payload": { "items": vec![json!({ "code": "X".repeat(128) }); 8] }
            }),
        );
        large.idempotency_key = Some("IDEMP-required-large-put".to_string());
        let stored = core.execute(large, &runtime());
        assert!(stored.ok, "{:?}", stored.error);
        let large_id = stored.result.unwrap()["id"].as_str().unwrap().to_string();
        let pointers: Vec<String> = (0..8).map(|index| format!("/items/{index}/code")).collect();
        let failed = core.execute(
            request(
                "REQ-required-too-large",
                "result.context",
                json!({
                    "result_id": large_id, "max_bytes": 512, "required_pointers": pointers
                }),
            ),
            &runtime(),
        );
        assert_eq!(failed.error.unwrap().code, "CONTEXT_BUDGET_TOO_SMALL");

        drop(core);
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
            req.idempotency_key = Some("IDEMP-replay".to_string());
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
        assert_eq!(replay.result.unwrap()["id"].as_str().unwrap(), first_id);

        let conflict = core.execute(make("REQ-conflict", 8), &runtime());
        assert!(!conflict.ok);
        assert_eq!(conflict.error.unwrap().code, "IDEMPOTENCY_CONFLICT");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn host_extension_is_not_called_for_denied_project_scope() {
        let dir = temp_dir("extension-authority");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let mut authority = ExecutionAuthority::local_user("CLIENT-scoped");
        authority.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let called = std::cell::Cell::new(false);
        let response = core.execute_authorized_with_extension(
            request(
                "REQ-denied-uefn",
                "uefn.static.inspect",
                json!({
                    "project_id": "PRJ-denied"
                }),
            ),
            &runtime(),
            &authority,
            |_| {
                called.set(true);
                None
            },
        );
        assert_eq!(response.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        assert!(!called.get());
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_tool_discovery_routes_through_host_extension() {
        let dir = temp_dir("tool-discovery-extension");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let authority = ExecutionAuthority::local_user("CLIENT-tools");
        let result = json!({
            "scope": "common_locations",
            "tools": [
                {"id":"uefn","detection_status":"not_detected","detection_source":null,"workflow_status":"untested"},
                {"id":"blender","detection_status":"not_detected","detection_source":null,"workflow_status":"untested"},
                {"id":"krita","detection_status":"not_detected","detection_source":null,"workflow_status":"untested"}
            ]
        });
        let response = core.execute_authorized_with_extension(
            request("REQ-tools", "tools.local.discover", json!({})),
            &runtime(),
            &authority,
            |_| Some(Ok(result.clone())),
        );
        assert!(response.ok, "{response:?}");
        assert_eq!(response.result, Some(result));

        let unavailable = core.execute_authorized_with_extension(
            request("REQ-tools-missing", "tools.local.discover", json!({})),
            &runtime(),
            &authority,
            |_| None,
        );
        assert_eq!(unavailable.error.unwrap().code, "INTEGRATION_UNAVAILABLE");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn result_list_exposes_metadata_only_and_requires_scope() {
        let dir = temp_dir("result-list-scope");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);
        let mut put = request(
            "REQ-list-put",
            "result.put",
            json!({
                "project_id": project_id, "kind": "TEST", "payload": { "secret": "PRIVATE-LIST-PAYLOAD" }
            }),
        );
        put.idempotency_key = Some("IDEMP-list-put".to_string());
        assert!(core.execute(put, &runtime()).ok);

        let listed = core.execute(
            request(
                "REQ-list-results",
                "result.list",
                json!({
                    "project_id": project_id, "limit": 10
                }),
            ),
            &runtime(),
        );
        assert!(listed.ok, "{listed:?}");
        let body = listed.result.unwrap();
        assert_eq!(body["results"].as_array().unwrap().len(), 1);
        let serialized = body.to_string();
        assert!(!serialized.contains("PRIVATE-LIST-PAYLOAD"));
        assert!(!serialized.contains("provenance"));

        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "REQ-list-denied",
                "result.list",
                json!({
                    "project_id": project_id
                }),
            ),
            &runtime(),
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        let required = core.execute_authorized(
            request("REQ-list-required", "result.list", json!({})),
            &runtime(),
            &scoped,
        );
        assert_eq!(required.error.unwrap().code, "PROJECT_SCOPE_REQUIRED");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn job_list_exposes_metadata_only_and_requires_scope() {
        let dir = temp_dir("job-list-scope");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let project_id = register_project(&core);
        let mut checkpoint = request(
            "REQ-list-job-put",
            "job.checkpoint",
            json!({
                "project_id": project_id, "command": "fixture.work", "state": "CHECKPOINTED",
                "checkpoint": { "secret": "PRIVATE-JOB-CHECKPOINT" }
            }),
        );
        checkpoint.idempotency_key = Some("IDEMP-list-job-put".to_string());
        assert!(core.execute(checkpoint, &runtime()).ok);
        let listed = core.execute(
            request(
                "REQ-list-jobs",
                "job.list",
                json!({
                    "project_id": project_id
                }),
            ),
            &runtime(),
        );
        assert!(listed.ok, "{listed:?}");
        let body = listed.result.unwrap();
        assert_eq!(body["jobs"].as_array().unwrap().len(), 1);
        let serialized = body.to_string();
        assert!(!serialized.contains("PRIVATE-JOB-CHECKPOINT"));
        assert!(!serialized.contains("provenance"));
        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "REQ-job-list-denied",
                "job.list",
                json!({
                    "project_id": project_id
                }),
            ),
            &runtime(),
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        let required = core.execute_authorized(
            request("REQ-job-list-required", "job.list", json!({})),
            &runtime(),
            &scoped,
        );
        assert_eq!(required.error.unwrap().code, "PROJECT_SCOPE_REQUIRED");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diagnostic_summary_excludes_command_arguments_and_raw_history() {
        let dir = temp_dir("diagnostic-summary");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let secret = "PRIVATE-DIAGNOSTIC-INPUT";
        let echoed = core.execute(
            request(
                "REQ-private-echo",
                "system.echo",
                json!({ "secret": secret }),
            ),
            &runtime(),
        );
        assert!(echoed.ok);
        let failed = core.execute(
            request(
                "REQ-private-failure",
                "diagnostics.summary",
                json!({ "secret": secret, "unexpected": secret }),
            ),
            &runtime(),
        );
        assert!(!failed.ok);
        let summary = core.execute(
            request("REQ-diagnostic-summary", "diagnostics.summary", json!({})),
            &runtime(),
        );
        assert!(summary.ok, "{:?}", summary.error);
        let body = serde_json::to_string(&summary.result.unwrap()).unwrap();
        assert!(body.contains("relay_version"));
        assert!(body.contains("by_severity"));
        assert!(body.contains("VALIDATION_FAILED"));
        assert!(body.contains("relay.command.completed"));
        assert!(!body.contains(secret));
        assert!(!body.contains("raw_history"));
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn automation_pause_is_visible_and_does_not_block_ordinary_commands() {
        let dir = temp_dir("automation-pause");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let unavailable = core.execute(
            request("REQ-pause-unavailable", "automation.pause", json!({})),
            &runtime(),
        );
        assert_eq!(unavailable.error.unwrap().code, "AUTOMATION_UNAVAILABLE");
        core.set_automation_available(true);
        let paused = core.execute(
            request("REQ-pause", "automation.pause", json!({})),
            &runtime(),
        );
        assert!(paused.ok, "{:?}", paused.error);
        assert_eq!(paused.result.unwrap()["mode"], "paused");
        assert!(core.automation_paused());
        let status = core.execute(
            request("REQ-paused-status", "system.status", json!({})),
            &runtime(),
        );
        assert_eq!(status.result.unwrap()["automation_mode"], "paused");
        let diagnostics = core.execute(
            request("REQ-paused-diag", "diagnostics.summary", json!({})),
            &runtime(),
        );
        assert_eq!(diagnostics.result.unwrap()["automation_mode"], "paused");
        assert!(
            core.execute(
                request("REQ-paused-echo", "system.echo", json!({ "ok": true })),
                &runtime()
            )
            .ok
        );
        let resumed = core.execute(
            request("REQ-resume", "automation.resume", json!({})),
            &runtime(),
        );
        assert!(resumed.ok, "{:?}", resumed.error);
        assert_eq!(resumed.result.unwrap()["mode"], "running");
        assert!(!core.automation_paused());
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn storage_and_diagnostics_health_degrade_independently() {
        let bad_storage_dir = temp_dir("bad-storage");
        fs::create_dir_all(&bad_storage_dir).unwrap();
        fs::write(bad_storage_dir.join("relay.sqlite3"), b"not sqlite").unwrap();
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
        assert_eq!(blocked.error.unwrap().code, "STORAGE_UNAVAILABLE");
        drop(core);
        fs::remove_dir_all(bad_storage_dir).unwrap();

        let bad_diag_dir = temp_dir("bad-diagnostics");
        fs::create_dir_all(&bad_diag_dir).unwrap();
        fs::write(bad_diag_dir.join("diagnostics"), b"blocks directory").unwrap();
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

        let mut effect_denied = ExecutionAuthority::local_user("CLIENT-effect");
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

        let response = core.execute_authorized(spoofed, &runtime, &authority);
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
        let result_id = result.result.unwrap()["id"].as_str().unwrap().to_string();

        let mut authority = ExecutionAuthority::local_user("CLIENT-scoped");
        authority.project_ids = Some(["PRJ-a".to_string()].into_iter().collect());

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
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        let mut cross_write = request(
            "REQ-cross-write",
            "result.put",
            json!({
                "project_id": "PRJ-b",
                "kind": "TEST",
                "payload": { "blocked": true }
            }),
        );
        cross_write.idempotency_key = Some("IDEMP-cross-write".to_string());
        let denied_write = core.execute_authorized(cross_write, &runtime, &authority);
        assert!(!denied_write.ok);
        assert_eq!(denied_write.error.unwrap().code, "PROJECT_SCOPE_DENIED");

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
        let mut hinted = request(
            "REQ-deny-hint-update",
            "project.index.apply_hints",
            json!({ "project_id": "PRJ-b", "hints": ["same.txt"] }),
        );
        hinted.idempotency_key = Some("IDEMP-deny-hint-update".to_string());
        let denied = core.execute_authorized(hinted, &runtime, &authority);
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
        let source_sha = files
            .iter()
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
            make_replace(
                "REQ-dep-wrong-hash",
                1,
                &"0".repeat(64),
                json!(["src/b.txt"]),
            ),
            &runtime,
        );
        assert_eq!(wrong_hash.error.unwrap().code, "INDEX_SOURCE_CHANGED");
        let missing_target = core.execute(
            make_replace(
                "REQ-dep-missing",
                1,
                &source_sha,
                json!(["src/missing.txt"]),
            ),
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
        assert_eq!(
            listed.result.unwrap()["edges"][0]["target_path"],
            "src/b.txt"
        );

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
        assert!(
            listed.result.unwrap()["edges"]
                .as_array()
                .unwrap()
                .is_empty()
        );
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
            make_replace(
                "REQ-dep-stale-after-change",
                1,
                &source_sha,
                json!(["src/b.txt"]),
            ),
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
            req.idempotency_key = Some("IDEMP-transaction-replay".to_string());
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

        let usage = core.execute(request("REQ-usage", "usage.summary", json!({})), &runtime);
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

        let mut authority = ExecutionAuthority::local_user("CLIENT-egress");
        authority.actor_id = "AGENT-egress".to_string();
        authority.delegator_id = Some("USER-owner".to_string());
        authority.project_ids = Some(["PRJ-egress".to_string()].into_iter().collect());
        authority.egress.destinations.insert(
            "remote-ai".to_string(),
            DestinationPolicy {
                remote: true,
                allowed_classes: [DataClass::Project].into_iter().collect(),
                allowed_modalities: ["text".to_string()].into_iter().collect(),
                allowed_projects: Some(["PRJ-egress".to_string()].into_iter().collect()),
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
        assert!(
            blocked.result.as_ref().unwrap()["reason"]
                .as_str()
                .unwrap()
                .contains("local-only")
        );

        authority.egress.local_only = false;
        let handle = CredentialHandle::active(
            "github.connection.fixture",
            "github",
            ["egress".to_string()],
        );
        core.register_credential_handle_metadata(&handle).unwrap();
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
        assert!(
            allowed.result.as_ref().unwrap()["ledger_id"]
                .as_str()
                .unwrap()
                .starts_with("EGR-")
        );

        core.revoke_credential_handle_metadata("github.connection.fixture")
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
        assert_eq!(revoked.error.unwrap().code, "CREDENTIAL_HANDLE_REVOKED");

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
    fn task_context_separates_current_state_from_selected_historical_evidence() {
        let dir = temp_dir("task-context");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();
        for project_id in ["PRJ-task-a", "PRJ-task-b"] {
            let mut register = request(
                &format!("REQ-{project_id}"),
                "project.register",
                json!({
                    "id": project_id, "name": "Fixture", "root_uri": "file:///private/project"
                }),
            );
            register.idempotency_key = Some(format!("IDEMP-{project_id}"));
            assert!(core.execute(register, &runtime).ok);
        }
        let mut put = request(
            "REQ-task-result",
            "result.put",
            json!({
                "project_id": "PRJ-task-a", "kind": "TEST", "payload": {
                    "failure_count": 3, "status": "FAILED", "api_token": "SECRET", "project_path": "C:/private/project"
                }
            }),
        );
        put.idempotency_key = Some("IDEMP-task-result".to_string());
        let result_id = core.execute(put, &runtime).result.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut plan = request(
            "REQ-task-plan",
            "project.removal.plan",
            json!({"project_id":"PRJ-task-a"}),
        );
        plan.idempotency_key = Some("IDEMP-task-plan".to_string());
        let approval_id = core.execute(plan, &runtime).result.unwrap()["approval_id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut decide = request(
            "REQ-task-decide",
            "project.removal.decide",
            json!({
                "project_id":"PRJ-task-a", "approval_id":approval_id, "decision":"approve"
            }),
        );
        decide.idempotency_key = Some("IDEMP-task-decide".to_string());
        assert!(core.execute(decide, &runtime).ok);
        let args = json!({
            "project_id":"PRJ-task-a", "task_kind":"project_admin", "max_bytes":8192,
            "result_ids":[result_id], "approval_ids":[approval_id],
            "required_pointers":[{"result_id":result_id,"pointer":"/failure_count"}],
            "focus_terms":["failure"]
        });
        let view = core.execute(
            request("REQ-task-context", "context.task.compile", args.clone()),
            &runtime,
        );
        assert!(view.ok, "{view:?}");
        let view = view.result.unwrap();
        assert_eq!(view["project_state"]["index_state"], "unknown");
        assert_eq!(
            view["result_currentness"],
            "unknown_without_project_generation_link"
        );
        assert_eq!(view["decision_evidence"][0]["recorded_decision"], "approve");
        assert_eq!(view["decision_evidence"][0]["human_presence"], "unverified");
        assert_eq!(
            view["decision_evidence"][0]["source_trust"],
            "durable_local_record"
        );
        assert!(
            view["result_context"]["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|fact| fact["pointer"] == "/failure_count" && fact["value"] == 3)
        );
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(serialized.len() <= 8192);
        assert!(!serialized.contains("SECRET"));
        assert!(!serialized.contains("C:/private"));
        assert!(!serialized.contains("local-user"));
        let mut too_small = args.clone();
        too_small["max_bytes"] = json!(1024);
        assert_eq!(
            core.execute(
                request("REQ-task-small", "context.task.compile", too_small),
                &runtime
            )
            .error
            .unwrap()
            .code,
            "CONTEXT_BUDGET_TOO_SMALL"
        );
        let mut unsafe_required = args.clone();
        unsafe_required["required_pointers"] =
            json!([{"result_id":result_id,"pointer":"/api_token"}]);
        assert_eq!(
            core.execute(
                request("REQ-task-private", "context.task.compile", unsafe_required),
                &runtime
            )
            .error
            .unwrap()
            .code,
            "CONTEXT_FACT_UNAVAILABLE"
        );
        core.with_storage(|storage| storage.replace_project_baseline("PRJ-task-a", &[], 0))
            .unwrap();
        let ready = core.execute(
            request("REQ-task-ready", "context.task.compile", args.clone()),
            &runtime,
        );
        let ready = ready.result.unwrap();
        assert_eq!(ready["project_state"]["index_state"], "ready");
        assert!(ready["project_state"]["last_reconciled_at"].is_string());
        core.with_storage(|storage| storage.mark_project_index_stale("PRJ-task-a"))
            .unwrap();
        let stale_index = core.execute(
            request("REQ-task-stale-index", "context.task.compile", args.clone()),
            &runtime,
        );
        assert_eq!(
            stale_index.result.unwrap()["project_state"]["index_state"],
            "stale"
        );
        let mut foreign_plan = request(
            "REQ-task-foreign-plan",
            "project.removal.plan",
            json!({"project_id":"PRJ-task-b"}),
        );
        foreign_plan.idempotency_key = Some("IDEMP-task-foreign-plan".to_string());
        let foreign_approval_id =
            core.execute(foreign_plan, &runtime).result.unwrap()["approval_id"]
                .as_str()
                .unwrap()
                .to_string();
        let mut foreign = args.clone();
        foreign["approval_ids"] = json!([foreign_approval_id]);
        assert_eq!(
            core.execute(
                request("REQ-task-foreign", "context.task.compile", foreign),
                &runtime
            )
            .error
            .unwrap()
            .code,
            "APPROVAL_NOT_FOUND"
        );
        let mut change = request(
            "REQ-task-change",
            "project.register",
            json!({
                "id":"PRJ-task-a", "name":"New", "root_uri":"file:///different"
            }),
        );
        change.idempotency_key = Some("IDEMP-task-change".to_string());
        assert!(core.execute(change, &runtime).ok);
        let stale = core.execute(
            request(
                "REQ-task-context-stale",
                "context.task.compile",
                args.clone(),
            ),
            &runtime,
        );
        assert_eq!(
            stale.result.unwrap()["decision_evidence"][0]["state"],
            "stale"
        );
        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-task-b".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request("REQ-task-context-denied", "context.task.compile", args),
            &runtime,
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn context_compiler_uses_only_authorized_project_results() {
        let dir = temp_dir("context-compiler");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();
        for project_id in ["PRJ-a", "PRJ-b"] {
            let mut register = request(
                &format!("REQ-register-{project_id}"),
                "project.register",
                json!({ "id": project_id, "name": project_id, "root_uri": format!("file:///{project_id}") }),
            );
            register.idempotency_key = Some(format!("IDEMP-register-{project_id}"));
            assert!(core.execute(register, &runtime).ok);
        }
        let mut ids = Vec::new();
        for (index, project_id, status) in [
            (0, "PRJ-a", "OLD"),
            (1, "PRJ-a", "NEW"),
            (2, "PRJ-b", "OTHER"),
        ] {
            let mut put = request(
                &format!("REQ-context-source-{index}"),
                "result.put",
                json!({ "project_id": project_id, "kind": "TEST", "payload": { "status": status, "failure_count": index } }),
            );
            put.idempotency_key = Some(format!("IDEMP-context-source-{index}"));
            let response = core.execute(put, &runtime);
            assert!(response.ok);
            ids.push(response.result.unwrap()["id"].as_str().unwrap().to_string());
        }
        let args = json!({ "project_id": "PRJ-a", "result_ids": [&ids[0], &ids[1]],
            "max_bytes": 4096, "required_pointers": [{ "result_id": ids[0], "pointer": "/status" }] });
        let view = core.execute(
            request("REQ-compile", "context.compile", args.clone()),
            &runtime,
        );
        assert!(view.ok, "{view:?}");
        let output = view.result.unwrap();
        assert_eq!(output["conflict_count"], 2);
        assert_eq!(output["sources"].as_array().unwrap().len(), 2);
        assert_eq!(output["freshness_basis"], "stored_source_timestamps_only");
        assert!(serde_json::to_vec(&output).unwrap().len() <= 4096);
        let first_usage = core.execute(
            request("REQ-cache-first", "usage.summary", json!({})),
            &runtime,
        );
        let first_cache = &first_usage.result.as_ref().unwrap()["context_cache"];
        assert_eq!(first_cache["misses"], 1);
        assert_eq!(first_cache["hits"], 0);
        let repeated = core.execute(
            request("REQ-compile-repeat", "context.compile", args.clone()),
            &runtime,
        );
        assert!(repeated.ok, "{repeated:?}");
        assert_eq!(repeated.result.unwrap(), output);
        let reused = core.execute(
            request("REQ-cache-reused", "usage.summary", json!({})),
            &runtime,
        );
        let reused_cache = &reused.result.as_ref().unwrap()["context_cache"];
        assert_eq!(reused_cache["hits"], 1);
        assert_eq!(reused_cache["source_fact_collections_avoided"], 2);
        assert_eq!(reused_cache["entries"], 1);
        let mut focused = args.clone();
        focused["focus_terms"] = json!(["status"]);
        assert!(
            core.execute(
                request("REQ-compile-focused", "context.compile", focused),
                &runtime
            )
            .ok
        );
        let distinct = core.execute(
            request("REQ-cache-distinct", "usage.summary", json!({})),
            &runtime,
        );
        assert_eq!(distinct.result.unwrap()["context_cache"]["misses"], 2);

        let mut foreign = args.clone();
        foreign["result_ids"] = json!([ids[0], ids[2]]);
        let denial = core.execute(
            request("REQ-compile-foreign", "context.compile", foreign),
            &runtime,
        );
        assert_eq!(denial.error.unwrap().code, "RESULT_NOT_FOUND");
        let mut scoped = ExecutionAuthority::local_user("CLIENT-b");
        scoped.project_ids = Some(["PRJ-b".to_string()].into_iter().collect());
        let denial = core.execute_authorized(
            request("REQ-compile-denied", "context.compile", args),
            &runtime,
            &scoped,
        );
        assert_eq!(denial.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        let after_denial = core.execute(
            request("REQ-cache-after-denial", "usage.summary", json!({})),
            &runtime,
        );
        assert_eq!(after_denial.result.unwrap()["context_cache"]["hits"], 1);
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn project_lifecycle_hides_active_discovery_and_preserves_history() {
        let dir = temp_dir("project-lifecycle");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let runtime = runtime();
        let mut register = request(
            "REQ-lifecycle-register",
            "project.register",
            json!({ "id": "PRJ-life", "name": "Fixture", "root_uri": "file:///fixture" }),
        );
        register.idempotency_key = Some("IDEMP-lifecycle-register".to_string());
        assert!(core.execute(register, &runtime).ok);

        let mut put = request(
            "REQ-lifecycle-result",
            "result.put",
            json!({ "project_id": "PRJ-life", "kind": "TEST", "payload": { "value": 7 } }),
        );
        put.idempotency_key = Some("IDEMP-lifecycle-result".to_string());
        let result = core.execute(put, &runtime);
        assert!(result.ok);
        let result_id = result.result.unwrap()["id"].as_str().unwrap().to_string();

        let mut scoped = ExecutionAuthority::local_user("CLIENT-other");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized(
            request(
                "REQ-life-denied",
                "project.archive",
                json!({ "project_id": "PRJ-life" }),
            ),
            &runtime,
            &scoped,
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");

        let mut archive = request(
            "REQ-lifecycle-archive",
            "project.archive",
            json!({ "project_id": "PRJ-life" }),
        );
        archive.idempotency_key = Some("IDEMP-lifecycle-archive".to_string());
        let archived = core.execute(archive, &runtime);
        assert!(archived.ok, "{archived:?}");
        assert_eq!(archived.result.unwrap()["lifecycle_state"], "archived");
        assert!(
            core.execute(
                request("REQ-life-list", "project.list", json!({})),
                &runtime
            )
            .result
            .unwrap()["projects"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let all = core.execute(
            request(
                "REQ-life-all",
                "project.list",
                json!({ "include_inactive": true }),
            ),
            &runtime,
        );
        assert_eq!(
            all.result.unwrap()["projects"][0]["lifecycle_state"],
            "archived"
        );
        assert!(
            core.execute(
                request(
                    "REQ-life-read",
                    "result.get",
                    json!({ "result_id": result_id })
                ),
                &runtime
            )
            .ok
        );

        let mut restore = request(
            "REQ-life-restore",
            "project.restore",
            json!({ "project_id": "PRJ-life" }),
        );
        restore.idempotency_key = Some("IDEMP-lifecycle-restore".to_string());
        assert_eq!(
            core.execute(restore, &runtime).result.unwrap()["lifecycle_state"],
            "active"
        );

        let mismatch = core.execute(
            request(
                "REQ-life-mismatch",
                "project.remove",
                json!({ "project_id": "PRJ-life", "confirm_project_id": "PRJ-other" }),
            ),
            &runtime,
        );
        assert_eq!(mismatch.error.unwrap().code, "APPROVAL_REQUIRED");
        let mut remove = request(
            "REQ-life-remove",
            "project.remove",
            json!({ "project_id": "PRJ-life", "confirm_project_id": "PRJ-life" }),
        );
        remove.idempotency_key = Some("IDEMP-lifecycle-remove".to_string());
        assert_eq!(
            core.execute(remove, &runtime).error.unwrap().code,
            "APPROVAL_REQUIRED"
        );
        let mut plan = request(
            "REQ-life-plan",
            "project.removal.plan",
            json!({"project_id":"PRJ-life"}),
        );
        plan.idempotency_key = Some("IDEMP-life-plan".to_string());
        let planned = core.execute(plan, &runtime);
        assert!(planned.ok, "{planned:?}");
        assert!(
            !serde_json::to_string(planned.result.as_ref().unwrap())
                .unwrap()
                .contains("file:///fixture")
        );
        let approval_id = planned.result.unwrap()["approval_id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut nonlocal = ExecutionAuthority::local_user("CLIENT-agent");
        nonlocal.permissions.remove("approve_destructive");
        let denied_decision = core.execute_authorized(
            request(
                "REQ-life-agent-decide",
                "project.removal.decide",
                json!({"project_id":"PRJ-life","approval_id":approval_id,"decision":"approve"}),
            ),
            &runtime,
            &nonlocal,
        );
        assert_eq!(denied_decision.error.unwrap().code, "PERMISSION_DENIED");
        let mut decide = request(
            "REQ-life-decide",
            "project.removal.decide",
            json!({"project_id":"PRJ-life","approval_id":approval_id,"decision":"approve"}),
        );
        decide.idempotency_key = Some("IDEMP-life-decide".to_string());
        assert_eq!(
            core.execute(decide, &runtime).result.unwrap()["state"],
            "approved_pending_execution"
        );
        let mut execute = request(
            "REQ-life-execute",
            "project.remove",
            json!({"project_id":"PRJ-life","approval_id":approval_id}),
        );
        execute.command_version = Some(2);
        execute.idempotency_key = Some("IDEMP-life-execute".to_string());
        assert_eq!(
            core.execute(execute, &runtime).result.unwrap()["state"],
            "executed"
        );
        assert!(
            core.execute(
                request(
                    "REQ-life-history",
                    "result.get",
                    json!({ "result_id": result_id })
                ),
                &runtime
            )
            .ok
        );
        let mut reuse = request(
            "REQ-life-reuse",
            "project.register",
            json!({ "id": "PRJ-life", "name": "Different", "root_uri": "file:///different" }),
        );
        reuse.idempotency_key = Some("IDEMP-lifecycle-reuse".to_string());
        assert_eq!(
            core.execute(reuse, &runtime).error.unwrap().code,
            "PROJECT_INACTIVE"
        );
        assert_eq!(
            core.execute(
                request(
                    "REQ-life-restore-removed",
                    "project.restore",
                    json!({ "project_id": "PRJ-life" })
                ),
                &runtime
            )
            .error
            .unwrap()
            .code,
            "PROJECT_REMOVED"
        );
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn incompatible_command_version_is_explicit() {
        let dir = temp_dir("version");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let mut req = request("REQ-version", "system.status", json!({}));
        req.command_version = Some(999);
        let response = core.execute(req, &runtime());
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "COMMAND_VERSION_INCOMPATIBLE");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn command_diagnostics_do_not_persist_payload_values() {
        let dir = temp_dir("diagnostic-privacy");
        let core = RelayCore::open(CoreConfig::new(&dir));
        let secret = "SHOULD-NOT-BE-IN-DIAGNOSTICS";
        let response = core.execute(
            request("REQ-echo", "system.echo", json!({ "value": secret })),
            &runtime(),
        );
        assert!(response.ok);
        core.flush_diagnostics();
        drop(core);

        let diagnostics_dir = dir.join("diagnostics");
        let mut raw = String::new();
        for entry in fs::read_dir(&diagnostics_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().ends_with(".jsonl") {
                raw.push_str(&fs::read_to_string(entry.path()).unwrap());
            }
        }
        assert!(raw.contains("relay.command.completed"));
        assert!(!raw.contains(secret));
        fs::remove_dir_all(dir).unwrap();
    }
}
