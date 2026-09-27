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

    let (evidence, diagnostic_codes, component_versions, timing, resource_use, reproduction) =
        if let Some(observation) = observation {
            (
                observation.evidence,
                observation.diagnostic_codes,
                observation.component_versions,
                observation.timing,
                observation.resource_use,
                observation.reproduction,
            )
        } else {
            (
                EvidenceSource::None,
                Vec::new(),
                Vec::new(),
                None,
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
