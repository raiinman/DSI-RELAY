//! Bounded JSON support bundle assembled from summaries only.
//!
//! The caller maps existing diagnostic health and integrated-run results into
//! these narrow types. This crate never accepts raw log lines, filesystem paths,
//! command arguments, or arbitrary diagnostic messages.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use relay_validation::{EvidenceSource, IntegratedReport, WorkflowStatus};

pub const BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const MAX_BUNDLE_BYTES: usize = 64 * 1024;
pub const MAX_COMPONENT_VERSIONS: usize = 32;
pub const MAX_DIAGNOSTIC_CODES: usize = 64;
pub const MAX_OUTCOME_REASONS: usize = 64;
pub const MAX_INTEGRATED_REPORT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleError {
    pub code: &'static str,
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for BundleError {}

fn error(code: &'static str) -> BundleError {
    BundleError { code }
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentVersion {
    pub component_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticHealthSummary {
    pub healthy: bool,
    pub current_bytes: u64,
    pub rotated_files: usize,
    pub evicted_events: u64,
    pub evicted_files: u64,
    pub recovered_partial_bytes: u64,
    pub last_sync_unix_ms: Option<u64>,
    pub detail_active: bool,
    /// Stable code only. Do not pass Core's free-form last_error text.
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeCount {
    pub code: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCounts {
    pub total_events: u64,
    pub invalid_lines: u64,
    pub incomplete_events: u64,
    pub sampled_events: u64,
    pub untrusted_source_events: u64,
    pub retention_evicted_events: u64,
    pub by_code: Vec<CodeCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Failed,
    Blocked,
    Untested,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationCounts {
    pub passed: u32,
    pub failed: u32,
    pub blocked: u32,
    pub untested: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResources {
    pub peak_rss_bytes: Option<u64>,
    pub total_cpu_ms: Option<u64>,
    pub io_read_bytes: Option<u64>,
    pub io_write_bytes: Option<u64>,
    pub peak_gpu_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegratedRunSummary {
    pub report_schema_version: u32,
    pub run_ref: String,
    pub completed_unix_ms: u64,
    pub duration_ms: u64,
    pub overall_status: ValidationStatus,
    pub counts: ValidationCounts,
    pub resources: Option<RunResources>,
    pub outcome_reasons: Vec<CodeCount>,
}

#[derive(Debug, Clone)]
pub struct SupportInput {
    pub generated_unix_ms: u64,
    pub relay_version: String,
    pub component_versions: Vec<ComponentVersion>,
    pub diagnostic_health: DiagnosticHealthSummary,
    pub diagnostic_counts: DiagnosticCounts,
    /// `None` means the one integrated end-of-build run has not occurred.
    pub integrated_run: Option<IntegratedRunSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrivacyManifest {
    pub policy: &'static str,
    pub includes_raw_logs: bool,
    pub includes_local_paths: bool,
    pub includes_secrets: bool,
    pub includes_command_arguments: bool,
    pub includes_project_file_contents: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportBundle {
    pub schema_version: u32,
    pub generated_unix_ms: u64,
    pub relay_version: String,
    pub component_versions: Vec<ComponentVersion>,
    pub diagnostic_health: DiagnosticHealthSummary,
    pub diagnostic_counts: DiagnosticCounts,
    pub integrated_run: Option<IntegratedRunSummary>,
    pub privacy: PrivacyManifest,
}

/// Assemble a typed bundle. The returned value contains no raw log or file data.
pub fn assemble_bundle(mut input: SupportInput) -> Result<SupportBundle, BundleError> {
    validate_input(&input)?;
    input
        .component_versions
        .sort_by(|left, right| left.component_id.cmp(&right.component_id));
    input
        .diagnostic_counts
        .by_code
        .sort_by(|left, right| left.code.cmp(&right.code));
    if let Some(run) = input.integrated_run.as_mut() {
        run.outcome_reasons
            .sort_by(|left, right| left.code.cmp(&right.code));
    }
    Ok(SupportBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        generated_unix_ms: input.generated_unix_ms,
        relay_version: input.relay_version,
        component_versions: input.component_versions,
        diagnostic_health: input.diagnostic_health,
        diagnostic_counts: input.diagnostic_counts,
        integrated_run: input.integrated_run,
        privacy: PrivacyManifest {
            policy: "summary_only_v1",
            includes_raw_logs: false,
            includes_local_paths: false,
            includes_secrets: false,
            includes_command_arguments: false,
            includes_project_file_contents: false,
        },
    })
}

/// Serialize the bundle as bounded JSON bytes for a caller-controlled export.
pub fn bundle_json(input: SupportInput) -> Result<Vec<u8>, BundleError> {
    let bundle = assemble_bundle(input)?;
    let bytes = serde_json::to_vec(&bundle).map_err(|_| error("BUNDLE_ENCODE_FAILED"))?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(error("BUNDLE_TOO_LARGE"));
    }
    Ok(bytes)
}

/// Extract only bounded counts and resource totals from a completed report.
/// This is a summary of the supplied report, not independent attestation of
/// its observations or a substitute for the report's local evidence journal.
pub fn summarize_integrated_report(bytes: &[u8]) -> Result<IntegratedRunSummary, BundleError> {
    if bytes.is_empty() || bytes.len() > MAX_INTEGRATED_REPORT_BYTES {
        return Err(error("INVALID_RUN_REPORT"));
    }
    let report: IntegratedReport =
        serde_json::from_slice(bytes).map_err(|_| error("INVALID_RUN_REPORT"))?;
    if report.schema_version != relay_validation::REPORT_SCHEMA_VERSION
        || !safe_ref(&report.run_id)
        || report.workflows.is_empty()
        || report.workflows.len() > relay_validation::MAX_WORKFLOWS
        || report.completed_unix_ms < report.started_unix_ms
        || report.duration_ms != report.completed_unix_ms - report.started_unix_ms
    {
        return Err(error("INVALID_RUN_REPORT"));
    }
    let mut seen = BTreeSet::new();
    let mut counts = ValidationCounts { passed: 0, failed: 0, blocked: 0, untested: 0 };
    let mut reasons = BTreeMap::<String, u64>::new();
    let mut peak_rss = None::<u64>;
    let mut cpu_ms = None::<u64>;
    let mut io_read = None::<u64>;
    let mut io_write = None::<u64>;
    let mut peak_gpu = None::<u64>;
    for workflow in &report.workflows {
        if !safe_ref(&workflow.workflow_id)
            || !seen.insert(&workflow.workflow_id)
            || workflow.phase > 11
            || !safe_code(&workflow.reason_code)
            || (workflow.status == WorkflowStatus::Passed
                && (!matches!(workflow.evidence, EvidenceSource::Observed { .. })
                    || workflow.requirements.iter().any(|item| {
                        item.availability != relay_validation::Availability::Available
                    })))
        {
            return Err(error("INVALID_RUN_REPORT"));
        }
        match workflow.status {
            WorkflowStatus::Passed => counts.passed += 1,
            WorkflowStatus::Failed => counts.failed += 1,
            WorkflowStatus::Blocked => counts.blocked += 1,
            WorkflowStatus::Untested => counts.untested += 1,
        }
        *reasons.entry(workflow.reason_code.clone()).or_default() += 1;
        if let Some(resources) = &workflow.resource_use {
            peak_rss = max_option(peak_rss, resources.peak_rss_bytes);
            peak_gpu = max_option(peak_gpu, resources.peak_gpu_bytes);
            cpu_ms = sum_option(cpu_ms, resources.cpu_ms)?;
            io_read = sum_option(io_read, resources.io_read_bytes)?;
            io_write = sum_option(io_write, resources.io_write_bytes)?;
        }
    }
    if reasons.len() > MAX_OUTCOME_REASONS
        || report.counts.passed != counts.passed as usize
        || report.counts.failed != counts.failed as usize
        || report.counts.blocked != counts.blocked as usize
        || report.counts.untested != counts.untested as usize
    {
        return Err(error("INCONSISTENT_RUN_STATUS"));
    }
    let overall_status = match report.overall_status {
        WorkflowStatus::Passed => ValidationStatus::Passed,
        WorkflowStatus::Failed => ValidationStatus::Failed,
        WorkflowStatus::Blocked => ValidationStatus::Blocked,
        WorkflowStatus::Untested => ValidationStatus::Untested,
    };
    if !status_matches_counts(overall_status, &counts) {
        return Err(error("INCONSISTENT_RUN_STATUS"));
    }
    let outcome_reasons = reasons.into_iter().map(|(code, count)| CodeCount { code, count }).collect();
    let resources = if [peak_rss, cpu_ms, io_read, io_write, peak_gpu].iter().all(Option::is_none) {
        None
    } else {
        Some(RunResources {
            peak_rss_bytes: peak_rss,
            total_cpu_ms: cpu_ms,
            io_read_bytes: io_read,
            io_write_bytes: io_write,
            peak_gpu_bytes: peak_gpu,
        })
    };
    Ok(IntegratedRunSummary {
        report_schema_version: report.schema_version,
        run_ref: report.run_id,
        completed_unix_ms: report.completed_unix_ms,
        duration_ms: report.duration_ms,
        overall_status,
        counts,
        resources,
        outcome_reasons,
    })
}

fn max_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn sum_option(left: Option<u64>, right: Option<u64>) -> Result<Option<u64>, BundleError> {
    match (left, right) {
        (Some(a), Some(b)) => a.checked_add(b).map(Some).ok_or_else(|| error("RUN_RESOURCE_OVERFLOW")),
        (Some(value), None) | (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn validate_input(input: &SupportInput) -> Result<(), BundleError> {
    if !safe_version(&input.relay_version)
        || input.component_versions.len() > MAX_COMPONENT_VERSIONS
        || input.diagnostic_counts.by_code.len() > MAX_DIAGNOSTIC_CODES
    {
        return Err(error("INVALID_BUNDLE_METADATA"));
    }
    let mut components = BTreeSet::new();
    for entry in &input.component_versions {
        if !safe_component(&entry.component_id)
            || !safe_version(&entry.version)
            || !components.insert(&entry.component_id)
        {
            return Err(error("INVALID_COMPONENT_VERSION"));
        }
    }
    if input
        .diagnostic_health
        .last_error_code
        .as_ref()
        .is_some_and(|code| !safe_code(code))
    {
        return Err(error("INVALID_HEALTH_CODE"));
    }
    let counts = &input.diagnostic_counts;
    if counts.incomplete_events > counts.total_events
        || counts.sampled_events > counts.total_events
        || counts.untrusted_source_events > counts.total_events
    {
        return Err(error("INVALID_DIAGNOSTIC_COUNTS"));
    }
    validate_code_counts(&counts.by_code, counts.total_events)?;
    if let Some(run) = &input.integrated_run {
        if run.report_schema_version == 0
            || !safe_ref(&run.run_ref)
            || run.outcome_reasons.len() > MAX_OUTCOME_REASONS
        {
            return Err(error("INVALID_RUN_SUMMARY"));
        }
        let total = u64::from(run.counts.passed)
            + u64::from(run.counts.failed)
            + u64::from(run.counts.blocked)
            + u64::from(run.counts.untested);
        if total == 0 || !status_matches_counts(run.overall_status, &run.counts) {
            return Err(error("INCONSISTENT_RUN_STATUS"));
        }
        validate_code_counts(&run.outcome_reasons, total)?;
    }
    Ok(())
}

fn status_matches_counts(status: ValidationStatus, counts: &ValidationCounts) -> bool {
    let expected = if counts.failed > 0 {
        ValidationStatus::Failed
    } else if counts.blocked > 0 {
        ValidationStatus::Blocked
    } else if counts.untested > 0 {
        ValidationStatus::Untested
    } else {
        ValidationStatus::Passed
    };
    status == expected
}

fn validate_code_counts(entries: &[CodeCount], max_total: u64) -> Result<(), BundleError> {
    let mut seen = BTreeSet::new();
    let mut sum = 0u64;
    for entry in entries {
        if !safe_code(&entry.code) || !seen.insert(&entry.code) {
            return Err(error("INVALID_CODE_COUNT"));
        }
        sum = sum
            .checked_add(entry.count)
            .ok_or_else(|| error("CODE_COUNT_OVERFLOW"))?;
    }
    if sum > max_total {
        return Err(error("CODE_COUNT_EXCEEDS_TOTAL"));
    }
    Ok(())
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && (value.as_bytes()[0].is_ascii_digit()
            || (value.as_bytes()[0] == b'v'
                && value.as_bytes().get(1).is_some_and(u8::is_ascii_digit)))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'+'))
}

fn safe_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn safe_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SupportInput {
        SupportInput {
            generated_unix_ms: 500,
            relay_version: "0.1.0".into(),
            component_versions: vec![ComponentVersion {
                component_id: "relayd".into(),
                version: "0.1.0".into(),
            }],
            diagnostic_health: DiagnosticHealthSummary {
                healthy: true,
                current_bytes: 1024,
                rotated_files: 1,
                evicted_events: 0,
                evicted_files: 0,
                recovered_partial_bytes: 0,
                last_sync_unix_ms: Some(450),
                detail_active: false,
                last_error_code: None,
            },
            diagnostic_counts: DiagnosticCounts {
                total_events: 2,
                invalid_lines: 0,
                incomplete_events: 0,
                sampled_events: 0,
                untrusted_source_events: 0,
                retention_evicted_events: 0,
                by_code: vec![CodeCount {
                    code: "DAEMON_READY".into(),
                    count: 2,
                }],
            },
            integrated_run: None,
        }
    }

    #[test]
    fn bundle_is_summary_only_when_final_run_is_absent() {
        let bytes = bundle_json(sample()).unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.contains("\"integrated_run\":null"));
        assert!(json.contains("\"includes_raw_logs\":false"));
        assert!(json.contains("\"includes_local_paths\":false"));
        assert!(json.contains("\"includes_command_arguments\":false"));
        assert!(json.len() <= MAX_BUNDLE_BYTES);
    }

    #[test]
    fn rejects_private_paths_and_raw_text_as_codes() {
        let mut input = sample();
        input.component_versions[0].version = "C:\\Users\\someone\\secret".into();
        assert_eq!(
            bundle_json(input).unwrap_err().code,
            "INVALID_COMPONENT_VERSION"
        );

        let mut input = sample();
        input.diagnostic_health.last_error_code = Some("failed to open C:\\Users\\someone".into());
        assert_eq!(bundle_json(input).unwrap_err().code, "INVALID_HEALTH_CODE");
    }

    #[test]
    fn rejects_false_green_integrated_summary() {
        let mut input = sample();
        input.integrated_run = Some(IntegratedRunSummary {
            report_schema_version: 1,
            run_ref: "RUN-1".into(),
            completed_unix_ms: 490,
            duration_ms: 100,
            overall_status: ValidationStatus::Passed,
            counts: ValidationCounts {
                passed: 1,
                failed: 0,
                blocked: 0,
                untested: 1,
            },
            resources: None,
            outcome_reasons: vec![],
        });
        assert_eq!(
            bundle_json(input).unwrap_err().code,
            "INCONSISTENT_RUN_STATUS"
        );
    }

    #[test]
    fn extracts_only_consistent_private_safe_run_totals() {
        let report = serde_json::json!({
            "schema_version": 1, "run_id": "RUN-42", "relay_version": "0.1.0",
            "started_unix_ms": 100, "completed_unix_ms": 150, "duration_ms": 50,
            "overall_status": "untested",
            "counts": {"passed": 1, "failed": 0, "blocked": 0, "untested": 1},
            "workflows": [
                {"workflow_id":"host.health", "phase":2, "status":"passed", "reason_code":"HEALTHY",
                 "requirements":[], "evidence":{"kind":"observed", "source_id":"runner", "log_ref":"sha256:abc"},
                 "diagnostic_codes":[], "component_versions":[], "timing":null,
                 "resource_use":{"peak_rss_bytes":1024,"cpu_ms":10,"io_read_bytes":null,"io_write_bytes":null,"peak_gpu_bytes":null},
                 "reproduction":null},
                {"workflow_id":"uefn.runtime", "phase":7, "status":"untested", "reason_code":"REQUIRED_ENVIRONMENT_UNAVAILABLE",
                 "requirements":[{"requirement":{"kind":"uefn"},"availability":"unavailable"}],
                 "evidence":{"kind":"none"}, "diagnostic_codes":[], "component_versions":[],
                 "timing":null, "resource_use":null, "reproduction":null}
            ]
        });
        let input = serde_json::to_vec(&report).unwrap();
        let summary = summarize_integrated_report(&input).unwrap();
        assert_eq!(summary.counts.untested, 1);
        assert_eq!(summary.resources.unwrap().peak_rss_bytes, Some(1024));
        assert_eq!(summary.outcome_reasons[1].code, "REQUIRED_ENVIRONMENT_UNAVAILABLE");
        let mut false_green = report;
        false_green["overall_status"] = serde_json::json!("passed");
        assert_eq!(summarize_integrated_report(&serde_json::to_vec(&false_green).unwrap()).unwrap_err().code,
            "INCONSISTENT_RUN_STATUS");
    }
}
