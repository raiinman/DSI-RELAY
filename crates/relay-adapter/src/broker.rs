use crate::backend::select_release_backend;
use crate::manifest::{
    validate_manifest, AdapterManifest, BrokerPolicy, CommandBinding,
};
use crate::sandbox::{run_sandboxed_worker, StableSandboxPolicy};
use crate::{AdapterError, JobLimitEvidence};
use relay_contracts::registry::{self, CommandSpec};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static MAILBOX_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct InstalledAdapter {
    manifest: AdapterManifest,
    worker_path: PathBuf,
    package_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Default, Clone)]
struct RuntimeState {
    failures: u32,
    backoff_until: Option<Instant>,
    quarantined: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvocationProvenance {
    pub adapter_id: String,
    pub adapter_version: String,
    pub publisher_id: String,
    pub artifact_sha256: String,
    pub review_status: String,
    pub worker_pid: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvocationOutcome {
    pub response: Value,
    pub provenance: InvocationProvenance,
    pub job_limits: JobLimitEvidence,
    pub sandbox_backend: String,
    pub sandbox_total_ms: f64,
}

pub struct AdapterBroker {
    policy: BrokerPolicy,
    mailbox_root: PathBuf,
    installed: Mutex<BTreeMap<String, InstalledAdapter>>,
    package_locks: Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>,
    runtime: Mutex<BTreeMap<String, RuntimeState>>,
}


impl AdapterBroker {
    pub fn new(
        policy: BrokerPolicy,
        mailbox_root: impl Into<PathBuf>,
    ) -> Result<Self, AdapterError> {
        let mailbox_root = mailbox_root.into();
        fs::create_dir_all(&mailbox_root).map_err(|error| {
            AdapterError::new(
                "ADAPTER_BROKER_INIT_FAILED",
                format!("create broker mailbox root: {error}"),
            )
        })?;
        Ok(Self {
            policy,
            mailbox_root,
            installed: Mutex::new(BTreeMap::new()),
            package_locks: Mutex::new(BTreeMap::new()),
            runtime: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn policy(&self) -> &BrokerPolicy {
        &self.policy
    }

    pub fn install(
        &self,
        manifest: AdapterManifest,
        worker_path: impl Into<PathBuf>,
    ) -> Result<(), AdapterError> {
        let worker_path = worker_path.into();
        validate_manifest(&manifest, &self.policy, &worker_path)?;
        let package_root = worker_path.parent().ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_WORKER_PATH_INVALID",
                "adapter worker must have a package directory",
            )
        })?;
        let lock_key = fs::canonicalize(package_root)
            .unwrap_or_else(|_| package_root.to_path_buf());
        let package_lock = {
            let mut locks = self.package_locks.lock().map_err(|_| {
                AdapterError::new(
                    "ADAPTER_BROKER_STATE_ERROR",
                    "adapter package-lock registry failed",
                )
            })?;
            locks
                .entry(lock_key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let id = manifest.adapter.id.clone();
        let mut installed = self.installed.lock().map_err(|_| {
            AdapterError::new(
                "ADAPTER_BROKER_STATE_ERROR",
                "installed-adapter registry lock failed",
            )
        })?;
        installed.insert(
            id,
            InstalledAdapter {
                manifest,
                worker_path,
                package_lock,
            },
        );
        Ok(())
    }

    pub fn uninstall(&self, adapter_id: &str) -> Result<(), AdapterError> {
        let mut installed = self.installed.lock().map_err(|_| {
            AdapterError::new(
                "ADAPTER_BROKER_STATE_ERROR",
                "installed-adapter registry lock failed",
            )
        })?;
        if installed.remove(adapter_id).is_none() {
            return Err(AdapterError::new(
                "ADAPTER_NOT_INSTALLED",
                format!("adapter {adapter_id} is not installed"),
            ));
        }
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.remove(adapter_id);
        }
        Ok(())
    }

    pub fn installed_count(&self) -> usize {
        self.installed
            .lock()
            .map(|value| value.len())
            .unwrap_or(0)
    }

    pub fn is_quarantined(&self, adapter_id: &str) -> bool {
        self.runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.get(adapter_id).cloned())
            .map(|state| state.quarantined)
            .unwrap_or(false)
    }

    pub fn clear_quarantine(&self, adapter_id: &str) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.remove(adapter_id);
        }
    }

    pub fn invoke(
        &self,
        adapter_id: &str,
        command: &str,
        command_version: u32,
        arguments: Value,
    ) -> Result<InvocationOutcome, AdapterError> {
        self.preflight_runtime(adapter_id)?;

        let installed = {
            let installed = self.installed.lock().map_err(|_| {
                AdapterError::new(
                    "ADAPTER_BROKER_STATE_ERROR",
                    "installed-adapter registry lock failed",
                )
            })?;
            installed.get(adapter_id).cloned().ok_or_else(|| {
                AdapterError::new(
                    "ADAPTER_NOT_INSTALLED",
                    format!("adapter {adapter_id} is not installed"),
                )
            })?
        };

        let _package_guard = installed.package_lock.lock().map_err(|_| {
            AdapterError::new(
                "ADAPTER_BROKER_STATE_ERROR",
                "adapter package invocation lock failed",
            )
        })?;

        validate_manifest(
            &installed.manifest,
            &self.policy,
            &installed.worker_path,
        )?;

        let binding = find_binding(
            &installed.manifest,
            command,
            command_version,
        )?;
        let spec = registry::resolve_command(command, Some(command_version))
            .map_err(|_| {
                AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!(
                        "{command}@{command_version} is not in the RELAY registry"
                    ),
                )
            })?;
        registry::validate_value(&spec.arguments_schema, &arguments)
            .map_err(|error| {
                AdapterError::new(
                    "VALIDATION_FAILED",
                    format!("{} {}", error.path, error.message),
                )
            })?;

        let result = self.invoke_worker(
            &installed,
            binding,
            spec,
            arguments,
        );

        match &result {
            Ok(_) => self.record_success(adapter_id),
            Err(_) => self.record_failure(adapter_id),
        }
        result
    }

    fn preflight_runtime(&self, adapter_id: &str) -> Result<(), AdapterError> {
        let runtime = self.runtime.lock().map_err(|_| {
            AdapterError::new(
                "ADAPTER_BROKER_STATE_ERROR",
                "adapter runtime-state lock failed",
            )
        })?;
        if let Some(state) = runtime.get(adapter_id) {
            if state.quarantined {
                return Err(AdapterError::new(
                    "ADAPTER_QUARANTINED",
                    "adapter is quarantined after repeated failures",
                ));
            }
            if state
                .backoff_until
                .map(|until| Instant::now() < until)
                .unwrap_or(false)
            {
                return Err(AdapterError::new(
                    "ADAPTER_BACKOFF",
                    "adapter restart backoff is active",
                ));
            }
        }
        Ok(())
    }

    fn record_failure(&self, adapter_id: &str) {
        if let Ok(mut runtime) = self.runtime.lock() {
            let state = runtime.entry(adapter_id.to_string()).or_default();
            state.failures = state.failures.saturating_add(1);
            if state.failures >= self.policy.max_failures_before_quarantine {
                state.quarantined = true;
                state.backoff_until = None;
            } else {
                state.backoff_until = Some(
                    Instant::now()
                        + Duration::from_millis(self.policy.backoff_ms),
                );
            }
        }
    }

    fn record_success(&self, adapter_id: &str) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.remove(adapter_id);
        }
    }

    fn invoke_worker(
        &self,
        installed: &InstalledAdapter,
        binding: &CommandBinding,
        spec: &CommandSpec,
        arguments: Value,
    ) -> Result<InvocationOutcome, AdapterError> {
        let backend = select_release_backend().ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_SANDBOX_UNAVAILABLE",
                "Windows sandbox backend could not be classified",
            )
        })?;
        if !backend.strong_untrusted_launch_enabled {
            return Err(AdapterError::new(
                "ADAPTER_SANDBOX_UNAVAILABLE",
                backend.reason,
            ));
        }

        let request_id = invocation_id();
        let mailbox = self.create_mailbox(&installed.manifest.adapter.id)?;
        let declared_capabilities: Vec<String> = installed
            .manifest
            .relay
            .command_bindings
            .iter()
            .map(|value| value.capability.clone())
            .collect();
        let request = json!({
            "type": "adapter_invoke",
            "request_id": request_id,
            "adapter_id": installed.manifest.adapter.id,
            "adapter_version": installed.manifest.adapter.version,
            "protocol": 1,
            "capability": binding.capability,
            "capabilities": declared_capabilities,
            "command": binding.command,
            "command_version": binding.command_version,
            "arguments": arguments
        });
        fs::write(
            mailbox.join("request.json"),
            serde_json::to_vec_pretty(&request).map_err(|error| {
                AdapterError::new(
                    "ADAPTER_REQUEST_SERIALIZE_FAILED",
                    format!("serialize adapter request: {error}"),
                )
            })?,
        )
        .map_err(|error| {
            AdapterError::new(
                "ADAPTER_MAILBOX_FAILED",
                format!("write adapter mailbox request: {error}"),
            )
        })?;

        let worker_parent = installed.worker_path.parent().ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_WORKER_PATH_INVALID",
                "adapter worker must have a parent directory",
            )
        })?;

        let sandbox_policy = StableSandboxPolicy {
            identity: sandbox_identity(
                &installed.manifest,
                &request_id,
            ),
            read_write_paths: vec![mailbox.clone()],
            read_only_paths: vec![worker_parent.to_path_buf()],
            capabilities: Vec::new(),
            disallow_win32k: true,
            max_process_memory_bytes: self.policy.max_process_memory_bytes,
            timeout_ms: self.policy.request_timeout_ms,
            environment: minimal_environment(),
        };

        let run = run_sandboxed_worker(
            &installed.worker_path,
            &mailbox,
            &sandbox_policy,
        );
        let evidence = match run {
            Ok(evidence) => evidence,
            Err(error) => {
                let _ = fs::remove_dir_all(&mailbox);
                return Err(error);
            }
        };

        let response = validate_worker_response(
            &evidence.result,
            &request_id,
            &installed.manifest,
            binding,
            spec,
            evidence.worker_pid,
        );
        let _ = fs::remove_dir_all(&mailbox);
        let response = response?;

        Ok(InvocationOutcome {
            response,
            provenance: InvocationProvenance {
                adapter_id: installed.manifest.adapter.id.clone(),
                adapter_version: installed.manifest.adapter.version.clone(),
                publisher_id: installed.manifest.publisher.id.clone(),
                artifact_sha256: installed.manifest.artifact.sha256.clone(),
                review_status: installed.manifest.trust.review_status.clone(),
                worker_pid: evidence.worker_pid,
            },
            job_limits: evidence.job_limits,
            sandbox_backend: evidence.backend,
            sandbox_total_ms: evidence.total_ms,
        })
    }

    fn create_mailbox(&self, adapter_id: &str) -> Result<PathBuf, AdapterError> {
        let counter = MAILBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mailbox = self.mailbox_root.join(format!(
            "{}-{}-{}",
            safe_segment(adapter_id),
            std::process::id(),
            counter
        ));
        fs::create_dir_all(&mailbox).map_err(|error| {
            AdapterError::new(
                "ADAPTER_MAILBOX_FAILED",
                format!("create adapter mailbox: {error}"),
            )
        })?;
        Ok(mailbox)
    }
}


fn find_binding<'a>(
    manifest: &'a AdapterManifest,
    command: &str,
    command_version: u32,
) -> Result<&'a CommandBinding, AdapterError> {
    manifest
        .relay
        .command_bindings
        .iter()
        .find(|binding| {
            binding.command == command
                && binding.command_version == command_version
        })
        .ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_COMMAND_NOT_BOUND",
                format!(
                    "{command}@{command_version} is not bound by this adapter"
                ),
            )
        })
}

fn invocation_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = MAILBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ADP-{}-{sequence}-{nanos}", std::process::id())
}

fn safe_segment(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(40)
        .collect();
    if value.is_empty() {
        "adapter".to_string()
    } else {
        value
    }
}

fn sandbox_identity(
    manifest: &AdapterManifest,
    request_id: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(manifest.adapter.id.as_bytes());
    hasher.update([0]);
    hasher.update(manifest.adapter.version.as_bytes());
    hasher.update([0]);
    hasher.update(manifest.artifact.sha256.as_bytes());
    hasher.update([0]);
    hasher.update(request_id.as_bytes());
    let digest = hasher.finalize();
    let mut text = String::with_capacity(24);
    for byte in digest.iter().take(12) {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}")
            .expect("writing to String cannot fail");
    }
    format!("DSIRelay.Adapter.{text}")
}

fn minimal_environment() -> Vec<(String, String)> {
    const KEYS: &[&str] = &[
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "ComSpec",
        "Path",
        "PATHEXT",
        "TEMP",
        "TMP",
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "PROCESSOR_ARCHITECTURE",
        "NUMBER_OF_PROCESSORS",
        "OS",
        "USERNAME",
        "USERDOMAIN",
    ];
    KEYS.iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_string(), value))
        })
        .collect()
}


fn validate_worker_response(
    message: &Value,
    request_id: &str,
    manifest: &AdapterManifest,
    binding: &CommandBinding,
    spec: &CommandSpec,
    worker_pid: u32,
) -> Result<Value, AdapterError> {
    if message.get("type").and_then(Value::as_str)
        != Some("adapter_result")
    {
        return Err(AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "worker response type must be adapter_result",
        ));
    }
    if message.get("request_id").and_then(Value::as_str)
        != Some(request_id)
    {
        return Err(AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "worker response request_id does not match invocation",
        ));
    }
    if message.get("adapter_id").and_then(Value::as_str)
        != Some(manifest.adapter.id.as_str())
        || message.get("adapter_version").and_then(Value::as_str)
            != Some(manifest.adapter.version.as_str())
    {
        return Err(AdapterError::new(
            "ADAPTER_IDENTITY_MISMATCH",
            "worker response identity/version does not match manifest",
        ));
    }
    if message.get("protocol").and_then(Value::as_u64)
        != Some(manifest.relay.protocol_max as u64)
        || manifest.relay.protocol_max != 1
    {
        return Err(AdapterError::new(
            "ADAPTER_PROTOCOL_INCOMPATIBLE",
            "worker response protocol does not match negotiated adapter protocol",
        ));
    }
    if message.get("pid").and_then(Value::as_u64)
        != Some(worker_pid as u64)
    {
        return Err(AdapterError::new(
            "ADAPTER_IDENTITY_MISMATCH",
            "worker-reported PID does not match launched process",
        ));
    }

    let actual_capabilities: BTreeSet<String> = message
        .get("capabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let declared_capabilities: BTreeSet<String> = manifest
        .relay
        .command_bindings
        .iter()
        .map(|value| value.capability.clone())
        .collect();
    if actual_capabilities != declared_capabilities
        || !actual_capabilities.contains(&binding.capability)
    {
        return Err(AdapterError::new(
            "ADAPTER_CAPABILITY_MISMATCH",
            "worker capabilities do not exactly match the manifest",
        ));
    }

    let ok = message
        .get("ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_INVALID_MESSAGE",
                "worker response is missing boolean ok",
            )
        })?;

    if ok {
        let result = message.get("result").cloned().ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_INVALID_MESSAGE",
                "successful worker response is missing result",
            )
        })?;
        registry::validate_value(&spec.result_schema, &result)
            .map_err(|error| {
                AdapterError::new(
                    "ADAPTER_RESULT_SCHEMA_VIOLATION",
                    format!("{} {}", error.path, error.message),
                )
            })?;
        return Ok(json!({
            "ok": true,
            "command": binding.command,
            "command_version": binding.command_version,
            "result": result
        }));
    }

    let error_value = message.get("error").cloned().ok_or_else(|| {
        AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "failed worker response is missing error",
        )
    })?;
    registry::validate_value(
        &registry::registry().error_schema,
        &error_value,
    )
    .map_err(|error| {
        AdapterError::new(
            "ADAPTER_ERROR_SCHEMA_VIOLATION",
            format!("{} {}", error.path, error.message),
        )
    })?;
    let code = error_value
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !registry::is_error_allowed(spec, code) {
        return Err(AdapterError::new(
            "ADAPTER_UNDECLARED_ERROR",
            format!("worker returned undeclared command error {code}"),
        ));
    }

    Ok(json!({
        "ok": false,
        "command": binding.command,
        "command_version": binding.command_version,
        "error": error_value
    }))
}
