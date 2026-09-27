use crate::AdapterError;
use relay_contracts::registry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const ADAPTER_MANIFEST_FORMAT: u32 = 1;
pub const ADAPTER_PROTOCOL_MIN: u32 = 1;
pub const ADAPTER_PROTOCOL_MAX: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterIdentity {
    pub id: String,
    pub version: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublisherMetadata {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactMetadata {
    pub sha256: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayCompatibility {
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub command_bindings: Vec<CommandBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandBinding {
    pub command: String,
    pub command_version: u32,
    pub capability: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetRequirement {
    pub tool: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentMetadata {
    pub id: String,
    pub kind: String,
    pub version: String,
    pub sha256: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyMetadata {
    pub name: String,
    pub version: String,
    pub license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateMetadata {
    pub channel: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
            max_failures_before_quarantine: 2,
            backoff_ms: 75,
        }
    }
}

pub fn sha256_file(path: &Path) -> Result<String, AdapterError> {
    let bytes = fs::read(path).map_err(|error| {
        AdapterError::new(
            "ADAPTER_ARTIFACT_UNREADABLE",
            format!("read worker artifact: {error}"),
        )
    })?;
    let digest = Sha256::digest(&bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}")
            .expect("writing to String cannot fail");
    }
    Ok(output)
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn protocol_intersects(min: u32, max: u32) -> bool {
    min <= max
        && max >= ADAPTER_PROTOCOL_MIN
        && min <= ADAPTER_PROTOCOL_MAX
}

pub fn validate_manifest(
    manifest: &AdapterManifest,
    policy: &BrokerPolicy,
    worker_path: &Path,
) -> Result<(), AdapterError> {
    if manifest.manifest_format != ADAPTER_MANIFEST_FORMAT {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INCOMPATIBLE",
            format!(
                "unsupported manifest format {}",
                manifest.manifest_format
            ),
        ));
    }

    if manifest.adapter.id.trim().is_empty()
        || manifest.adapter.version.trim().is_empty()
        || manifest.adapter.display_name.trim().is_empty()
        || manifest.publisher.id.trim().is_empty()
        || manifest.publisher.source.trim().is_empty()
        || manifest.trust.build_provenance.trim().is_empty()
        || manifest.trust.review_status.trim().is_empty()
    {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter identity, publisher, provenance, and review fields are required",
        ));
    }

    if !protocol_intersects(
        manifest.relay.protocol_min,
        manifest.relay.protocol_max,
    ) {
        return Err(AdapterError::new(
            "ADAPTER_PROTOCOL_INCOMPATIBLE",
            "adapter protocol range does not overlap RELAY adapter protocol",
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

    validate_requested_permissions(&manifest.permissions, policy)?;
    validate_bindings(&manifest.relay.command_bindings)?;

    let actual = sha256_file(worker_path)?;
    if !is_hex_digest(&manifest.artifact.sha256)
        || !manifest.artifact.sha256.eq_ignore_ascii_case(&actual)
    {
        return Err(AdapterError::new(
            "ADAPTER_INTEGRITY_MISMATCH",
            "worker executable digest does not match manifest",
        ));
    }

    validate_components(&manifest.components, &actual)?;

    if manifest.artifact.source.trim().is_empty()
        || manifest.update.channel.trim().is_empty()
        || manifest.update.source.trim().is_empty()
    {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "artifact/update source metadata is required",
        ));
    }

    Ok(())
}

fn validate_requested_permissions(
    requested: &RequestedPermissions,
    policy: &BrokerPolicy,
) -> Result<(), AdapterError> {
    if requested.network && !policy.allow_network {
        return Err(AdapterError::new(
            "ADAPTER_PERMISSION_DENIED",
            "manifest requests network access not granted by policy",
        ));
    }
    if requested.subprocess && !policy.allow_subprocess {
        return Err(AdapterError::new(
            "ADAPTER_PERMISSION_DENIED",
            "manifest requests subprocess access not granted by policy",
        ));
    }

    for scope in &requested.project_read {
        if !policy.allowed_project_read.contains(scope) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("ungranted project-read scope {scope}"),
            ));
        }
    }
    for scope in &requested.project_write {
        if !policy.allowed_project_write.contains(scope) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("ungranted project-write scope {scope}"),
            ));
        }
    }
    for handle in &requested.credentials {
        if !policy.allowed_credentials.contains(handle) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("ungranted credential handle {handle}"),
            ));
        }
    }
    for app in &requested.external_apps {
        if !policy.allowed_external_apps.contains(app) {
            return Err(AdapterError::new(
                "ADAPTER_PERMISSION_DENIED",
                format!("ungranted external app {app}"),
            ));
        }
    }
    Ok(())
}

fn validate_bindings(bindings: &[CommandBinding]) -> Result<(), AdapterError> {
    if bindings.is_empty() {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter must declare at least one command binding",
        ));
    }

    let mut capabilities = BTreeSet::new();
    for binding in bindings {
        if binding.capability.trim().is_empty()
            || !capabilities.insert(binding.capability.clone())
        {
            return Err(AdapterError::new(
                "ADAPTER_MANIFEST_INVALID",
                "capability IDs must be non-empty and unique",
            ));
        }

        match registry::resolve_command(
            &binding.command,
            Some(binding.command_version),
        ) {
            Ok(_) if registry::is_surface_exposed(
                &binding.command,
                "adapter",
            ) => {}
            Ok(_) => {
                return Err(AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!(
                        "{}@{} is not exposed to adapters",
                        binding.command, binding.command_version
                    ),
                ));
            }
            Err(_) => {
                return Err(AdapterError::new(
                    "ADAPTER_COMMAND_INCOMPATIBLE",
                    format!(
                        "unknown or incompatible command {}@{}",
                        binding.command, binding.command_version
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn validate_components(
    components: &[ComponentMetadata],
    actual_worker_sha256: &str,
) -> Result<(), AdapterError> {
    if components.is_empty() {
        return Err(AdapterError::new(
            "ADAPTER_MANIFEST_INVALID",
            "adapter must inventory at least the worker component",
        ));
    }

    let mut worker_match = false;
    let mut ids = BTreeSet::new();
    for component in components {
        if component.id.trim().is_empty()
            || !ids.insert(component.id.clone())
            || component.kind.trim().is_empty()
            || component.version.trim().is_empty()
            || component.source.trim().is_empty()
            || !is_hex_digest(&component.sha256)
        {
            return Err(AdapterError::new(
                "ADAPTER_MANIFEST_INVALID",
                "component identity, kind, version, source, and digest must be valid",
            ));
        }

        if component.kind == "worker"
            && component
                .sha256
                .eq_ignore_ascii_case(actual_worker_sha256)
        {
            worker_match = true;
        }
    }

    if !worker_match {
        return Err(AdapterError::new(
            "ADAPTER_INTEGRITY_MISMATCH",
            "worker component digest does not match launched artifact",
        ));
    }

    Ok(())
}
