use relay_contracts::CommandRequest;
use relay_core::{policy::ExecutionAuthority, service::{ExtensionError, RelayCore, RuntimeContext}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Local deterministic asset checks after Core validation and project scope checks.
pub fn execute_authorized(
    core: &RelayCore,
    request: &CommandRequest,
    runtime: &RuntimeContext,
    authority: &ExecutionAuthority,
) -> Option<Result<Value, ExtensionError>> {
    if !matches!(request.command.as_str(),
        "assets.manifest.validate" | "assets.krita.inspect" | "assets.impact.analyze"
        | "assets.blender.mesh.validate" | "assets.krita.export") {
        return None;
    }
    let project_id = request.arguments["project_id"]
        .as_str()
        .expect("shared registry validates project_id");
    if request.command == "assets.blender.mesh.validate" {
        return Some((|| {
            let root = core.trusted_project_root(project_id)?;
            let relative = request.arguments["blend_path"].as_str()
                .expect("shared registry validates blend_path");
            let file = project_blend_file(&root, relative)?;
            let report = relay_blender::validate_blend(&file, None);
            blender_result(report)
        })());
    }
    if request.command == "assets.krita.export" {
        return Some((|| {
            authority.require_permission("state_write").map_err(|error| {
                ExtensionError::new(error.code, "build recording permission is not granted")
            })?;
            authority.require_effect("relay_state_write").map_err(|error| {
                ExtensionError::new(error.code, "build recording effect is not granted")
            })?;
            let root = core.trusted_project_root(project_id)?;
            let source_relative = request.arguments["source_path"].as_str()
                .expect("shared registry validates source_path");
            let export_relative = request.arguments["export_path"].as_str()
                .expect("shared registry validates export_path");
            let source = project_existing_file(&root, source_relative, "kra")?;
            let target = project_new_file(&root, export_relative, "png")?;
            let report = relay_krita::export_png(&source, &target);
            let mut result = serde_json::to_value(&report).map_err(|_| {
                ExtensionError::new("ASSET_SERIALIZATION_FAILED", "Krita export result is unavailable")
            })?;
            result["build_record_state"] = json!("not_attempted");
            result["result_id"] = Value::Null;
            result["source_identity_sha256"] = Value::Null;
            result["export_identity_sha256"] = Value::Null;
            if report.status == relay_krita::ExportStatus::Exported {
                let source_identity = relative_identity_sha256(source_relative);
                let export_identity = relative_identity_sha256(export_relative);
                match record_krita_build(core, request, runtime, authority, &source_identity,
                    &export_identity, &report) {
                    Some(id) => {
                        result["build_record_state"] = json!("stored");
                        result["result_id"] = json!(id);
                        result["source_identity_sha256"] = json!(source_identity);
                        result["export_identity_sha256"] = json!(export_identity);
                    }
                    None => {
                        return Err(ExtensionError::new("ASSET_BUILD_RECORD_FAILED",
                            "PNG was published, but build evidence could not be stored; the output remains in place"));
                    }
                }
            }
            Ok(result)
        })());
    }
    let manifest_text = request.arguments["manifest_json"]
        .as_str()
        .expect("shared registry validates manifest_json");
    Some((|| {
        let manifest = relay_assets::parse_manifest(manifest_text.as_bytes()).map_err(|_| {
            ExtensionError::new("ASSET_MANIFEST_INVALID", "asset manifest is invalid")
        })?;
        if manifest.project_id != project_id {
            return Err(ExtensionError::new(
                "ASSET_PROJECT_MISMATCH",
                "asset manifest project does not match selected project",
            ));
        }
        let root = core.trusted_project_root(project_id)?;
        if request.command == "assets.impact.analyze" {
            // Impact is structural. Missing or stale files are expected change inputs;
            // the analyzer rechecks bounded IDs, relationships, and path syntax.
            let changed_paths: Vec<String> = request.arguments["changed_paths"].as_array()
                .expect("shared registry validates changed_paths")
                .iter().map(|path| path.as_str().expect("shared registry validates path strings").to_string())
                .collect();
            let report = relay_assets::analyze_impact(&manifest, &changed_paths).map_err(|_| {
                ExtensionError::new("ASSET_IMPACT_INVALID", "asset impact input is invalid")
            })?;
            return serde_json::to_value(report).map_err(|_| {
                ExtensionError::new("ASSET_SERIALIZATION_FAILED", "asset impact result is unavailable")
            });
        }
        if request.command == "assets.krita.inspect" {
            let report = relay_krita::inspect(&manifest, &root, None).map_err(|_| {
                ExtensionError::new("ASSET_VALIDATION_UNAVAILABLE", "asset validation is unavailable")
            })?;
            return serde_json::to_value(report).map_err(|_| {
                ExtensionError::new("ASSET_SERIALIZATION_FAILED", "Krita inspection result is unavailable")
            });
        }
        let mut report = relay_assets::validate_manifest(&manifest, &root).map_err(|_| {
            ExtensionError::new("ASSET_VALIDATION_UNAVAILABLE", "asset validation is unavailable")
        })?;
        if report.findings.len() > 64 {
            report.findings.truncate(64);
            report.findings_truncated = true;
        }
        serde_json::to_value(report).map_err(|_| {
            ExtensionError::new(
                "ASSET_SERIALIZATION_FAILED",
                "asset validation result is unavailable",
            )
        })
    })())
}

#[cfg(test)]
fn execute(core: &RelayCore, request: &CommandRequest) -> Option<Result<Value, ExtensionError>> {
    execute_authorized(core, request, &RuntimeContext::in_process(),
        &ExecutionAuthority::local_user("asset-test"))
}

fn record_krita_build(
    core: &RelayCore,
    request: &CommandRequest,
    runtime: &RuntimeContext,
    authority: &ExecutionAuthority,
    source_identity: &str,
    export_identity: &str,
    report: &relay_krita::ExportReport,
) -> Option<String> {
    let source_sha256 = report.source_sha256.as_ref()?;
    let output_sha256 = report.output_sha256.as_ref()?;
    let output_bytes = report.output_bytes?;
    let mut request_hasher = Sha256::new();
    request_hasher.update(b"relay-krita-build-request-v1\0");
    request_hasher.update(request.request_id.as_bytes());
    if let Some(key) = &request.idempotency_key {
        request_hasher.update(b"\0");
        request_hasher.update(key.as_bytes());
    }
    let request_digest: String = request_hasher.finalize().iter()
        .map(|byte| format!("{byte:02x}")).collect();
    let inner = CommandRequest {
        request_id: format!("BUILD-{}", &request_digest[..32]),
        command: "result.put".to_string(),
        command_version: Some(1),
        arguments: json!({
            "project_id": request.arguments["project_id"],
            "kind": "ASSET_KRITA_EXPORT_BUILD",
            "payload": {
                "schema_version": 1,
                "source_identity_sha256": source_identity,
                "export_identity_sha256": export_identity,
                "source_sha256": source_sha256,
                "output_sha256": output_sha256,
                "output_bytes": output_bytes,
                "native_workflow_status": "checked",
                "export_status": "exported"
            }
        }),
        idempotency_key: None,
        context: request.context.clone(),
    };
    let stored = core.execute_authorized(inner, runtime, authority);
    stored.ok.then(|| stored.result?["id"].as_str().map(str::to_string)).flatten()
}

fn relative_identity_sha256(relative: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"relay-project-relative-path-v1\0");
    hasher.update(relative.as_bytes());
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn project_blend_file(root: &Path, relative: &str) -> Result<PathBuf, ExtensionError> {
    project_existing_file(root, relative, "blend")
}

fn valid_asset_relative(relative: &str, extension: &str) -> bool {
    if relative.is_empty() || relative.len() > 512 || relative.starts_with('/')
        || relative.contains(['\\', ':']) || relative.bytes().any(|byte| byte < 32 || byte == 127)
        || !Path::new(relative).extension().and_then(|part| part.to_str())
            .is_some_and(|part| part.eq_ignore_ascii_case(extension))
    {
        return false;
    }
    let parts: Vec<&str> = relative.split('/').collect();
    if parts.iter().any(|part| part.is_empty() || *part == "." || *part == ".."
        || part.ends_with([' ', '.']) || is_windows_device_name(part)) {
        return false;
    }
    true
}

fn project_existing_file(root: &Path, relative: &str, extension: &str) -> Result<PathBuf, ExtensionError> {
    let invalid = || ExtensionError::new("ASSET_PATH_INVALID", "project-relative asset path is invalid");
    if !valid_asset_relative(relative, extension) { return Err(invalid()); }
    let parts: Vec<&str> = relative.split('/').collect();
    let mut file = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        file.push(part);
        match fs::symlink_metadata(&file) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata)
                    || (index + 1 < parts.len() && !metadata.is_dir())
                    || (index + 1 == parts.len() && !metadata.is_file()) {
                    return Err(invalid());
                }
            }
            Err(_) => return Err(invalid()),
        }
    }
    let canonical = fs::canonicalize(&file).map_err(|_| invalid())?;
    if !canonical.starts_with(root) {
        return Err(invalid());
    }
    Ok(canonical)
}

fn project_new_file(root: &Path, relative: &str, extension: &str) -> Result<PathBuf, ExtensionError> {
    let invalid = || ExtensionError::new("ASSET_PATH_INVALID", "project-relative export path is invalid");
    if !valid_asset_relative(relative, extension) { return Err(invalid()); }
    let parts: Vec<&str> = relative.split('/').collect();
    let mut parent = root.to_path_buf();
    for part in &parts[..parts.len() - 1] {
        parent.push(part);
        let meta = fs::symlink_metadata(&parent).map_err(|_| invalid())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err(invalid());
        }
    }
    let canonical = fs::canonicalize(parent).map_err(|_| invalid())?;
    if !canonical.starts_with(root) { return Err(invalid()); }
    let target = canonical.join(parts[parts.len() - 1]);
    if fs::symlink_metadata(&target).is_ok() { return Err(invalid()); }
    Ok(target)
}

fn blender_result(report: relay_blender::ValidationReport) -> Result<Value, ExtensionError> {
    let native_workflow_status = match &report.status {
        relay_blender::ValidationStatus::Passed | relay_blender::ValidationStatus::Failed => "checked",
        relay_blender::ValidationStatus::Incomplete => "incomplete",
        relay_blender::ValidationStatus::Unavailable
        | relay_blender::ValidationStatus::InvalidInput
        | relay_blender::ValidationStatus::ToolError => "untested",
    };
    let mut result = serde_json::to_value(report).map_err(|_| {
        ExtensionError::new("ASSET_SERIALIZATION_FAILED", "Blender result is unavailable")
    })?;
    result["native_workflow_status"] = json!(native_workflow_status);
    Ok(result)
}

fn is_windows_device_name(segment: &str) -> bool {
    let base = segment.split('.').next().unwrap_or_default().to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool { false }

#[cfg(test)]
mod tests {
    use super::*;
    use relay_contracts::RequestContext;
    use relay_core::{policy::ExecutionAuthority, service::{CoreConfig, RuntimeContext}};
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn request(command: &str, arguments: Value) -> CommandRequest {
        CommandRequest {
            request_id: format!("REQ-{command}"),
            command: command.to_string(),
            command_version: Some(1),
            arguments,
            idempotency_key: None,
            context: RequestContext::default(),
        }
    }

    #[test]
    fn impact_command_enforces_scope_and_returns_ids_without_paths() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("relay-impact-{}-{nonce}", std::process::id()));
        let project_root = dir.join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join("source.blend"), b"source").unwrap();
        fs::write(project_root.join("export.fbx"), b"export").unwrap();
        let core = RelayCore::open(CoreConfig::new(dir.join("state")));
        let runtime = RuntimeContext::in_process();
        let registered = core.execute(request("project.register", json!({
            "id": "PRJ-impact", "name": "Impact", "root_uri": project_root.to_string_lossy()
        })), &runtime);
        assert!(registered.ok, "{registered:?}");
        let manifest = json!({
            "schema_version": 1, "project_id": "PRJ-impact", "assets": [{
                "id": "mesh_1", "creator_tool": "blender", "kind": "mesh",
                "source": "source.blend", "export": "export.fbx",
                "source_revision": "rev1", "export_built_from_revision": "rev1"
            }]
        }).to_string();
        let arguments = json!({
            "project_id": "PRJ-impact", "manifest_json": manifest,
            "changed_paths": ["source.blend"]
        });
        let response = core.execute_authorized_with_extension(
            request("assets.impact.analyze", arguments.clone()), &runtime,
            &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert!(response.ok, "{response:?}");
        let result = response.result.unwrap();
        assert_eq!(result["creator_app_execution"], "not_checked");
        assert_eq!(result["assets"][0]["asset_id"], "mesh_1");
        assert_eq!(result["assets"][0]["reasons"][0]["kind"], "source_changed");
        let encoded = result.to_string();
        assert!(!encoded.contains("source.blend"));
        assert!(!encoded.contains("export.fbx"));
        assert!(!encoded.contains(project_root.to_string_lossy().as_ref()));

        fs::remove_file(project_root.join("source.blend")).unwrap();
        let deleted_source = core.execute_authorized_with_extension(
            request("assets.impact.analyze", arguments.clone()), &runtime,
            &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert!(deleted_source.ok, "{deleted_source:?}");
        assert_eq!(deleted_source.result.unwrap()["assets"][0]["asset_id"], "mesh_1");

        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized_with_extension(
            request("assets.impact.analyze", arguments.clone()), &runtime, &scoped,
            |request| execute(&core, request),
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        let invalid = core.execute_authorized_with_extension(
            request("assets.impact.analyze", json!({
                "project_id": "PRJ-impact", "manifest_json": manifest,
                "changed_paths": ["../outside"]
            })), &runtime, &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert_eq!(invalid.error.unwrap().code, "ASSET_IMPACT_INVALID");
        let unsafe_manifest = json!({
            "schema_version": 1, "project_id": "PRJ-impact", "assets": [{
                "id": "mesh_1", "creator_tool": "blender", "kind": "mesh",
                "source": "../outside", "export": "export.fbx"
            }]
        }).to_string();
        let invalid = core.execute_authorized_with_extension(
            request("assets.impact.analyze", json!({
                "project_id": "PRJ-impact", "manifest_json": unsafe_manifest,
                "changed_paths": ["export.fbx"]
            })), &runtime, &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert_eq!(invalid.error.unwrap().code, "ASSET_IMPACT_INVALID");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn blender_command_rejects_missing_or_escaping_paths_and_scopes() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("relay-blender-route-{}-{nonce}", std::process::id()));
        let project_root = dir.join("project");
        fs::create_dir_all(&project_root).unwrap();
        let core = RelayCore::open(CoreConfig::new(dir.join("state")));
        let runtime = RuntimeContext::in_process();
        let registered = core.execute(request("project.register", json!({
            "id": "PRJ-blender", "name": "Blender", "root_uri": project_root.to_string_lossy()
        })), &runtime);
        assert!(registered.ok, "{registered:?}");
        let missing = core.execute_authorized_with_extension(
            request("assets.blender.mesh.validate", json!({
                "project_id": "PRJ-blender", "blend_path": "missing.blend"
            })), &runtime, &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert_eq!(missing.error.unwrap().code, "ASSET_PATH_INVALID");
        let blend = project_root.join("fixture.blend");
        fs::write(&blend, b"fixture").unwrap();
        let unavailable = blender_result(relay_blender::validate_blend(
            &blend, Some(&project_root.join("missing-blender.exe")),
        )).unwrap_or_else(|_| panic!("Blender report could not be serialized"));
        assert_eq!(unavailable["status"], "unavailable");
        assert_eq!(unavailable["native_workflow_status"], "untested");
        assert!(!unavailable.to_string().contains(project_root.to_string_lossy().as_ref()));
        let spec = relay_contracts::registry::resolve_command("assets.blender.mesh.validate", Some(1)).unwrap();
        assert!(relay_contracts::registry::validate_value(&spec.result_schema, &unavailable).is_ok());
        let escaped = core.execute_authorized_with_extension(
            request("assets.blender.mesh.validate", json!({
                "project_id": "PRJ-blender", "blend_path": "../outside.blend"
            })), &runtime, &ExecutionAuthority::local_user("CLIENT-local"),
            |request| execute(&core, request),
        );
        assert_eq!(escaped.error.unwrap().code, "ASSET_PATH_INVALID");
        let mut scoped = ExecutionAuthority::local_user("CLIENT-scoped");
        scoped.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let denied = core.execute_authorized_with_extension(
            request("assets.blender.mesh.validate", json!({
                "project_id": "PRJ-blender", "blend_path": "missing.blend"
            })), &runtime, &scoped, |request| execute(&core, request),
        );
        assert_eq!(denied.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn krita_export_paths_are_create_only_and_project_local() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("relay-krita-path-{}-{nonce}", std::process::id()));
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join("assets/source.kra"), b"fixture").unwrap();
        let root = fs::canonicalize(&dir).unwrap();
        assert!(project_existing_file(&root, "assets/source.kra", "kra").is_ok());
        assert!(project_new_file(&root, "assets/export.png", "png").is_ok());
        fs::write(dir.join("assets/export.png"), b"existing").unwrap();
        assert!(project_new_file(&root, "assets/export.png", "png").is_err());
        assert!(project_new_file(&root, "../escape.png", "png").is_err());
        assert!(project_new_file(&root, "missing/export.png", "png").is_err());
        assert!(project_new_file(&root, "assets/export.kra", "png").is_err());
        let mut result = serde_json::to_value(relay_krita::export_png(
            Path::new("wrong.psd"), Path::new("unused.png"),
        )).unwrap();
        result["build_record_state"] = json!("not_attempted");
        result["result_id"] = Value::Null;
        result["source_identity_sha256"] = Value::Null;
        result["export_identity_sha256"] = Value::Null;
        let spec = relay_contracts::registry::resolve_command("assets.krita.export", Some(1)).unwrap();
        assert!(relay_contracts::registry::validate_value(&spec.result_schema, &result).is_ok());
        assert_eq!(result["native_workflow_status"], "untested");
        assert!(!result.to_string().contains(root.to_string_lossy().as_ref()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn synthetic_krita_build_record_uses_shared_scoped_result_store() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("relay-krita-record-{}-{nonce}", std::process::id()));
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        let core = RelayCore::open(CoreConfig::new(dir.join("state")));
        let runtime = RuntimeContext::in_process();
        let registered = core.execute(request("project.register", json!({
            "id": "PRJ-krita-record", "name": "Krita record", "root_uri": root.to_string_lossy()
        })), &runtime);
        assert!(registered.ok, "{registered:?}");
        let outer = request("assets.krita.export", json!({
            "project_id": "PRJ-krita-record", "source_path": "art/source.kra",
            "export_path": "art/export.png"
        }));
        // This is a storage fixture only. It does not assert that Krita ran.
        let fixture = relay_krita::ExportReport {
            status: relay_krita::ExportStatus::Exported,
            native_workflow_status: "checked", scope: "fixed_krita_kra_to_png_export",
            message: "fixture", output_bytes: Some(128),
            source_sha256: Some("a".repeat(64)), output_sha256: Some("b".repeat(64)),
            source_changed: false, elapsed_ms: 1,
        };
        let source_identity = relative_identity_sha256("art/source.kra");
        let export_identity = relative_identity_sha256("art/export.png");
        let authority = ExecutionAuthority::local_user("CLIENT-local");
        let id = record_krita_build(&core, &outer, &runtime, &authority,
            &source_identity, &export_identity, &fixture).unwrap();
        let mut command_result = serde_json::to_value(&fixture).unwrap();
        command_result["build_record_state"] = json!("stored");
        command_result["result_id"] = json!(id);
        command_result["source_identity_sha256"] = json!(source_identity);
        command_result["export_identity_sha256"] = json!(export_identity);
        let spec = relay_contracts::registry::resolve_command("assets.krita.export", Some(1)).unwrap();
        assert!(relay_contracts::registry::validate_value(&spec.result_schema, &command_result).is_ok());
        let result = core.execute_authorized(request("result.get", json!({
            "result_id": id
        })), &runtime, &authority);
        assert!(result.ok, "{result:?}");
        let payload = &result.result.as_ref().unwrap()["payload"];
        assert_eq!(payload["source_identity_sha256"], source_identity);
        assert_eq!(payload["export_identity_sha256"], export_identity);
        assert_eq!(payload["source_sha256"], "a".repeat(64));
        assert_eq!(payload["output_sha256"], "b".repeat(64));
        assert_eq!(result.result.as_ref().unwrap()["project_id"], "PRJ-krita-record");
        assert!(!payload.to_string().contains("art/source.kra"));
        assert!(!payload.to_string().contains("art/export.png"));
        let mut denied = ExecutionAuthority::local_user("CLIENT-denied");
        denied.permissions.remove("state_write");
        assert!(record_krita_build(&core, &outer, &runtime, &denied,
            &source_identity, &export_identity, &fixture).is_none());
        let mut scoped_writer = ExecutionAuthority::local_user("CLIENT-other-project");
        scoped_writer.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        assert!(record_krita_build(&core, &outer, &runtime, &scoped_writer,
            &source_identity, &export_identity, &fixture).is_none());
        denied.project_ids = Some(["PRJ-other".to_string()].into_iter().collect());
        let scoped = core.execute_authorized(request("result.get", json!({
            "result_id": id
        })), &runtime, &denied);
        assert_eq!(scoped.error.unwrap().code, "PROJECT_SCOPE_DENIED");
        let partial = core.execute_authorized_with_extension(outer, &runtime, &authority, |_| {
            Some(Err(ExtensionError::new("ASSET_BUILD_RECORD_FAILED",
                "PNG was published, but build evidence could not be stored; the output remains in place")))
        });
        assert_eq!(partial.error.unwrap().code, "ASSET_BUILD_RECORD_FAILED");
        let history = core.execute_authorized(request("transaction.list", json!({
            "project_id": "PRJ-krita-record", "limit": 20
        })), &runtime, &authority);
        assert!(history.ok, "{history:?}");
        assert!(history.result.unwrap()["transactions"].as_array().unwrap().iter().any(|item| {
            item["command"] == "assets.krita.export" && item["state"] == "FAILED"
        }));
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }
}
