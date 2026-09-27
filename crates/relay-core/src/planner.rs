use crate::indexing;
use crate::storage::{DependencyCoverageRecord, PreparedCheckExecution, ProjectPlanSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_CHECKS: usize = 128;
const MAX_PATHS_PER_CHECK: usize = 64;
const MAX_CATALOG_BYTES: usize = 128 * 1024;

#[derive(Debug)]
pub struct PlannerError {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckCatalog {
    format_version: u32,
    checks: Vec<CheckDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckDefinition {
    id: String,
    roots: Vec<String>,
    leaves: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assertion: Option<CheckAssertion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependency_mode: Option<DependencyMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DependencyMode {
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CheckAssertion {
    IndexedFilePresent { path: String },
    IndexedFileDigest { path: String, sha256: String },
}

fn error(code: &'static str, message: &'static str) -> PlannerError {
    PlannerError { code, message }
}

fn decode_catalog(value: &Value) -> Result<CheckCatalog, PlannerError> {
    let mut catalog: CheckCatalog = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            "CHECK_CATALOG_INVALID",
            "check catalog does not match format 1",
        )
    })?;
    if catalog.format_version != 1 || catalog.checks.is_empty() || catalog.checks.len() > MAX_CHECKS
    {
        return Err(error(
            "CHECK_CATALOG_INVALID",
            "check catalog version or count is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for check in &mut catalog.checks {
        if check.id.is_empty()
            || check.id.len() > 64
            || !check
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || !ids.insert(check.id.clone())
            || check.roots.is_empty()
            || check.roots.len() > MAX_PATHS_PER_CHECK
            || check.leaves.len() > MAX_PATHS_PER_CHECK
        {
            return Err(error(
                "CHECK_CATALOG_INVALID",
                "check ID or path count is invalid",
            ));
        }
        let mut paths = BTreeSet::new();
        for path in check.roots.iter_mut().chain(check.leaves.iter_mut()) {
            let normalized = indexing::normalize_project_relative_path(path).map_err(|_| {
                error(
                    "CHECK_CATALOG_INVALID",
                    "check path must be project relative",
                )
            })?;
            if normalized.len() > 4096 || !paths.insert(normalized.clone()) {
                return Err(error(
                    "CHECK_CATALOG_INVALID",
                    "check paths must be distinct and bounded",
                ));
            }
            *path = normalized;
        }
        check.roots.sort();
        check.leaves.sort();
        if let Some(assertion) = &mut check.assertion {
            let path = match assertion {
                CheckAssertion::IndexedFilePresent { path } => path,
                CheckAssertion::IndexedFileDigest { path, sha256 } => {
                    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(error(
                            "CHECK_CATALOG_INVALID",
                            "assertion digest is invalid",
                        ));
                    }
                    *sha256 = sha256.to_ascii_lowercase();
                    path
                }
            };
            *path = indexing::normalize_project_relative_path(path).map_err(|_| {
                error(
                    "CHECK_CATALOG_INVALID",
                    "assertion path must be project relative",
                )
            })?;
            if !paths.contains(path) {
                return Err(error(
                    "CHECK_CATALOG_INVALID",
                    "assertion path must be declared",
                ));
            }
        }
    }
    catalog.checks.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(catalog)
}

pub fn evaluate_declared_checks(
    snapshot: &ProjectPlanSnapshot,
    planned: &Value,
) -> Result<Vec<PreparedCheckExecution>, PlannerError> {
    let catalog = decode_catalog(&snapshot.catalog.catalog)?;
    let definitions: BTreeMap<&str, &CheckDefinition> = catalog
        .checks
        .iter()
        .map(|check| (check.id.as_str(), check))
        .collect();
    let files: BTreeMap<&str, &str> = snapshot
        .files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.content_sha256.as_str()))
        .collect();
    let plan_id = planned["plan_id"]
        .as_str()
        .ok_or_else(|| error("CHECK_PLAN_CONFLICT", "planned check identity is missing"))?;
    let mut prepared = Vec::new();
    for entry in planned["checks"]
        .as_array()
        .ok_or_else(|| error("CHECK_PLAN_CONFLICT", "planned checks are missing"))?
    {
        let check_id = entry["check_id"]
            .as_str()
            .ok_or_else(|| error("CHECK_PLAN_CONFLICT", "planned check ID is missing"))?;
        let check = definitions
            .get(check_id)
            .ok_or_else(|| error("CHECK_PLAN_CONFLICT", "planned check is no longer declared"))?;
        let planned_check_id = entry["planned_check_id"]
            .as_str()
            .ok_or_else(|| error("CHECK_PLAN_CONFLICT", "planned check identity is missing"))?;
        let (status, reason_code, payload) = match &check.assertion {
            None => ("untested", "ASSERTION_NOT_DECLARED", None),
            Some(assertion) => {
                let (path, expected) = match assertion {
                    CheckAssertion::IndexedFilePresent { path } => (path, None),
                    CheckAssertion::IndexedFileDigest { path, sha256 } => (path, Some(sha256)),
                };
                let observed = files.get(path.as_str()).copied();
                let (status, reason_code) = match (observed, expected) {
                    (None, _) => ("failed", "INDEXED_FILE_MISSING"),
                    (Some(actual), Some(wanted)) if actual != wanted => {
                        ("failed", "INDEXED_DIGEST_MISMATCH")
                    }
                    _ => ("passed", "INDEX_ASSERTION_SATISFIED"),
                };
                let path_digest = Sha256::digest(path.as_bytes());
                let path_sha256: String = path_digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                let payload = json!({
                    "plan_id": plan_id,
                    "check_id": check_id,
                    "planned_check_id": planned_check_id,
                    "index_generation": snapshot.state.generation,
                    "assertion_kind": if expected.is_some() { "indexed_file_digest" } else { "indexed_file_present" },
                    "path_sha256": path_sha256,
                    "expected_sha256": expected,
                    "observed_sha256": observed,
                    "status": status,
                    "reason_code": reason_code,
                    "native_workflow_status": "untested"
                });
                (status, reason_code, Some(payload))
            }
        };
        prepared.push(PreparedCheckExecution {
            check_id: check_id.to_string(),
            planned_check_id: planned_check_id.to_string(),
            status,
            reason_code,
            result_payload: payload,
        });
    }
    Ok(prepared)
}

pub fn canonical_catalog(value: &Value) -> Result<Value, PlannerError> {
    let canonical = serde_json::to_value(decode_catalog(value)?).map_err(|_| {
        error(
            "CHECK_CATALOG_INVALID",
            "check catalog serialization failed",
        )
    })?;
    if serde_json::to_vec(&canonical).map_or(true, |bytes| bytes.len() > MAX_CATALOG_BYTES) {
        return Err(error(
            "CHECK_CATALOG_INVALID",
            "check catalog exceeds the byte limit",
        ));
    }
    Ok(canonical)
}

fn digest_id(prefix: &str, material: &Value) -> String {
    let bytes = serde_json::to_vec(material).expect("planner material is serializable");
    let digest = Sha256::digest(bytes);
    let mut id = String::from(prefix);
    id.push('-');
    for byte in digest.iter().take(16) {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    id
}

fn valid_coverage<'a>(
    path: &str,
    coverage: &'a BTreeMap<String, &'a DependencyCoverageRecord>,
    files: &BTreeMap<String, String>,
    producer_id: &str,
    producer_version: &str,
    configuration_revision: i64,
    edge_counts: &BTreeMap<String, usize>,
) -> bool {
    let Some(record) = coverage.get(path) else {
        return false;
    };
    record.producer_id == producer_id
        && record.producer_version == producer_version
        && record.configuration_revision == Some(configuration_revision)
        && files
            .get(path)
            .is_some_and(|sha| sha == &record.source_sha256)
        && edge_counts.get(path).copied().unwrap_or(0) == record.target_count as usize
}

pub fn plan(snapshot: ProjectPlanSnapshot, after_generation: i64) -> Result<Value, PlannerError> {
    let catalog = decode_catalog(&snapshot.catalog.catalog)?;
    let state = &snapshot.state;
    if after_generation > state.generation {
        return Err(error(
            "INDEX_GENERATION_CONFLICT",
            "requested generation is newer than the index",
        ));
    }
    let configuration = snapshot.configuration.as_ref();
    let configured = configuration.and_then(|config| {
        Some((
            config.revision,
            config.adapter_id.as_deref()?,
            config.adapter_version.as_deref()?,
        ))
    });
    let needs_parser = catalog
        .checks
        .iter()
        .any(|check| check.dependency_mode != Some(DependencyMode::Direct));
    let mut fallback_reason = if state.status != "ready" || state.content_verification_required {
        Some("index_not_ready")
    } else if after_generation < state.baseline_generation {
        Some("continuity_gap")
    } else if snapshot.catalog.index_generation > after_generation {
        Some("catalog_newer_than_delta")
    } else if snapshot.bounds_exceeded {
        Some("planning_bound_exceeded")
    } else if needs_parser
        && (configured.is_none()
            || snapshot.catalog.configuration_revision != configured.map(|(revision, _, _)| revision))
    {
        Some("parser_configuration_unavailable")
    } else {
        None
    };
    let changed: BTreeSet<String> = snapshot
        .changes
        .iter()
        .flat_map(|change| {
            [
                Some(change.relative_path.clone()),
                change.previous_path.clone(),
            ]
            .into_iter()
            .flatten()
        })
        .collect();
    // The first ready baseline is the initial direct input set. The v1 plan
    // reason remains `changed_input`, meaning changed from no prior index.
    let initial_baseline = state.generation == state.baseline_generation
        && after_generation == state.baseline_generation
        && snapshot.catalog.index_generation == state.generation;
    let mut affected = BTreeSet::new();
    if fallback_reason.is_none() {
        let (revision, producer_id, producer_version) = configured.unwrap_or((0, "", ""));
        let files: BTreeMap<String, String> = snapshot
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.content_sha256.clone()))
            .collect();
        let coverage: BTreeMap<String, &DependencyCoverageRecord> = snapshot
            .coverage
            .iter()
            .filter(|record| record.producer_id == producer_id)
            .map(|record| (record.source_path.clone(), record))
            .collect();
        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut edge_counts: BTreeMap<String, usize> = BTreeMap::new();
        for edge in &snapshot.edges {
            if edge.producer_id == producer_id && edge.producer_version == producer_version {
                graph
                    .entry(edge.source_path.clone())
                    .or_default()
                    .insert(edge.target_path.clone());
                *edge_counts.entry(edge.source_path.clone()).or_default() += 1;
            }
        }
        for edge in &snapshot.invalidations {
            if edge.producer_id == producer_id && edge.producer_version == producer_version {
                graph
                    .entry(edge.source_path.clone())
                    .or_default()
                    .insert(edge.target_path.clone());
            }
        }
        for check in &catalog.checks {
            if check.dependency_mode == Some(DependencyMode::Direct) {
                if initial_baseline
                    || check.roots.iter().chain(&check.leaves).any(|path| changed.contains(path))
                {
                    affected.insert(check.id.clone());
                }
                continue;
            }
            let leaves: BTreeSet<&str> = check.leaves.iter().map(String::as_str).collect();
            let mut visited = BTreeSet::new();
            let mut inputs: BTreeSet<String> = check.leaves.iter().cloned().collect();
            let mut queue: VecDeque<&str> = check.roots.iter().map(String::as_str).collect();
            while let Some(source) = queue.pop_front() {
                if !visited.insert(source.to_string()) {
                    continue;
                }
                inputs.insert(source.to_string());
                if !valid_coverage(
                    source,
                    &coverage,
                    &files,
                    producer_id,
                    producer_version,
                    revision,
                    &edge_counts,
                ) {
                    fallback_reason = Some("parser_coverage_incomplete");
                    break;
                }
                if let Some(targets) = graph.get(source) {
                    for target in targets {
                        inputs.insert(target.clone());
                        if !leaves.contains(target.as_str()) {
                            queue.push_back(target);
                        }
                    }
                }
            }
            if fallback_reason.is_some() {
                break;
            }
            if inputs.iter().any(|path| changed.contains(path)) {
                affected.insert(check.id.clone());
            }
        }
    }
    let mode = if fallback_reason.is_some() {
        "full_catalog_fallback"
    } else {
        "selective"
    };
    let selected: Vec<&CheckDefinition> = catalog
        .checks
        .iter()
        .filter(|check| fallback_reason.is_some() || affected.contains(&check.id))
        .collect();
    let selected_ids: Vec<&str> = selected.iter().map(|check| check.id.as_str()).collect();
    let plan_id = digest_id(
        "PLAN",
        &json!({
            "project_id": state.project_id,
            "catalog_revision": snapshot.catalog.revision,
            "catalog_sha256": snapshot.catalog.catalog_sha256,
            "after_generation": after_generation,
            "index_generation": state.generation,
            "configuration_revision": configured.map(|(revision, _, _)| revision),
            "mode": mode,
            "fallback_reason": fallback_reason,
            "selected_ids": selected_ids,
        }),
    );
    let checks: Vec<Value> = selected.into_iter().map(|check| json!({
        "check_id": check.id,
        "planned_check_id": digest_id("PCHK", &json!({"plan_id": plan_id, "check_id": check.id})),
        "status": "planned_not_run",
        "reason": if fallback_reason.is_some() { "coverage_fallback" } else { "changed_input" },
        "result_id": null,
    })).collect();
    Ok(json!({
        "project_id": state.project_id,
        "plan_id": plan_id,
        "catalog_revision": snapshot.catalog.revision,
        "index_generation": state.generation,
        "after_generation": after_generation,
        "mode": mode,
        "fallback_reason": fallback_reason,
        "coverage_basis": if needs_parser {
            "declared_catalog_and_guarded_parser_observations"
        } else {
            "declared_direct_paths"
        },
        "declared_check_count": catalog.checks.len(),
        "checks": checks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::IndexedFileSnapshot;
    use crate::storage::{
        DependencyEdgeRecord, ProjectChangeRecord, ProjectCheckCatalogRecord, ProjectConfiguration,
        ProjectIndexState,
    };

    fn snapshot(include_source_coverage: bool) -> ProjectPlanSnapshot {
        let catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [
                {"id":"check.a", "roots":["source.txt"], "leaves":["target.txt"]},
                {"id":"check.b", "roots":["other.txt"], "leaves":[]}
            ]
        }))
        .unwrap();
        let mut coverage = vec![DependencyCoverageRecord {
            project_id: "PRJ-a".into(),
            source_path: "other.txt".into(),
            source_sha256: "b".repeat(64),
            producer_id: "parser".into(),
            producer_version: "1".into(),
            indexed_generation: 1,
            configuration_revision: Some(1),
            target_count: 0,
        }];
        if include_source_coverage {
            coverage.push(DependencyCoverageRecord {
                project_id: "PRJ-a".into(),
                source_path: "source.txt".into(),
                source_sha256: "a".repeat(64),
                producer_id: "parser".into(),
                producer_version: "1".into(),
                indexed_generation: 2,
                configuration_revision: Some(1),
                target_count: 1,
            });
        }
        ProjectPlanSnapshot {
            state: ProjectIndexState {
                project_id: "PRJ-a".into(),
                generation: 2,
                baseline_generation: 1,
                status: "ready".into(),
                content_verification_required: false,
                file_count: 3,
                total_bytes: 3,
                last_reconciled_at: "fixture".into(),
                updated_at: "fixture".into(),
            },
            configuration: Some(ProjectConfiguration {
                project_id: "PRJ-a".into(),
                revision: 1,
                format_version: 1,
                project_type: "fixture".into(),
                adapter_id: Some("parser".into()),
                adapter_version: Some("1".into()),
                updated_at: "fixture".into(),
            }),
            catalog: ProjectCheckCatalogRecord {
                project_id: "PRJ-a".into(),
                revision: 1,
                index_generation: 1,
                configuration_revision: Some(1),
                catalog,
                catalog_sha256: "c".repeat(64),
            },
            files: [("source.txt", "a"), ("other.txt", "b"), ("target.txt", "d")]
                .into_iter()
                .map(|(path, hash)| IndexedFileSnapshot {
                    relative_path: path.into(),
                    size_bytes: 1,
                    modified_unix_ns: 1,
                    content_sha256: hash.repeat(64),
                })
                .collect(),
            changes: vec![ProjectChangeRecord {
                id: "CHG-a".into(),
                project_id: "PRJ-a".into(),
                generation: 2,
                change_kind: "modified".into(),
                relative_path: "target.txt".into(),
                previous_path: None,
                before_sha256: Some("c".repeat(64)),
                after_sha256: Some("d".repeat(64)),
                detected_at: "fixture".into(),
            }],
            edges: vec![DependencyEdgeRecord {
                project_id: "PRJ-a".into(),
                source_path: "source.txt".into(),
                target_path: "target.txt".into(),
                source_sha256: "a".repeat(64),
                producer_id: "parser".into(),
                producer_version: "1".into(),
                indexed_generation: 2,
            }],
            invalidations: vec![],
            coverage,
            bounds_exceeded: false,
        }
    }

    #[test]
    fn catalog_normalizes_and_rejects_duplicate_paths() {
        let canonical = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":[]}]
        }))
        .unwrap();
        assert_eq!(canonical["checks"][0]["id"], "check.a");
        assert!(
            canonical_catalog(&json!({
                "format_version": 1,
                "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":["src/a.txt"]}]
            }))
            .is_err()
        );
        assert!(canonical["checks"][0].get("assertion").is_none());
        assert!(canonical["checks"][0].get("dependency_mode").is_none());
        assert!(
            canonical_catalog(&json!({
                "format_version": 1,
                "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":[],
                    "assertion":{"kind":"indexed_file_present", "path":"other.txt"}}]
            }))
            .is_err()
        );
        assert!(
            canonical_catalog(&json!({
                "format_version": 1,
                "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":[],
                    "assertion":{"kind":"indexed_file_digest", "path":"src/a.txt", "sha256":"bad"}}]
            }))
            .is_err()
        );
        assert!(canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":[],
                "assertion":{"kind":"shell_command", "path":"src/a.txt", "command":"echo bad"}}]
        })).is_err());
        assert!(canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.a", "roots":["src/a.txt"], "leaves":[],
                "dependency_mode":"shell"}]
        })).is_err());
    }

    #[test]
    fn direct_mode_selects_declared_change_without_parser_configuration_or_coverage() {
        let mut basis = snapshot(false);
        basis.configuration = None;
        basis.coverage.clear();
        basis.catalog.configuration_revision = None;
        basis.catalog.catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.direct", "roots":["source.txt"], "leaves":["target.txt"],
                "dependency_mode":"direct",
                "assertion":{"kind":"indexed_file_digest", "path":"target.txt", "sha256":"d".repeat(64)}}]
        })).unwrap();
        let planned = plan(basis.clone(), 1).unwrap();
        assert_eq!(planned["mode"], "selective");
        assert_eq!(planned["coverage_basis"], "declared_direct_paths");
        assert_eq!(planned["checks"].as_array().unwrap().len(), 1);
        assert_eq!(planned["checks"][0]["check_id"], "check.direct");
        let evaluated = evaluate_declared_checks(&basis, &planned).unwrap();
        assert_eq!(evaluated[0].status, "passed");
    }

    #[test]
    fn direct_mode_selects_initial_baseline_without_delta_rows() {
        let mut basis = snapshot(false);
        basis.state.generation = 1;
        basis.state.baseline_generation = 1;
        basis.changes.clear();
        basis.configuration = None;
        basis.coverage.clear();
        basis.catalog.configuration_revision = None;
        basis.catalog.catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.first", "roots":["source.txt"], "leaves":[],
                "dependency_mode":"direct",
                "assertion":{"kind":"indexed_file_present", "path":"source.txt"}}]
        })).unwrap();
        let direct = plan(basis.clone(), 1).unwrap();
        assert_eq!(direct["mode"], "selective");
        assert_eq!(direct["checks"][0]["check_id"], "check.first");
        assert_eq!(evaluate_declared_checks(&basis, &direct).unwrap()[0].status, "passed");

        basis.catalog.catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.old", "roots":["source.txt"], "leaves":[]}]
        })).unwrap();
        let legacy = plan(basis, 1).unwrap();
        assert_eq!(legacy["mode"], "full_catalog_fallback");
        assert_eq!(legacy["fallback_reason"], "parser_configuration_unavailable");
    }

    #[test]
    fn mixed_direct_and_legacy_checks_fall_back_when_parser_basis_is_missing() {
        let mut basis = snapshot(false);
        basis.configuration = None;
        basis.coverage.clear();
        basis.catalog.configuration_revision = None;
        basis.catalog.catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [
                {"id":"check.direct", "roots":["target.txt"], "leaves":[], "dependency_mode":"direct"},
                {"id":"check.legacy", "roots":["source.txt"], "leaves":[]}
            ]
        })).unwrap();
        let planned = plan(basis.clone(), 1).unwrap();
        assert_eq!(planned["mode"], "full_catalog_fallback");
        assert_eq!(planned["fallback_reason"], "parser_configuration_unavailable");
        assert_eq!(planned["checks"].as_array().unwrap().len(), 2);

        basis.configuration = snapshot(false).configuration;
        basis.catalog.configuration_revision = Some(1);
        let no_coverage = plan(basis, 1).unwrap();
        assert_eq!(no_coverage["mode"], "full_catalog_fallback");
        assert_eq!(no_coverage["fallback_reason"], "parser_coverage_incomplete");
    }

    #[test]
    fn indexed_assertions_use_only_guarded_snapshot_and_hash_path_identity() {
        let mut basis = snapshot(true);
        basis.catalog.catalog = canonical_catalog(&json!({
            "format_version": 1,
            "checks": [{"id":"check.a", "roots":["source.txt"], "leaves":["target.txt"],
                "assertion":{"kind":"indexed_file_digest", "path":"target.txt", "sha256":"c".repeat(64)}}]
        })).unwrap();
        let planned = plan(basis.clone(), 1).unwrap();
        assert_eq!(planned["mode"], "selective");
        let evaluated = evaluate_declared_checks(&basis, &planned).unwrap();
        assert_eq!(evaluated.len(), 1);
        assert_eq!(evaluated[0].status, "failed");
        assert_eq!(evaluated[0].reason_code, "INDEXED_DIGEST_MISMATCH");
        let payload = evaluated[0].result_payload.as_ref().unwrap();
        assert_eq!(payload["observed_sha256"], "d".repeat(64));
        assert!(
            !serde_json::to_string(payload)
                .unwrap()
                .contains("target.txt")
        );
        assert_eq!(payload["native_workflow_status"], "untested");
    }

    #[test]
    fn selective_plan_is_stable_and_never_claims_execution() {
        let first = plan(snapshot(true), 1).unwrap();
        let second = plan(snapshot(true), 1).unwrap();
        assert_eq!(first, second);
        assert_eq!(first["mode"], "selective");
        assert_eq!(first["checks"].as_array().unwrap().len(), 1);
        assert_eq!(first["checks"][0]["check_id"], "check.a");
        assert_eq!(first["checks"][0]["status"], "planned_not_run");
        assert!(first["checks"][0]["result_id"].is_null());
    }

    #[test]
    fn missing_parser_coverage_plans_full_catalog() {
        let result = plan(snapshot(false), 1).unwrap();
        assert_eq!(result["mode"], "full_catalog_fallback");
        assert_eq!(result["fallback_reason"], "parser_coverage_incomplete");
        assert_eq!(result["checks"].as_array().unwrap().len(), 2);
        assert!(
            result["checks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|check| check["status"] == "planned_not_run")
        );
    }

    #[test]
    fn bounded_snapshot_never_returns_a_truncated_selective_plan() {
        let mut bounded = snapshot(true);
        bounded.bounds_exceeded = true;
        let result = plan(bounded, 1).unwrap();
        assert_eq!(result["mode"], "full_catalog_fallback");
        assert_eq!(result["fallback_reason"], "planning_bound_exceeded");
        assert_eq!(result["checks"].as_array().unwrap().len(), 2);
    }
}
