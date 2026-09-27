//! Deterministic, privacy-bounded report assembly for one integrated run.
//!
//! The caller is responsible for executing workflows and verifying that an
//! `Observed` evidence reference genuinely came from the declared environment.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const REPORT_SCHEMA_VERSION: u32 = 1;
pub const MAX_WORKFLOWS: usize = 256;
pub const MAX_REQUIREMENTS: usize = 64;
pub const MAX_OBSERVATIONS: usize = 256;
pub const MAX_DIAGNOSTIC_CODES: usize = 32;
pub const MAX_VERSIONS: usize = 64;
pub const MAX_REPRO_STEPS: usize = 16;
pub const MAX_TRANSPORT_METRICS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextByteScope {
    SumOfStoredPayloadJsonVsCompiledResultJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenEstimateStatus {
    NotMeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnverifiedStatus {
    Untested,
}

/// One local comparison over already stored project results. Elapsed time is
/// the observed CLI command path and may include a warm context cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCostMetric {
    pub byte_scope: ContextByteScope,
    pub source_count: u8,
    pub full_payload_json_bytes: u64,
    pub compiled_context_json_bytes: u64,
    /// Integer thousandths of compiled bytes divided by summed source bytes.
    pub compiled_to_full_ratio_milli: u64,
    pub full_payload_elapsed_ms: u64,
    pub compiled_context_elapsed_ms: u64,
    pub token_estimate_status: TokenEstimateStatus,
    pub model_answer_quality_status: UnverifiedStatus,
    pub remote_cost_status: UnverifiedStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostTierDeclaration {
    MinimumCandidate,
    RecommendedCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForegroundObservation {
    Seen,
    NotSeen,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedMemoryClass {
    Below16Gib,
    AtLeast16Gib,
    AtLeast32Gib,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedCpuClass {
    BelowFourPhysicalCores,
    AtLeastFourPhysicalCores,
}

/// A bounded one-run host/index observation, not a hardware-tier budget pass
/// or paired creator-app interference measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResourceMetric {
    pub host_tier_declaration: HostTierDeclaration,
    pub support_tier_budget_status: UnverifiedStatus,
    pub physical_core_count: u16,
    pub logical_processor_count: u16,
    pub physical_memory_bytes: u64,
    pub observed_memory_class: ObservedMemoryClass,
    pub observed_cpu_class: ObservedCpuClass,
    pub index_command_elapsed_ms: u64,
    pub daemon_cpu_ms: u64,
    pub cli_cpu_ms: u64,
    pub daemon_peak_working_set_bytes: u64,
    pub cli_peak_working_set_bytes: u64,
    pub sample_count: u16,
    pub sample_interval_ms: u16,
    pub sampling_overhead_ms: u64,
    pub host_probe_overhead_ms: u64,
    pub foreground_probe_overhead_ms: u64,
    pub foreground_observation: ForegroundObservation,
    pub foreground_creator_samples: u16,
    pub foreground_interference_status: UnverifiedStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportError {
    pub code: &'static str,
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for ReportError {}

fn error(code: &'static str) -> ReportError {
    ReportError { code }
}

/// Required environment for a workflow. IDs are short opaque labels, not paths.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Requirement {
    Uefn,
    CreatorApp { app_id: String },
    HardwareClass { class_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentEntry {
    pub requirement: Requirement,
    pub availability: Availability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub workflow_id: String,
    pub phase: u8,
    #[serde(default)]
    pub requires: Vec<Requirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlan {
    pub run_id: String,
    pub relay_version: String,
    pub started_unix_ms: u64,
    pub workflows: Vec<WorkflowPlan>,
    #[serde(default)]
    pub environment: Vec<EnvironmentEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Passed,
    Failed,
    Blocked,
    Untested,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceSource {
    /// A trusted runner observed the declared workflow in its required environment.
    Observed {
        source_id: String,
        log_ref: String,
    },
    Synthetic {
        fixture_ref: String,
    },
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentVersion {
    pub component_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timing {
    pub started_unix_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUse {
    pub peak_rss_bytes: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub io_read_bytes: Option<u64>,
    pub io_write_bytes: Option<u64>,
    pub peak_gpu_bytes: Option<u64>,
}

/// Comparable local application-JSON exchange measurements. These are not
/// model-token or remote-network cost estimates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportMetric {
    pub path_id: String,
    pub byte_scope: String,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reproduction {
    /// Registered command ID, not a shell command or arbitrary text.
    pub command_id: String,
    /// Opaque fixture or scenario ID; never a local path.
    pub scenario_ref: String,
    /// Ordered, registered step codes; no raw arguments or secrets.
    pub step_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowObservation {
    pub workflow_id: String,
    pub status: WorkflowStatus,
    /// Stable diagnostic category. Free-form error messages are not accepted.
    pub reason_code: String,
    pub evidence: EvidenceSource,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
    #[serde(default)]
    pub component_versions: Vec<ComponentVersion>,
    pub timing: Option<Timing>,
    pub resource_use: Option<ResourceUse>,
    pub reproduction: Option<Reproduction>,
    #[serde(default)]
    pub transport_metrics: Vec<TransportMetric>,
    #[serde(default)]
    pub context_cost_metric: Option<ContextCostMetric>,
    #[serde(default)]
    pub host_resource_metric: Option<HostResourceMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementState {
    pub requirement: Requirement,
    pub availability: Availability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_id: String,
    pub phase: u8,
    pub status: WorkflowStatus,
    pub reason_code: String,
    pub requirements: Vec<RequirementState>,
    pub evidence: EvidenceSource,
    pub diagnostic_codes: Vec<String>,
    pub component_versions: Vec<ComponentVersion>,
    pub timing: Option<Timing>,
    pub resource_use: Option<ResourceUse>,
    pub reproduction: Option<Reproduction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transport_metrics: Vec<TransportMetric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_cost_metric: Option<ContextCostMetric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_resource_metric: Option<HostResourceMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeCounts {
    pub passed: usize,
    pub failed: usize,
    pub blocked: usize,
    pub untested: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedReport {
    pub schema_version: u32,
    pub run_id: String,
    pub relay_version: String,
    pub started_unix_ms: u64,
    pub completed_unix_ms: u64,
    pub duration_ms: u64,
    pub overall_status: WorkflowStatus,
    pub counts: OutcomeCounts,
    pub workflows: Vec<WorkflowResult>,
}

/// Assemble the final run report. Plans are sorted by phase and workflow ID.
/// No test executes here; all observations come from the caller.
pub fn assemble_report(
    plan: RunPlan,
    observations: Vec<WorkflowObservation>,
    completed_unix_ms: u64,
) -> Result<IntegratedReport, ReportError> {
    validate_plan(&plan)?;
    if completed_unix_ms < plan.started_unix_ms {
        return Err(error("INVALID_RUN_TIMING"));
    }
    if observations.len() > MAX_OBSERVATIONS {
        return Err(error("TOO_MANY_OBSERVATIONS"));
    }

    let workflow_ids: BTreeSet<_> = plan
        .workflows
        .iter()
        .map(|workflow| workflow.workflow_id.as_str())
        .collect();
    let mut observation_by_id = BTreeMap::new();
    for observation in observations {
        validate_observation(&observation)?;
        if !workflow_ids.contains(observation.workflow_id.as_str()) {
            return Err(error("UNKNOWN_WORKFLOW_OBSERVATION"));
        }
        if observation_by_id
            .insert(observation.workflow_id.clone(), observation)
            .is_some()
        {
            return Err(error("DUPLICATE_WORKFLOW_OBSERVATION"));
        }
    }

    let environment: BTreeMap<_, _> = plan
        .environment
        .iter()
        .map(|entry| (entry.requirement.clone(), entry.availability))
        .collect();
    let mut workflows = plan.workflows;
    workflows.sort_by(|left, right| {
        left.phase
            .cmp(&right.phase)
            .then_with(|| left.workflow_id.cmp(&right.workflow_id))
    });
    let results: Vec<_> = workflows
        .into_iter()
        .map(|workflow| {
            let requirements: Vec<_> = workflow
                .requires
                .iter()
                .map(|requirement| RequirementState {
                    requirement: requirement.clone(),
                    availability: environment
                        .get(requirement)
                        .copied()
                        .unwrap_or(Availability::Unknown),
                })
                .collect();
            let observation = observation_by_id.remove(&workflow.workflow_id);
            classify_workflow(workflow, requirements, observation)
        })
        .collect();

    let mut counts = OutcomeCounts {
        passed: 0,
        failed: 0,
        blocked: 0,
        untested: 0,
    };
    for result in &results {
        match result.status {
            WorkflowStatus::Passed => counts.passed += 1,
            WorkflowStatus::Failed => counts.failed += 1,
            WorkflowStatus::Blocked => counts.blocked += 1,
            WorkflowStatus::Untested => counts.untested += 1,
        }
    }
    let overall_status = if counts.failed > 0 {
        WorkflowStatus::Failed
    } else if counts.blocked > 0 {
        WorkflowStatus::Blocked
    } else if counts.untested > 0 {
        WorkflowStatus::Untested
    } else {
        WorkflowStatus::Passed
    };

    Ok(IntegratedReport {
        schema_version: REPORT_SCHEMA_VERSION,
        run_id: plan.run_id,
        relay_version: plan.relay_version,
        started_unix_ms: plan.started_unix_ms,
        completed_unix_ms,
        duration_ms: completed_unix_ms - plan.started_unix_ms,
        overall_status,
        counts,
        workflows: results,
    })
}

fn classify_workflow(
    workflow: WorkflowPlan,
    requirements: Vec<RequirementState>,
    observation: Option<WorkflowObservation>,
) -> WorkflowResult {
    let unavailable = requirements
        .iter()
        .any(|entry| entry.availability == Availability::Unavailable);
    let unknown = requirements
        .iter()
        .any(|entry| entry.availability == Availability::Unknown);

    let (status, reason_code) = if observation
        .as_ref()
        .is_some_and(|observation| observation.status == WorkflowStatus::Failed)
    {
        let observation = observation.as_ref().expect("checked above");
        (WorkflowStatus::Failed, observation.reason_code.clone())
    } else if unavailable {
        (
            WorkflowStatus::Untested,
            "REQUIRED_ENVIRONMENT_UNAVAILABLE".to_string(),
        )
    } else if unknown {
        (
            WorkflowStatus::Untested,
            "REQUIRED_ENVIRONMENT_UNKNOWN".to_string(),
        )
    } else if let Some(observation) = observation.as_ref() {
        if observation.status == WorkflowStatus::Passed
            && !matches!(observation.evidence, EvidenceSource::Observed { .. })
        {
            (
                WorkflowStatus::Untested,
                "OBSERVED_EVIDENCE_REQUIRED".to_string(),
            )
        } else {
            (observation.status, observation.reason_code.clone())
        }
    } else {
        (WorkflowStatus::Untested, "NOT_EXECUTED".to_string())
    };

    let (
        evidence,
        diagnostic_codes,
        component_versions,
        timing,
        resource_use,
        reproduction,
        transport_metrics,
        context_cost_metric,
        host_resource_metric,
    ) = if let Some(observation) = observation {
        (
            observation.evidence,
            observation.diagnostic_codes,
            observation.component_versions,
            observation.timing,
            observation.resource_use,
            observation.reproduction,
            observation.transport_metrics,
            observation.context_cost_metric,
            observation.host_resource_metric,
        )
    } else {
        (
            EvidenceSource::None,
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
            Vec::new(),
            None,
            None,
        )
    };
    WorkflowResult {
        workflow_id: workflow.workflow_id,
        phase: workflow.phase,
        status,
        reason_code,
        requirements,
        evidence,
        diagnostic_codes,
        component_versions,
        timing,
        resource_use,
        reproduction,
        transport_metrics,
        context_cost_metric,
        host_resource_metric,
    }
}

fn validate_plan(plan: &RunPlan) -> Result<(), ReportError> {
    if !safe_token(&plan.run_id, 96) || !safe_token(&plan.relay_version, 64) {
        return Err(error("INVALID_RUN_IDENTITY"));
    }
    if plan.workflows.is_empty() || plan.workflows.len() > MAX_WORKFLOWS {
        return Err(error("INVALID_WORKFLOW_COUNT"));
    }
    if plan.environment.len() > MAX_REQUIREMENTS {
        return Err(error("TOO_MANY_ENVIRONMENTS"));
    }
    let mut ids = BTreeSet::new();
    for workflow in &plan.workflows {
        if !safe_token(&workflow.workflow_id, 96) || workflow.phase > 12 {
            return Err(error("INVALID_WORKFLOW_PLAN"));
        }
        if !ids.insert(&workflow.workflow_id) {
            return Err(error("DUPLICATE_WORKFLOW_PLAN"));
        }
        if workflow.requires.len() > MAX_REQUIREMENTS {
            return Err(error("TOO_MANY_WORKFLOW_REQUIREMENTS"));
        }
        let mut requires = BTreeSet::new();
        for requirement in &workflow.requires {
            validate_requirement(requirement)?;
            if !requires.insert(requirement) {
                return Err(error("DUPLICATE_WORKFLOW_REQUIREMENT"));
            }
        }
    }
    let mut environment = BTreeSet::new();
    for entry in &plan.environment {
        validate_requirement(&entry.requirement)?;
        if !environment.insert(&entry.requirement) {
            return Err(error("DUPLICATE_ENVIRONMENT_ENTRY"));
        }
    }
    Ok(())
}

fn validate_requirement(requirement: &Requirement) -> Result<(), ReportError> {
    match requirement {
        Requirement::Uefn => Ok(()),
        Requirement::CreatorApp { app_id } if safe_token(app_id, 64) => Ok(()),
        Requirement::HardwareClass { class_id } if safe_token(class_id, 64) => Ok(()),
        _ => Err(error("INVALID_ENVIRONMENT_REQUIREMENT")),
    }
}

fn validate_observation(observation: &WorkflowObservation) -> Result<(), ReportError> {
    if !safe_token(&observation.workflow_id, 96)
        || !safe_token(&observation.reason_code, 96)
        || observation.diagnostic_codes.len() > MAX_DIAGNOSTIC_CODES
        || observation.component_versions.len() > MAX_VERSIONS
        || observation.transport_metrics.len() > MAX_TRANSPORT_METRICS
        || observation
            .diagnostic_codes
            .iter()
            .any(|code| !safe_token(code, 96))
        || observation.component_versions.iter().any(|version| {
            !safe_token(&version.component_id, 64) || !safe_token(&version.version, 64)
        })
    {
        return Err(error("INVALID_OBSERVATION_METADATA"));
    }
    let mut metric_paths = BTreeSet::new();
    for metric in &observation.transport_metrics {
        if !matches!(metric.path_id.as_str(), "cli_stdin" | "local_mcp_http")
            || metric.byte_scope != "application_json"
            || metric.request_bytes > 1_048_576
            || metric.response_bytes > 1_048_576
            || metric.elapsed_ms > 600_000
            || !metric_paths.insert(metric.path_id.as_str())
        {
            return Err(error("INVALID_TRANSPORT_METRIC"));
        }
    }
    if let Some(metric) = &observation.context_cost_metric {
        if observation.workflow_id != "context.cost_benchmark"
            || !(1..=8).contains(&metric.source_count)
            || metric.full_payload_json_bytes == 0
            || metric.compiled_context_json_bytes == 0
            || metric.full_payload_json_bytes > 1_048_576
            || metric.compiled_context_json_bytes > 1_048_576
            || metric.compiled_to_full_ratio_milli
                != metric.compiled_context_json_bytes.saturating_mul(1000)
                    / metric.full_payload_json_bytes
            || metric.full_payload_elapsed_ms > 600_000
            || metric.compiled_context_elapsed_ms > 600_000
        {
            return Err(error("INVALID_CONTEXT_COST_METRIC"));
        }
    }
    if let Some(metric) = &observation.host_resource_metric {
        if observation.workflow_id != "project.supported_host_resource"
            || metric.physical_core_count == 0
            || metric.physical_core_count > 512
            || metric.logical_processor_count < metric.physical_core_count
            || metric.logical_processor_count > 1024
            || !(1_073_741_824..=4_398_046_511_104).contains(&metric.physical_memory_bytes)
            || metric.index_command_elapsed_ms > 600_000
            || metric.daemon_cpu_ms > 614_400_000
            || metric.cli_cpu_ms > 614_400_000
            || metric.daemon_peak_working_set_bytes == 0
            || metric.daemon_peak_working_set_bytes > 1_099_511_627_776
            || metric.cli_peak_working_set_bytes == 0
            || metric.cli_peak_working_set_bytes > 1_099_511_627_776
            || !(2..=6002).contains(&metric.sample_count)
            || metric.sample_interval_ms != 100
            || metric.sampling_overhead_ms > 600_000
            || metric.host_probe_overhead_ms > 600_000
            || metric.foreground_probe_overhead_ms > 600_000
            || metric.foreground_creator_samples > metric.sample_count
            || (metric.foreground_observation == ForegroundObservation::Unavailable
                && metric.foreground_creator_samples != 0)
            || metric.observed_memory_class
                != if metric.physical_memory_bytes >= 32 * 1_073_741_824 {
                    ObservedMemoryClass::AtLeast32Gib
                } else if metric.physical_memory_bytes >= 16 * 1_073_741_824 {
                    ObservedMemoryClass::AtLeast16Gib
                } else {
                    ObservedMemoryClass::Below16Gib
                }
            || metric.observed_cpu_class
                != if metric.physical_core_count >= 4 {
                    ObservedCpuClass::AtLeastFourPhysicalCores
                } else {
                    ObservedCpuClass::BelowFourPhysicalCores
                }
        {
            return Err(error("INVALID_HOST_RESOURCE_METRIC"));
        }
    }
    match &observation.evidence {
        EvidenceSource::Observed { source_id, log_ref } => {
            if !safe_token(source_id, 64) || !safe_token(log_ref, 96) {
                return Err(error("INVALID_EVIDENCE_REFERENCE"));
            }
        }
        EvidenceSource::Synthetic { fixture_ref } => {
            if !safe_token(fixture_ref, 96) {
                return Err(error("INVALID_EVIDENCE_REFERENCE"));
            }
        }
        EvidenceSource::None => {}
    }
    if let Some(reproduction) = &observation.reproduction {
        if !safe_token(&reproduction.command_id, 96)
            || !safe_token(&reproduction.scenario_ref, 96)
            || reproduction.step_codes.len() > MAX_REPRO_STEPS
            || reproduction
                .step_codes
                .iter()
                .any(|code| !safe_token(code, 96))
        {
            return Err(error("INVALID_REPRODUCTION_REFERENCE"));
        }
    }
    Ok(())
}

fn safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(requirement: Requirement, availability: Availability) -> RunPlan {
        RunPlan {
            run_id: "RUN-1".into(),
            relay_version: "0.1.0".into(),
            started_unix_ms: 100,
            workflows: vec![WorkflowPlan {
                workflow_id: "uefn.runtime".into(),
                phase: 7,
                requires: vec![requirement.clone()],
            }],
            environment: vec![EnvironmentEntry {
                requirement,
                availability,
            }],
        }
    }

    fn observation(evidence: EvidenceSource) -> WorkflowObservation {
        WorkflowObservation {
            workflow_id: "uefn.runtime".into(),
            status: WorkflowStatus::Passed,
            reason_code: "ASSERTIONS_PASSED".into(),
            evidence,
            diagnostic_codes: vec!["SESSION_CAPTURED".into()],
            component_versions: vec![ComponentVersion {
                component_id: "uefn".into(),
                version: "42.00".into(),
            }],
            timing: Some(Timing {
                started_unix_ms: 101,
                duration_ms: 20,
            }),
            resource_use: Some(ResourceUse {
                peak_rss_bytes: Some(1024),
                cpu_ms: Some(10),
                io_read_bytes: None,
                io_write_bytes: None,
                peak_gpu_bytes: None,
            }),
            reproduction: Some(Reproduction {
                command_id: "relay.audit".into(),
                scenario_ref: "SCENARIO-1".into(),
                step_codes: vec!["OPEN_SESSION".into(), "RUN_ASSERTION".into()],
            }),
            transport_metrics: Vec::new(),
            context_cost_metric: None,
            host_resource_metric: None,
        }
    }

    #[test]
    fn unavailable_creator_app_and_hardware_stay_untested() {
        for requirement in [
            Requirement::CreatorApp {
                app_id: "blender".into(),
            },
            Requirement::HardwareClass {
                class_id: "minimum_pc".into(),
            },
        ] {
            let report = assemble_report(
                plan(requirement, Availability::Unavailable),
                vec![observation(EvidenceSource::Observed {
                    source_id: "runner".into(),
                    log_ref: "LOG-1".into(),
                })],
                200,
            )
            .unwrap();
            assert_eq!(report.overall_status, WorkflowStatus::Untested);
            assert_eq!(
                report.workflows[0].reason_code,
                "REQUIRED_ENVIRONMENT_UNAVAILABLE"
            );
        }
    }

    #[test]
    fn synthetic_uefn_success_cannot_pass_integrated_run() {
        let report = assemble_report(
            plan(Requirement::Uefn, Availability::Available),
            vec![observation(EvidenceSource::Synthetic {
                fixture_ref: "FIXTURE-1".into(),
            })],
            200,
        )
        .unwrap();
        assert_eq!(report.workflows[0].status, WorkflowStatus::Untested);
        assert_eq!(
            report.workflows[0].reason_code,
            "OBSERVED_EVIDENCE_REQUIRED"
        );
    }

    #[test]
    fn attempted_failure_is_not_hidden_by_later_unavailable_environment() {
        let mut failed = observation(EvidenceSource::None);
        failed.status = WorkflowStatus::Failed;
        failed.reason_code = "RUNNER_INTERRUPTED".into();
        let report = assemble_report(
            plan(Requirement::Uefn, Availability::Unavailable),
            vec![failed],
            200,
        )
        .unwrap();
        assert_eq!(report.workflows[0].status, WorkflowStatus::Failed);
        assert_eq!(report.workflows[0].reason_code, "RUNNER_INTERRUPTED");
    }

    #[test]
    fn bounded_local_transport_metrics_remain_numeric_and_typed() {
        let mut measured = observation(EvidenceSource::Observed {
            source_id: "runner".into(),
            log_ref: "LOG-1".into(),
        });
        measured.transport_metrics = vec![TransportMetric {
            path_id: "local_mcp_http".into(),
            byte_scope: "application_json".into(),
            request_bytes: 120,
            response_bytes: 240,
            elapsed_ms: 12,
        }];
        let report = assemble_report(
            plan(Requirement::Uefn, Availability::Available),
            vec![measured.clone()],
            200,
        )
        .unwrap();
        assert_eq!(report.workflows[0].transport_metrics[0].response_bytes, 240);
        measured.transport_metrics[0].path_id = "remote_secret_path".into();
        assert_eq!(
            assemble_report(
                plan(Requirement::Uefn, Availability::Available),
                vec![measured],
                200,
            )
            .unwrap_err()
            .code,
            "INVALID_TRANSPORT_METRIC"
        );
    }

    #[test]
    fn context_cost_measurement_preserves_regression_without_token_claim() {
        let mut measured = observation(EvidenceSource::Observed {
            source_id: "runner".into(),
            log_ref: "LOG-1".into(),
        });
        measured.workflow_id = "context.cost_benchmark".into();
        measured.context_cost_metric = Some(ContextCostMetric {
            byte_scope: ContextByteScope::SumOfStoredPayloadJsonVsCompiledResultJson,
            source_count: 2,
            full_payload_json_bytes: 100,
            compiled_context_json_bytes: 140,
            compiled_to_full_ratio_milli: 1400,
            full_payload_elapsed_ms: 20,
            compiled_context_elapsed_ms: 30,
            token_estimate_status: TokenEstimateStatus::NotMeasured,
            model_answer_quality_status: UnverifiedStatus::Untested,
            remote_cost_status: UnverifiedStatus::Untested,
        });
        let mut run_plan = plan(Requirement::Uefn, Availability::Available);
        run_plan.workflows[0].workflow_id = "context.cost_benchmark".into();
        let report = assemble_report(run_plan.clone(), vec![measured.clone()], 200).unwrap();
        assert_eq!(report.workflows[0].status, WorkflowStatus::Passed);
        let metric = report.workflows[0].context_cost_metric.as_ref().unwrap();
        assert_eq!(metric.compiled_to_full_ratio_milli, 1400);
        assert_eq!(metric.full_payload_json_bytes, 100);

        measured
            .context_cost_metric
            .as_mut()
            .unwrap()
            .compiled_to_full_ratio_milli = 1;
        assert_eq!(
            assemble_report(run_plan, vec![measured], 200)
                .unwrap_err()
                .code,
            "INVALID_CONTEXT_COST_METRIC"
        );
    }

    #[test]
    fn host_resource_measurement_is_bounded_and_keeps_budget_untested() {
        let mut measured = observation(EvidenceSource::Observed {
            source_id: "runner".into(),
            log_ref: "LOG-1".into(),
        });
        measured.workflow_id = "project.supported_host_resource".into();
        measured.host_resource_metric = Some(HostResourceMetric {
            host_tier_declaration: HostTierDeclaration::MinimumCandidate,
            support_tier_budget_status: UnverifiedStatus::Untested,
            physical_core_count: 4,
            logical_processor_count: 8,
            physical_memory_bytes: 17_179_869_184,
            observed_memory_class: ObservedMemoryClass::AtLeast16Gib,
            observed_cpu_class: ObservedCpuClass::AtLeastFourPhysicalCores,
            index_command_elapsed_ms: 120,
            daemon_cpu_ms: 30,
            cli_cpu_ms: 10,
            daemon_peak_working_set_bytes: 12_000_000,
            cli_peak_working_set_bytes: 6_000_000,
            sample_count: 3,
            sample_interval_ms: 100,
            sampling_overhead_ms: 2,
            host_probe_overhead_ms: 12,
            foreground_probe_overhead_ms: 3,
            foreground_observation: ForegroundObservation::NotSeen,
            foreground_creator_samples: 0,
            foreground_interference_status: UnverifiedStatus::Untested,
        });
        let mut run_plan = plan(Requirement::Uefn, Availability::Available);
        run_plan.workflows[0].workflow_id = measured.workflow_id.clone();
        let report = assemble_report(run_plan.clone(), vec![measured.clone()], 200).unwrap();
        assert_eq!(report.workflows[0].status, WorkflowStatus::Passed);
        assert_eq!(
            report.workflows[0]
                .host_resource_metric
                .as_ref()
                .unwrap()
                .foreground_interference_status,
            UnverifiedStatus::Untested
        );

        measured
            .host_resource_metric
            .as_mut()
            .unwrap()
            .observed_memory_class = ObservedMemoryClass::AtLeast32Gib;
        assert_eq!(
            assemble_report(run_plan, vec![measured], 200)
                .unwrap_err()
                .code,
            "INVALID_HOST_RESOURCE_METRIC"
        );
    }

    #[test]
    fn observed_failure_keeps_bounded_reproduction_and_rejects_paths() {
        let mut captured = observation(EvidenceSource::Observed {
            source_id: "runner".into(),
            log_ref: "LOG-1".into(),
        });
        captured.status = WorkflowStatus::Failed;
        captured.reason_code = "ASSERTION_FAILED".into();
        let report = assemble_report(
            plan(Requirement::Uefn, Availability::Available),
            vec![captured.clone()],
            200,
        )
        .unwrap();
        assert_eq!(report.overall_status, WorkflowStatus::Failed);
        assert_eq!(report.workflows[0].component_versions[0].version, "42.00");
        assert_eq!(
            report.workflows[0].resource_use.as_ref().unwrap().cpu_ms,
            Some(10)
        );
        captured.reproduction.as_mut().unwrap().scenario_ref = "C:\\private\\island".into();
        assert_eq!(
            assemble_report(
                plan(Requirement::Uefn, Availability::Available),
                vec![captured],
                200,
            )
            .unwrap_err()
            .code,
            "INVALID_REPRODUCTION_REFERENCE"
        );
    }
}
