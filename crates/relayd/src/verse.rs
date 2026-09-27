use relay_contracts::CommandRequest;
use relay_core::service::{ExtensionError, RelayCore};
use relay_verse::{AssertionSpec, CaptureContext, CaptureSourceKind};
use serde_json::{Value, json};

/// Analyze caller-supplied structured log data without claiming a live UEFN session.
pub fn execute(
    core: &RelayCore,
    request: &CommandRequest,
) -> Option<Result<Value, ExtensionError>> {
    if request.command != "runtime.capture.analyze" {
        return None;
    }
    Some((|| {
        let project_id = request.arguments["project_id"].as_str().unwrap_or_default();
        core.require_registered_project(project_id)?;
        let source_kind = match request.arguments["source_kind"].as_str() {
            Some("uefn_log") => CaptureSourceKind::UefnLog,
            Some("imported_log") => CaptureSourceKind::ImportedLog,
            Some("synthetic_fixture") => CaptureSourceKind::SyntheticFixture,
            _ => {
                return Err(ExtensionError::new(
                    "CAPTURE_INVALID",
                    "capture source is invalid",
                ));
            }
        };
        let context = CaptureContext {
            session_id: request.arguments["session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            source_kind,
            source_version: request.arguments["source_version"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            capture_ref: request.arguments["capture_ref"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };
        let capture_text = request.arguments["capture_text"]
            .as_str()
            .unwrap_or_default();
        let specs: Vec<AssertionSpec> = serde_json::from_value(
            request
                .arguments
                .get("assertions")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|_| {
            ExtensionError::new(
                "ASSERTION_SPEC_INVALID",
                "assertion specification is invalid",
            )
        })?;
        let report = relay_verse::normalize_capture(&context, capture_text)
            .map_err(|_| ExtensionError::new("CAPTURE_INVALID", "capture could not be analyzed"))?;
        let evaluation = relay_verse::evaluate_assertions(&report, &specs);
        Ok(json!({
            "schema_version": 1,
            "session_id": context.session_id,
            "source_kind": context.source_kind,
            "source_version": context.source_version,
            "capture_ref": context.capture_ref,
            "counts": report.counts,
            "quality": report.quality,
            "assertions": evaluation.results,
            "omitted_assertions": evaluation.omitted_specs,
            "live_uefn_status": "untested"
        }))
    })())
}
