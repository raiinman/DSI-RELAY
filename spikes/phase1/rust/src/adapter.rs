use crate::registry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, QueryInformationJobObject,
    SetInformationJobObject, JobObjectExtendedLimitInformation,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};

pub const ADAPTER_MANIFEST_FORMAT: u32 = 1;
pub const ADAPTER_PROTOCOL_MIN: u32 = 1;
pub const ADAPTER_PROTOCOL_MAX: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterManifest {
    pub manifest_format: u32,
    pub adapter: AdapterIdentity,
    pub publisher: PublisherMetadata,
    pub artifact: ArtifactMetadata,
    pub relay: RelayCompatibility,
    pub permissions: RequestedPermissions,
    pub target: TargetRequirement,
    pub components: Vec<ComponentMetadata>,
    pub dependencies: Vec<DependencyMetadata>,
    pub update: UpdateMetadata,
    pub trust: TrustMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterIdentity {
    pub id: String,
    pub version: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherMetadata {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub sha256: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayCompatibility {
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub command_bindings: Vec<CommandBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBinding {
    pub command: String,
    pub command_version: u32,
    pub capability: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestedPermissions {
    #[serde(default)]
    pub project_read: Vec<String>,
    #[serde(default)]
    pub project_write: Vec<String>,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub credentials: Vec<String>,
    #[serde(default)]
    pub subprocess: bool,
    #[serde(default)]
    pub external_apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRequirement {
    pub tool: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMetadata {
    pub id: String,
    pub kind: String,
    pub version: String,
    pub sha256: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyMetadata {
    pub name: String,
    pub version: String,
    pub license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadata {
    pub channel: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustMetadata {
    pub build_provenance: String,
    pub review_status: String,
}

#[derive(Debug, Clone)]
pub struct BrokerPolicy {
    pub allowed_project_read: BTreeSet<String>,
    pub allowed_project_write: BTreeSet<String>,
    pub allow_network: bool,
    pub allowed_credentials: BTreeSet<String>,
    pub allow_subprocess: bool,
    pub allowed_external_apps: BTreeSet<String>,
    pub target_tool: String,
    pub target_version: String,
    pub max_process_memory_bytes: usize,
    pub request_timeout_ms: u64,
    pub max_message_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_failures_before_quarantine: u32,
    pub backoff_ms: u64,
}

impl BrokerPolicy {
    pub fn synthetic_default() -> Self {
        Self {
            allowed_project_read: BTreeSet::new(),
            allowed_project_write: BTreeSet::new(),
            allow_network: false,
            allowed_credentials: BTreeSet::new(),
            allow_subprocess: false,
            allowed_external_apps: BTreeSet::new(),
            target_tool: "synthetic".to_string(),
            target_version: "1.0.0".to_string(),
            max_process_memory_bytes: 32 * 1024 * 1024,
            request_timeout_ms: 400,
            max_message_bytes: 64 * 1024,
            max_stderr_bytes: 16 * 1024,
            max_failures_before_quarantine: 2,
            backoff_ms: 75,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdapterError {
    pub code: &'static str,
    pub message: String,
    pub untrusted_stderr: Option<String>,
}

impl AdapterError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            untrusted_stderr: None,
        }
    }

    fn with_stderr(mut self, stderr: Option<String>) -> Self {
        self.untrusted_stderr = stderr;
        self
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AdapterError {}

pub fn sha256_file(path: &Path) -> Result<String, AdapterError> {
    let bytes = fs::read(path).map_err(|error| {
        AdapterError::new("ADAPTER_ARTIFACT_UNREADABLE", format!("read worker artifact: {error}"))
    })?;
    let digest = Sha256::digest(&bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn protocol_intersects(min: u32, max: u32) -> bool {
    min <= max && max >= ADAPTER_PROTOCOL_MIN && min <= ADAPTER_PROTOCOL_MAX
}

pub fn validate_manifest(
    manifest: &AdapterManifest,
    policy: &BrokerPolicy,
    worker_path: &Path,
) -> Result<(), AdapterError> {
    if manifest.manifest_format != ADAPTER_MANIFEST_FORMAT {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INCOMPATIBLE",
            format!("unsupported manifest format {}", manifest.manifest_format),
        ));
    }
    if manifest.adapter.id.trim().is_empty()
        || manifest.adapter.version.trim().is_empty()
        || manifest.publisher.id.trim().is_empty()
        || manifest.publisher.source.trim().is_empty()
        || manifest.trust.build_provenance.trim().is_empty()
        || manifest.trust.review_status.trim().is_empty()
    {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter identity/publisher/provenance fields are required",
        ));
    }
    if !protocol_intersects(
        manifest.relay.protocol_min,
        manifest.relay.protocol_max,
    ) {
        return Err(AdapterError::new(
            "ADAPTER_PROTOCOL_INCOMPATIBLE",
            "adapter protocol range does not overlap RELAY broker protocol",
        ));
    }

    if manifest.target.tool != policy.target_tool
        || manifest.target.version != policy.target_version
    {
        return Err(AdapterError::new(
            "ADAPTER_TARGET_INCOMPATIBLE",
            format!(
                "adapter target {}@{} does not match available {}@{}",
                manifest.target.tool,
                manifest.target.version,
                policy.target_tool,
                policy.target_version
            ),
        ));
    }

    if manifest.permissions.network && !policy.allow_network {
        return Err(AdapterError::new(
            "ADAPTER_PERMISSION_DENIED",
            "manifest requests network access not granted by policy",
        ));
    }
    if manifest.permissions.subprocess && !policy.allow_subprocess {
        return Err(AdapterError::new(
            "ADAPTER_PERMISSION_DENIED",
            "manifest requests subprocess access not granted by policy",
        ));
    }
    for scope in &manifest.permissions.project_read {
        if !policy.allowed_project_read.contains(scope) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("manifest requests ungranted project-read scope {scope}"),
            ));
        }
    }
    for scope in &manifest.permissions.project_write {
        if !policy.allowed_project_write.contains(scope) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("manifest requests ungranted project-write scope {scope}"),
            ));
        }
    }
    for credential in &manifest.permissions.credentials {
        if !policy.allowed_credentials.contains(credential) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("manifest requests ungranted credential {credential}"),
            ));
        }
    }
    for app in &manifest.permissions.external_apps {
        if !policy.allowed_external_apps.contains(app) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("manifest requests ungranted external app {app}"),
            ));
        }
    }

    if manifest.relay.command_bindings.is_empty() {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter must declare at least one command binding",
        ));
    }
    let mut capabilities = BTreeSet::new();
    for binding in &manifest.relay.command_bindings {
        if binding.capability.trim().is_empty()
            || !capabilities.insert(binding.capability.clone())
        {
            return Err(AdapterError::new(
                "ADAPTER_MANIFEST_INVALID",
                "adapter capability IDs must be non-empty and unique",
            ));
        }
        match registry::resolve_command(&binding.command, Some(binding.command_version)) {
            Ok(_) if registry::is_surface_exposed(&binding.command, "adapter") => {}
            Ok(_) => {
                return Err(AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!(
                        "command {}@{} is not exposed to adapters",
                        binding.command, binding.command_version
                    ),
                ));
            }
            Err(_) => {
                return Err(AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!(
                        "unknown/incompatible command {}@{}",
                        binding.command, binding.command_version
                    ),
                ));
            }
        }
    }

    if !is_hex_digest(&manifest.artifact.sha256) {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "artifact sha256 must be 64 hexadecimal characters",
        ));
    }
    let actual = sha256_file(worker_path)?;
    if !manifest.artifact.sha256.eq_ignore_ascii_case(&actual) {
        return Err(AdapterError::new(
            "ADAPTER_INTEGRITY_MISMATCH",
            "worker executable digest does not match manifest",
        ));
    }

    if manifest.components.is_empty() {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter must inventory its executable worker component",
        ));
    }
    let mut worker_component_matches = false;
    for component in &manifest.components {
        if component.id.trim().is_empty()
            || component.version.trim().is_empty()
            || component.source.trim().is_empty()
            || !is_hex_digest(&component.sha256)
        {
            return Err(AdapterError::new(
                "ADAPTER_MANIFEST_INVALID",
                "component identity/version/source/digest is invalid",
            ));
        }
        if component.kind == "worker" && component.sha256.eq_ignore_ascii_case(&actual) {
            worker_component_matches = true;
        }
    }
    if !worker_component_matches {
        return Err(AdapterError::new(
            "ADAPTER_INTEGRITY_MISMATCH",
            "manifest worker component digest does not match the launched artifact",
        ));
    }

    if manifest.update.channel.trim().is_empty() || manifest.update.source.trim().is_empty() {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "update channel/source are required",
        ));
    }
    Ok(())
}


#[derive(Debug, Clone, Serialize)]
pub struct JobLimitEvidence {
    pub active_process_limit: u32,
    pub process_memory_limit_bytes: usize,
    pub kill_on_close: bool,
}

struct JobGuard {
    handle: HANDLE,
    evidence: JobLimitEvidence,
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

impl JobGuard {
    fn create_for(child: &Child, max_process_memory_bytes: usize) -> Result<Self, AdapterError> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("CreateJobObjectW failed: {}", GetLastError()),
                ));
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                    | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            limits.BasicLimitInformation.ActiveProcessLimit = 1;
            limits.ProcessMemoryLimit = max_process_memory_bytes;

            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0 {
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("SetInformationJobObject failed: {}", GetLastError()),
                ));
            }

            let process_handle = child.as_raw_handle() as HANDLE;
            if AssignProcessToJobObject(job, process_handle) == 0 {
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("AssignProcessToJobObject failed: {}", GetLastError()),
                ));
            }

            let mut actual = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            if QueryInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut actual as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                std::ptr::null_mut(),
            ) == 0 {
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("QueryInformationJobObject failed: {}", GetLastError()),
                ));
            }

            let flags = actual.BasicLimitInformation.LimitFlags;
            Ok(Self {
                handle: job,
                evidence: JobLimitEvidence {
                    active_process_limit: actual.BasicLimitInformation.ActiveProcessLimit,
                    process_memory_limit_bytes: actual.ProcessMemoryLimit,
                    kill_on_close: flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE != 0,
                },
            })
        }
    }

    fn evidence(&self) -> JobLimitEvidence {
        self.evidence.clone()
    }
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
    pub limits: JobLimitEvidence,
    pub stderr_untrusted: Option<String>,
}

pub struct AdapterBroker {
    policy: BrokerPolicy,
    states: Arc<Mutex<BTreeMap<String, RuntimeState>>>,
    manifests: Arc<Mutex<BTreeMap<String, AdapterManifest>>>,
}

impl AdapterBroker {
    pub fn new(policy: BrokerPolicy) -> Self {
        Self {
            policy,
            states: Arc::new(Mutex::new(BTreeMap::new())),
            manifests: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn policy(&self) -> &BrokerPolicy {
        &self.policy
    }

    pub fn install(
        &self,
        manifest: AdapterManifest,
        worker_path: &Path,
    ) -> Result<(), AdapterError> {
        validate_manifest(&manifest, &self.policy, worker_path)?;
        let mut manifests = self.manifests.lock().map_err(|_| {
            AdapterError::new("ADAPTER_BROKER_STATE_ERROR", "manifest registry lock failed")
        })?;
        manifests.insert(manifest.adapter.id.clone(), manifest);
        Ok(())
    }

    pub fn installed_count(&self) -> usize {
        self.manifests
            .lock()
            .map(|manifests| manifests.len())
            .unwrap_or(0)
    }

    pub fn installed_manifest(&self, adapter_id: &str) -> Option<AdapterManifest> {
        self.manifests
            .lock()
            .ok()
            .and_then(|manifests| manifests.get(adapter_id).cloned())
    }

    pub fn invoke_installed(
        &self,
        adapter_id: &str,
        worker_path: &Path,
        worker_mode: &str,
        command: &str,
        command_version: u32,
        arguments: Value,
    ) -> Result<InvocationOutcome, AdapterError> {
        let manifest = self.installed_manifest(adapter_id).ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_NOT_INSTALLED",
                format!("adapter {adapter_id} is not installed"),
            )
        })?;
        self.invoke(
            &manifest,
            worker_path,
            worker_mode,
            command,
            command_version,
            arguments,
        )
    }

    pub fn is_quarantined(&self, adapter_id: &str) -> bool {
        self.states
            .lock()
            .ok()
            .and_then(|states| states.get(adapter_id).cloned())
            .map(|state| state.quarantined)
            .unwrap_or(false)
    }

    pub fn clear_quarantine(&self, adapter_id: &str) {
        if let Ok(mut states) = self.states.lock() {
            states.remove(adapter_id);
        }
    }

    fn preflight_runtime(&self, adapter_id: &str) -> Result<(), AdapterError> {
        let states = self.states.lock().map_err(|_| {
            AdapterError::new("ADAPTER_BROKER_STATE_ERROR", "broker state lock failed")
        })?;
        if let Some(state) = states.get(adapter_id) {
            if state.quarantined {
                return Err(AdapterError::new(
                    "ADAPTER_QUARANTINED",
                    "adapter is quarantined after repeated failures",
                ));
            }
            if let Some(until) = state.backoff_until {
                if Instant::now() < until {
                    return Err(AdapterError::new(
                        "ADAPTER_BACKOFF",
                        "adapter restart backoff is active",
                    ));
                }
            }
        }
        Ok(())
    }

    fn record_failure(&self, adapter_id: &str) {
        if let Ok(mut states) = self.states.lock() {
            let state = states.entry(adapter_id.to_string()).or_default();
            state.failures = state.failures.saturating_add(1);
            if state.failures >= self.policy.max_failures_before_quarantine {
                state.quarantined = true;
                state.backoff_until = None;
            } else {
                state.backoff_until =
                    Some(Instant::now() + Duration::from_millis(self.policy.backoff_ms));
            }
        }
    }

    fn record_success(&self, adapter_id: &str) {
        if let Ok(mut states) = self.states.lock() {
            states.remove(adapter_id);
        }
    }
}

fn spawn_line_reader(
    stdout: impl Read + Send + 'static,
    max_bytes: usize,
) -> std::sync::mpsc::Receiver<Result<String, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut bytes = Vec::new();
            match reader.read_until(b'\n', &mut bytes) {
                Ok(0) => break,
                Ok(_) => {
                    if bytes.len() > max_bytes {
                        let _ = tx.send(Err("adapter message exceeded limit".to_string()));
                        break;
                    }
                    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
                        bytes.pop();
                    }
                    match String::from_utf8(bytes) {
                        Ok(line) => {
                            if tx.send(Ok(line)).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = tx.send(Err(format!("adapter stdout was not UTF-8: {error}")));
                            break;
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(format!("read adapter stdout: {error}")));
                    break;
                }
            }
        }
    });
    rx
}

fn spawn_stderr_reader(
    stderr: impl Read + Send + 'static,
    max_bytes: usize,
) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut limited = stderr.take((max_bytes + 1) as u64);
        let _ = limited.read_to_end(&mut bytes);
        if bytes.len() > max_bytes {
            bytes.truncate(max_bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}


fn recv_line(
    rx: &std::sync::mpsc::Receiver<Result<String, String>>,
    timeout: Duration,
) -> Result<String, AdapterError> {
    match rx.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(message)) => Err(AdapterError::new("ADAPTER_INVALID_MESSAGE", message)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(AdapterError::new(
            "ADAPTER_TIMEOUT",
            "adapter did not respond before the broker timeout",
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(AdapterError::new(
            "ADAPTER_WORKER_EXITED",
            "adapter worker exited before producing the expected message",
        )),
    }
}

fn parse_message(line: &str) -> Result<Value, AdapterError> {
    serde_json::from_str(line).map_err(|error| {
        AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            format!("adapter emitted invalid JSON: {error}"),
        )
    })
}

fn minimal_worker_environment(command: &mut Command) {
    let keep = ["SystemRoot", "WINDIR", "TEMP", "TMP"];
    let values: Vec<(String, String)> = keep
        .iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| ((*key).to_string(), value)))
        .collect();
    command.env_clear();
    for (key, value) in values {
        command.env(key, value);
    }
}


impl AdapterBroker {
    pub fn invoke(
        &self,
        manifest: &AdapterManifest,
        worker_path: &Path,
        worker_mode: &str,
        command: &str,
        command_version: u32,
        arguments: Value,
    ) -> Result<InvocationOutcome, AdapterError> {
        validate_manifest(manifest, &self.policy, worker_path)?;
        self.preflight_runtime(&manifest.adapter.id)?;

        let binding = manifest
            .relay
            .command_bindings
            .iter()
            .find(|binding| {
                binding.command == command && binding.command_version == command_version
            })
            .ok_or_else(|| {
                AdapterError::new(
                    "ADAPTER_COMMAND_NOT_BOUND",
                    format!("{command}@{command_version} is not bound by this adapter"),
                )
            })?;

        let spec = registry::resolve_command(command, Some(command_version))
            .map_err(|_| {
                AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!("{command}@{command_version} is not in the RELAY registry"),
                )
            })?;

        registry::validate_value(&spec.arguments_schema, &arguments).map_err(|error| {
            AdapterError::new(
                "VALIDATION_FAILED",
                format!("{} {}", error.path, error.message),
            )
        })?;

        let result = self.invoke_worker(
            manifest,
            worker_path,
            worker_mode,
            binding,
            spec,
            arguments,
        );

        match &result {
            Ok(_) => self.record_success(&manifest.adapter.id),
            Err(_) => self.record_failure(&manifest.adapter.id),
        }
        result
    }

    fn invoke_worker(
        &self,
        manifest: &AdapterManifest,
        worker_path: &Path,
        worker_mode: &str,
        binding: &CommandBinding,
        spec: &registry::CommandSpec,
        arguments: Value,
    ) -> Result<InvocationOutcome, AdapterError> {
        let mut command = Command::new(worker_path);
        minimal_worker_environment(&mut command);
        command
            .env("RELAY_ADAPTER_ID", &manifest.adapter.id)
            .env("RELAY_ADAPTER_VERSION", &manifest.adapter.version)
            .env("RELAY_ADAPTER_CAPABILITY", &binding.capability)
            .env("RELAY_ADAPTER_MODE", worker_mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            AdapterError::new(
                "ADAPTER_LAUNCH_FAILED",
                format!("launch adapter worker: {error}"),
            )
        })?;
        let worker_pid = child.id();
        let job = JobGuard::create_for(
            &child,
            self.policy.max_process_memory_bytes,
        )?;
        let limit_evidence = job.evidence();

        let stdout = child.stdout.take().ok_or_else(|| {
            AdapterError::new("ADAPTER_LAUNCH_FAILED", "worker stdout was unavailable")
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AdapterError::new("ADAPTER_LAUNCH_FAILED", "worker stderr was unavailable")
        })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            AdapterError::new("ADAPTER_LAUNCH_FAILED", "worker stdin was unavailable")
        })?;

        let lines = spawn_line_reader(stdout, self.policy.max_message_bytes);
        let stderr_handle =
            spawn_stderr_reader(stderr, self.policy.max_stderr_bytes);
        let timeout = Duration::from_millis(self.policy.request_timeout_ms);

        let hello_line = match recv_line(&lines, timeout) {
            Ok(line) => line,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Err(error.with_stderr(nonempty(stderr)));
            }
        };
        let hello = match parse_message(&hello_line) {
            Ok(value) => value,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Err(error.with_stderr(nonempty(stderr)));
            }
        };

        validate_worker_hello(
            &hello,
            manifest,
            binding,
            worker_pid,
        )?;

        let request_id = format!(
            "adapter-{}-{}",
            worker_pid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let request = json!({
            "type": "adapter_invoke",
            "request_id": request_id,
            "command": binding.command,
            "command_version": binding.command_version,
            "capability": binding.capability,
            "arguments": arguments
        });
        let encoded = serde_json::to_string(&request).map_err(|error| {
            AdapterError::new(
                "ADAPTER_BROKER_SERIALIZE_ERROR",
                format!("serialize adapter request: {error}"),
            )
        })?;
        stdin.write_all(encoded.as_bytes()).map_err(|error| {
            AdapterError::new(
                "ADAPTER_WORKER_IO",
                format!("write adapter request: {error}"),
            )
        })?;
        stdin.write_all(b"\n").map_err(|error| {
            AdapterError::new(
                "ADAPTER_WORKER_IO",
                format!("finish adapter request: {error}"),
            )
        })?;
        stdin.flush().map_err(|error| {
            AdapterError::new(
                "ADAPTER_WORKER_IO",
                format!("flush adapter request: {error}"),
            )
        })?;

        let response_line = match recv_line(&lines, timeout) {
            Ok(line) => line,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Err(error.with_stderr(nonempty(stderr)));
            }
        };
        let worker_response = match parse_message(&response_line) {
            Ok(value) => value,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Err(error.with_stderr(nonempty(stderr)));
            }
        };

        let response = validate_worker_response(
            &worker_response,
            &request_id,
            spec,
        )?;

        let _ = child.kill();
        let _ = child.wait();
        let stderr = stderr_handle.join().unwrap_or_default();

        Ok(InvocationOutcome {
            response,
            provenance: InvocationProvenance {
                adapter_id: manifest.adapter.id.clone(),
                adapter_version: manifest.adapter.version.clone(),
                publisher_id: manifest.publisher.id.clone(),
                artifact_sha256: manifest.artifact.sha256.clone(),
                review_status: manifest.trust.review_status.clone(),
                worker_pid,
            },
            limits: limit_evidence,
            stderr_untrusted: nonempty(stderr),
        })
    }
}

fn nonempty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}


fn validate_worker_hello(
    hello: &Value,
    manifest: &AdapterManifest,
    binding: &CommandBinding,
    worker_pid: u32,
) -> Result<(), AdapterError> {
    if hello.get("type").and_then(Value::as_str) != Some("adapter_hello") {
        return Err(AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "worker did not begin with adapter_hello",
        ));
    }
    if hello.get("adapter_id").and_then(Value::as_str)
        != Some(manifest.adapter.id.as_str())
        || hello.get("adapter_version").and_then(Value::as_str)
            != Some(manifest.adapter.version.as_str())
    {
        return Err(AdapterError::new(
            "ADAPTER_IDENTITY_MISMATCH",
            "worker identity/version does not match manifest",
        ));
    }
    let protocol = hello
        .get("protocol")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    if protocol < manifest.relay.protocol_min
        || protocol > manifest.relay.protocol_max
        || protocol < ADAPTER_PROTOCOL_MIN
        || protocol > ADAPTER_PROTOCOL_MAX
    {
        return Err(AdapterError::new(
            "ADAPTER_PROTOCOL_INCOMPATIBLE",
            format!("worker protocol {protocol} is outside negotiated range"),
        ));
    }
    if hello.get("pid").and_then(Value::as_u64) != Some(worker_pid as u64) {
        return Err(AdapterError::new(
            "ADAPTER_IDENTITY_MISMATCH",
            "worker-reported PID does not match launched process",
        ));
    }
    let capabilities: BTreeSet<String> = hello
        .get("capabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let declared: BTreeSet<String> = manifest
        .relay
        .command_bindings
        .iter()
        .map(|item| item.capability.clone())
        .collect();
    if capabilities != declared || !capabilities.contains(&binding.capability) {
        return Err(AdapterError::new(
            "ADAPTER_CAPABILITY_MISMATCH",
            "worker capabilities do not exactly match the manifest",
        ));
    }
    Ok(())
}


fn validate_worker_response(
    message: &Value,
    request_id: &str,
    spec: &registry::CommandSpec,
) -> Result<Value, AdapterError> {
    if message.get("type").and_then(Value::as_str) != Some("adapter_result")
        || message.get("request_id").and_then(Value::as_str) != Some(request_id)
    {
        return Err(AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "worker result type/request_id did not match the invocation",
        ));
    }

    let ok = message
        .get("ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_INVALID_MESSAGE",
                "worker result is missing boolean ok",
            )
        })?;

    if ok {
        let result = message.get("result").cloned().ok_or_else(|| {
            AdapterError::new(
                "ADAPTER_INVALID_MESSAGE",
                "successful worker result is missing result",
            )
        })?;
        registry::validate_value(&spec.result_schema, &result).map_err(|error| {
            AdapterError::new(
                "ADAPTER_RESULT_SCHEMA_VIOLATION",
                format!("{} {}", error.path, error.message),
            )
        })?;
        return Ok(json!({
            "type": "command_result",
            "request_id": request_id,
            "command_version": spec.version,
            "ok": true,
            "result": result
        }));
    }

    let error_value = message.get("error").cloned().ok_or_else(|| {
        AdapterError::new(
            "ADAPTER_INVALID_MESSAGE",
            "failed worker result is missing error",
        )
    })?;
    registry::validate_value(&registry::registry().error_schema, &error_value).map_err(
        |error| {
            AdapterError::new(
                "ADAPTER_ERROR_SCHEMA_VIOLATION",
                format!("{} {}", error.path, error.message),
            )
        },
    )?;
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
        "type": "command_result",
        "request_id": request_id,
        "command_version": spec.version,
        "ok": false,
        "error": error_value
    }))
}
