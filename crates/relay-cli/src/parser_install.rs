//! Offline, local-user parser package installation. The daemon verifies the grant again
//! at startup; this path never treats a project configuration as source authority.
use crate::client;
use relay_adapter::{AdapterManifest, BrokerPolicy, sha256_file, validate_manifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_WORKER_BYTES: u64 = 128 * 1024 * 1024;
static TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub project_id: String,
    pub manifest_path: PathBuf,
    pub worker_path: PathBuf,
    pub source_extensions: Vec<String>,
    pub allow_source_delivery: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationFile {
    format_version: u32,
    installations: Vec<InstallationGrant>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationGrant {
    project_id: String,
    manifest_path: PathBuf,
    worker_path: PathBuf,
    adapter_id: String,
    adapter_version: String,
    worker_sha256: String,
    target_tool: String,
    target_version: String,
    allow_source_delivery: bool,
    source_extensions: Vec<String>,
}

fn token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}-{}", std::process::id(), TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed))
}

fn validate_extensions(values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.len() > 32 {
        return Err("PARSER_EXTENSIONS_INVALID: select 1 to 32 source extensions".into());
    }
    let mut unique = BTreeSet::new();
    for value in values {
        if value.is_empty()
            || value.len() > 16
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !unique.insert(value)
        {
            return Err(
                "PARSER_EXTENSIONS_INVALID: use distinct lowercase extensions without dots".into(),
            );
        }
    }
    Ok(())
}

fn validate_package(
    options: &InstallOptions,
) -> Result<(AdapterManifest, PathBuf, PathBuf), String> {
    if !options.allow_source_delivery {
        return Err(
            "PARSER_SOURCE_GRANT_REQUIRED: explicitly allow project source delivery".into(),
        );
    }
    if options.project_id.is_empty()
        || options.project_id.len() > 128
        || !options
            .project_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "PARSER_PROJECT_INVALID: use an existing project ID with letters, digits, - or _"
                .into(),
        );
    }
    validate_extensions(&options.source_extensions)?;
    for path in [&options.manifest_path, &options.worker_path] {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("PARSER_PACKAGE_INVALID: package entries must be ordinary files".into());
        }
    }
    let manifest_path = fs::canonicalize(&options.manifest_path)
        .map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?;
    let worker_path = fs::canonicalize(&options.worker_path)
        .map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?;
    if manifest_path.parent() != worker_path.parent()
        || manifest_path.file_name().and_then(|v| v.to_str()) != Some("manifest.json")
        || worker_path
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("exe"))
    {
        return Err("PARSER_PACKAGE_INVALID: manifest.json and worker .exe must share one package directory".into());
    }
    if fs::metadata(&manifest_path)
        .map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?
        .len()
        > MAX_MANIFEST_BYTES
    {
        return Err("PARSER_MANIFEST_TOO_LARGE".into());
    }
    if fs::metadata(&worker_path)
        .map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?
        .len()
        > MAX_WORKER_BYTES
    {
        return Err("PARSER_WORKER_TOO_LARGE".into());
    }
    let bytes = fs::read(&manifest_path).map_err(|_| "PARSER_PACKAGE_UNREADABLE".to_string())?;
    let manifest: AdapterManifest =
        serde_json::from_slice(&bytes).map_err(|_| "PARSER_MANIFEST_INVALID".to_string())?;
    if manifest.permissions.project_read != [options.project_id.clone()]
        || manifest.permissions.network
        || manifest.permissions.subprocess
        || !manifest.permissions.project_write.is_empty()
        || !manifest.permissions.credentials.is_empty()
        || !manifest.permissions.external_apps.is_empty()
        || !manifest
            .relay
            .command_bindings
            .iter()
            .any(|b| b.command == "adapter.dependencies.parse" && b.command_version == 1)
    {
        return Err("PARSER_SCOPE_DENIED: package must request only this project's read scope and parser binding".into());
    }
    let policy = BrokerPolicy {
        allowed_project_read: BTreeSet::from([options.project_id.clone()]),
        allowed_project_write: BTreeSet::new(),
        allow_network: false,
        allowed_credentials: BTreeSet::new(),
        allow_subprocess: false,
        allowed_external_apps: BTreeSet::new(),
        target_tool: manifest.target.tool.clone(),
        target_version: manifest.target.version.clone(),
        max_process_memory_bytes: 64 * 1024 * 1024,
        request_timeout_ms: 3_000,
        max_failures_before_quarantine: 2,
        backoff_ms: 500,
    };
    validate_manifest(&manifest, &policy, &worker_path)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    Ok((manifest, manifest_path, worker_path))
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn publish_file(temp: &Path, target: &Path) -> Result<(), String> {
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    let result = unsafe { MoveFileExW(wide(temp).as_ptr(), wide(target).as_ptr(), flags) };
    if result == 0 {
        return Err("PARSER_INSTALL_PUBLISH_FAILED".into());
    }
    Ok(())
}

pub fn install(options: InstallOptions) -> Result<String, String> {
    let state_dir = client::state_dir();
    install_in_state_dir(&state_dir, options)
}

pub fn install_in_state_dir(state_dir: &Path, options: InstallOptions) -> Result<String, String> {
    // Installation changes startup state; a running host must never retain a stale grant map.
    if state_dir.join("host.json").exists() {
        return Err("PARSER_HOST_MUST_STOP: shut down RELAY before installing a parser".into());
    }
    let (manifest, source_manifest, source_worker) = validate_package(&options)?;
    fs::create_dir_all(&state_dir).map_err(|_| "PARSER_STATE_UNAVAILABLE".to_string())?;
    let lock_path = state_dir.join("parser-install.lock");
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|_| "PARSER_INSTALL_BUSY: another install may be in progress".to_string())?;
    lock_file
        .try_lock()
        .map_err(|_| "PARSER_INSTALL_BUSY: another install may be in progress".to_string())?;
    if state_dir.join("host.json").exists() {
        return Err("PARSER_HOST_MUST_STOP: shut down RELAY before installing a parser".into());
    }
    let registry_path = state_dir.join("parser-installations.json");
    let mut registry = match fs::read(&registry_path) {
        Ok(bytes) if bytes.len() <= MAX_MANIFEST_BYTES as usize => {
            serde_json::from_slice::<InstallationFile>(&bytes)
                .map_err(|_| "PARSER_INSTALL_REGISTRY_INVALID".to_string())?
        }
        Ok(_) => return Err("PARSER_INSTALL_REGISTRY_INVALID".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => InstallationFile {
            format_version: 1,
            installations: Vec::new(),
        },
        Err(_) => return Err("PARSER_INSTALL_REGISTRY_UNREADABLE".into()),
    };
    if registry.format_version != 1 || registry.installations.len() >= 32 {
        return Err("PARSER_INSTALL_REGISTRY_INCOMPATIBLE".into());
    }
    if registry
        .installations
        .iter()
        .any(|grant| grant.project_id == options.project_id)
    {
        return Err("PARSER_ALREADY_INSTALLED: one parser grant per project".into());
    }
    let packages_root = state_dir.join("parser-packages");
    fs::create_dir_all(&packages_root).map_err(|_| "PARSER_PACKAGE_STAGE_FAILED".to_string())?;
    let package_dir = packages_root.join(format!("package-{}", token()));
    fs::create_dir(&package_dir).map_err(|_| "PARSER_PACKAGE_STAGE_FAILED".to_string())?;
    let result = (|| {
        let worker_name = source_worker.file_name().ok_or("PARSER_PACKAGE_INVALID")?;
        let staged_manifest = package_dir.join("manifest.json");
        let staged_worker = package_dir.join(worker_name);
        fs::copy(&source_manifest, &staged_manifest).map_err(|_| "PARSER_PACKAGE_STAGE_FAILED")?;
        fs::copy(&source_worker, &staged_worker).map_err(|_| "PARSER_PACKAGE_STAGE_FAILED")?;
        let staged = InstallOptions {
            manifest_path: staged_manifest.clone(),
            worker_path: staged_worker.clone(),
            ..options.clone()
        };
        let (staged_manifest_data, staged_manifest, staged_worker) = validate_package(&staged)?;
        if staged_manifest_data != manifest {
            return Err("PARSER_PACKAGE_CHANGED".to_string());
        }
        let digest =
            sha256_file(&staged_worker).map_err(|_| "PARSER_PACKAGE_STAGE_FAILED".to_string())?;
        registry.installations.push(InstallationGrant {
            project_id: options.project_id.clone(),
            manifest_path: staged_manifest,
            worker_path: staged_worker,
            adapter_id: manifest.adapter.id.clone(),
            adapter_version: manifest.adapter.version.clone(),
            worker_sha256: digest,
            target_tool: manifest.target.tool.clone(),
            target_version: manifest.target.version.clone(),
            allow_source_delivery: true,
            source_extensions: options.source_extensions,
        });
        let bytes =
            serde_json::to_vec_pretty(&registry).map_err(|_| "PARSER_INSTALL_SERIALIZE_FAILED")?;
        let temp = state_dir.join(format!("parser-installations-{}.tmp", token()));
        let write_result = (|| {
            let mut file = File::create_new(&temp).map_err(|_| "PARSER_INSTALL_PUBLISH_FAILED")?;
            file.write_all(&bytes)
                .map_err(|_| "PARSER_INSTALL_PUBLISH_FAILED")?;
            file.sync_all()
                .map_err(|_| "PARSER_INSTALL_PUBLISH_FAILED")?;
            publish_file(&temp, &registry_path).map_err(|_| "PARSER_INSTALL_PUBLISH_FAILED")
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        write_result.map_err(str::to_string)?;
        Ok::<_, String>(format!(
            "Parser {}@{} installed for project {}. Start RELAY to activate it.",
            manifest.adapter.id, manifest.adapter.version, options.project_id
        ))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&package_dir);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (PathBuf, InstallOptions) {
        let root = std::env::temp_dir().join(format!("relay-parser-install-test-{}", token()));
        let package = root.join("source-package");
        fs::create_dir_all(&package).unwrap();
        let worker_path = package.join("worker.exe");
        fs::write(&worker_path, b"synthetic worker artifact").unwrap();
        let digest = sha256_file(&worker_path).unwrap();
        let manifest_path = package.join("manifest.json");
        let manifest = json!({
            "manifest_format": 1,
            "adapter": { "id": "fixture.parser", "version": "1.0.0", "display_name": "Fixture parser" },
            "publisher": { "id": "fixture.publisher", "source": "local-test" },
            "artifact": { "sha256": digest, "source": "local-test" },
            "relay": { "protocol_min": 1, "protocol_max": 1, "command_bindings": [{
                "command": "adapter.dependencies.parse", "command_version": 1,
                "capability": "fixture.dependencies.parse"
            }] },
            "permissions": { "project_read": ["PRJ-fixture"], "project_write": [],
                "network": false, "credentials": [], "subprocess": false, "external_apps": [] },
            "target": { "tool": "synthetic", "version": "1.0.0" },
            "components": [{ "id": "worker", "kind": "worker", "version": "1.0.0",
                "sha256": digest, "source": "local-test" }],
            "dependencies": [],
            "update": { "channel": "local-test", "source": "local-test" },
            "trust": { "build_provenance": "fixture", "review_status": "test-fixture" }
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        (
            root,
            InstallOptions {
                project_id: "PRJ-fixture".into(),
                manifest_path,
                worker_path,
                source_extensions: vec!["json".into()],
                allow_source_delivery: true,
            },
        )
    }

    #[test]
    fn stages_verified_package_and_explicit_grant() {
        let (root, options) = fixture();
        let state = root.join("state");
        let message = install_in_state_dir(&state, options.clone()).unwrap();
        assert!(message.contains("Start RELAY"));
        let registry: InstallationFile =
            serde_json::from_slice(&fs::read(state.join("parser-installations.json")).unwrap())
                .unwrap();
        assert_eq!(registry.installations.len(), 1);
        let grant = &registry.installations[0];
        assert!(grant.allow_source_delivery);
        assert_eq!(grant.project_id, "PRJ-fixture");
        let package_root = fs::canonicalize(state.join("parser-packages")).unwrap();
        assert!(grant.manifest_path.starts_with(&package_root));
        assert!(grant.worker_path.starts_with(&package_root));
        assert_eq!(
            sha256_file(&grant.worker_path).unwrap(),
            grant.worker_sha256
        );
        assert_eq!(
            fs::read(&grant.manifest_path).unwrap(),
            fs::read(&options.manifest_path).unwrap()
        );
        assert!(state.join("parser-install.lock").exists());
        let duplicate = install_in_state_dir(&state, options).unwrap_err();
        assert!(duplicate.starts_with("PARSER_ALREADY_INSTALLED"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_implicit_scope_digest_and_running_host_without_publishing() {
        let (root, mut options) = fixture();
        let state = root.join("state");
        options.allow_source_delivery = false;
        assert!(
            install_in_state_dir(&state, options.clone())
                .unwrap_err()
                .starts_with("PARSER_SOURCE_GRANT_REQUIRED")
        );
        options.allow_source_delivery = true;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&options.manifest_path).unwrap()).unwrap();
        manifest["permissions"]["network"] = json!(true);
        fs::write(
            &options.manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(
            install_in_state_dir(&state, options.clone())
                .unwrap_err()
                .starts_with("PARSER_SCOPE_DENIED")
        );
        manifest["permissions"]["network"] = json!(false);
        manifest["artifact"]["sha256"] = json!("0".repeat(64));
        fs::write(
            &options.manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(
            install_in_state_dir(&state, options.clone())
                .unwrap_err()
                .contains("ADAPTER_INTEGRITY_MISMATCH")
        );
        assert!(!state.join("parser-installations.json").exists());
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("host.json"), b"active host marker").unwrap();
        assert!(
            install_in_state_dir(&state, options)
                .unwrap_err()
                .starts_with("PARSER_HOST_MUST_STOP")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn appends_a_second_project_without_changing_first_grant() {
        let (first_root, first) = fixture();
        let (second_root, mut second) = fixture();
        let state = first_root.join("state");
        install_in_state_dir(&state, first).unwrap();
        let first_bytes = fs::read(state.join("parser-installations.json")).unwrap();
        let first_file: InstallationFile = serde_json::from_slice(&first_bytes).unwrap();
        second.project_id = "PRJ-second".into();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&second.manifest_path).unwrap()).unwrap();
        manifest["permissions"]["project_read"] = json!(["PRJ-second"]);
        fs::write(&second.manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        install_in_state_dir(&state, second).unwrap();
        let combined: InstallationFile = serde_json::from_slice(
            &fs::read(state.join("parser-installations.json")).unwrap()).unwrap();
        assert_eq!(combined.installations.len(), 2);
        assert_eq!(
            serde_json::to_value(&combined.installations[0]).unwrap(),
            serde_json::to_value(&first_file.installations[0]).unwrap()
        );
        assert_eq!(combined.installations[1].project_id, "PRJ-second");
        fs::remove_dir_all(first_root).unwrap();
        fs::remove_dir_all(second_root).unwrap();
    }
}
