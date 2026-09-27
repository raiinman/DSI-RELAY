use relay_contracts::CommandRequest;
use relay_core::service::{ExtensionError, RelayCore};
use serde_json::Value;

/// Local deterministic asset checks after Core validation and project scope checks.
pub fn execute(
    core: &RelayCore,
    request: &CommandRequest,
) -> Option<Result<Value, ExtensionError>> {
    if request.command != "assets.manifest.validate" && request.command != "assets.krita.inspect" {
        return None;
    }
    let project_id = request.arguments["project_id"]
        .as_str()
        .expect("shared registry validates project_id");
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
