//! Bounded project-local asset provenance and link validation.
//!
//! This validates manifest structure and file relationships. It does not run
//! Blender, Krita, UEFN, or validate the contents of asset files.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_MANIFEST_BYTES: usize = 1_048_576;
pub const MAX_ASSETS: usize = 1_024;
pub const MAX_FINDINGS: usize = 2_048;
pub const MAX_TRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const MAX_CHANGED_PATHS: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub project_id: String,
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRecord {
    pub id: String,
    /// A bounded identifier such as `blender` or `krita`; no app is launched.
    pub creator_tool: String,
    pub kind: AssetKind,
    /// Forward-slash-separated paths relative to the selected project root.
    pub source: String,
    pub export: String,
    #[serde(default)]
    pub project_target: Option<String>,
    /// Opaque revision written by a producer. Equality is checked, not its truth.
    #[serde(default)]
    pub source_revision: Option<String>,
    #[serde(default)]
    pub export_built_from_revision: Option<String>,
    #[serde(default)]
    pub related_asset_ids: Vec<String>,
    #[serde(default)]
    pub validation_result_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Mesh,
    Texture,
    Material,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingCode {
    InvalidId,
    InvalidCreatorTool,
    InvalidPath,
    DuplicateId,
    DuplicateExport,
    DuplicateTarget,
    SameSourceAndExport,
    MissingFile,
    InaccessibleFile,
    NonRegularFile,
    LinkedPath,
    PathOutsideProject,
    FileTooLarge,
    MissingLineage,
    InvalidRevision,
    IncompleteLineage,
    StaleExport,
    UnknownRelatedAsset,
    SelfRelatedAsset,
    DuplicateRelatedAsset,
    InvalidResultId,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub asset_id: Option<String>,
    pub field: Option<&'static str>,
    pub code: FindingCode,
    pub message: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalExecutionStatus {
    NotChecked,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub schema_version: u32,
    pub asset_count: usize,
    pub local_checks_passed: bool,
    pub finding_count: usize,
    pub findings_truncated: bool,
    pub creator_app_execution: ExternalExecutionStatus,
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    fn new(asset_count: usize) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            asset_count,
            local_checks_passed: true,
            finding_count: 0,
            findings_truncated: false,
            creator_app_execution: ExternalExecutionStatus::NotChecked,
            findings: Vec::new(),
        }
    }

    fn add(&mut self, asset_id: Option<&str>, code: FindingCode, message: &'static str) {
        self.local_checks_passed = false;
        self.finding_count += 1;
        if self.findings.len() < MAX_FINDINGS {
            self.findings.push(Finding {
                asset_id: asset_id.map(str::to_owned),
                field: None,
                code,
                message,
            });
        } else {
            self.findings_truncated = true;
        }
    }

    fn add_link(
        &mut self,
        asset_id: Option<&str>,
        field: &'static str,
        code: FindingCode,
        message: &'static str,
    ) {
        self.add(asset_id, code, message);
        if !self.findings_truncated {
            if let Some(last) = self.findings.last_mut() {
                last.field = Some(field);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    TooLarge,
    InvalidJson,
    UnsupportedSchema,
    BoundsExceeded,
    ProjectRootUnavailable,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooLarge => "asset manifest exceeds the byte limit",
            Self::InvalidJson => "asset manifest is not valid schema JSON",
            Self::UnsupportedSchema => "asset manifest schema version is unsupported",
            Self::BoundsExceeded => "asset manifest exceeds a field or count limit",
            Self::ProjectRootUnavailable => "selected project root is unavailable",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ManifestError {}

/// Parse a bounded manifest. Errors intentionally omit submitted data and paths.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| ManifestError::InvalidJson)?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(ManifestError::UnsupportedSchema);
    }
    check_field_bounds(&manifest)?;
    Ok(manifest)
}

fn check_field_bounds(manifest: &Manifest) -> Result<(), ManifestError> {
    if manifest.project_id.is_empty()
        || manifest.project_id.len() > 128
        || manifest.assets.len() > MAX_ASSETS
        || manifest.assets.iter().any(|a| {
            a.id.len() > 128
                || a.creator_tool.len() > 64
                || a.source.len() > 512
                || a.export.len() > 512
                || a.project_target.as_ref().is_some_and(|v| v.len() > 512)
                || a.source_revision.as_ref().is_some_and(|v| v.len() > 128)
                || a.export_built_from_revision
                    .as_ref()
                    .is_some_and(|v| v.len() > 128)
                || a.related_asset_ids.len() > 64
                || a.validation_result_ids.len() > 32
                || a.related_asset_ids.iter().any(|v| v.len() > 128)
                || a.validation_result_ids.iter().any(|v| v.len() > 128)
        })
    {
        return Err(ManifestError::BoundsExceeded);
    }
    Ok(())
}

/// Validate at most 1,024 asset records and 2,048 project-local file links.
/// File contents are never read; recorded revisions are only compared.
pub fn validate_manifest_bytes(
    bytes: &[u8],
    project_root: &Path,
) -> Result<ValidationReport, ManifestError> {
    let manifest = parse_manifest(bytes)?;
    validate_manifest(&manifest, project_root)
}

pub fn validate_manifest(
    manifest: &Manifest,
    project_root: &Path,
) -> Result<ValidationReport, ManifestError> {
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(ManifestError::UnsupportedSchema);
    }
    check_field_bounds(manifest)?;
    if serde_json::to_vec(manifest)
        .map_err(|_| ManifestError::BoundsExceeded)?
        .len()
        > MAX_MANIFEST_BYTES
    {
        return Err(ManifestError::TooLarge);
    }
    let root = fs::canonicalize(project_root).map_err(|_| ManifestError::ProjectRootUnavailable)?;
    if !root.is_dir() {
        return Err(ManifestError::ProjectRootUnavailable);
    }

    let mut report = ValidationReport::new(manifest.assets.len());
    let mut ids = HashMap::new();
    let mut exports = HashSet::new();
    let mut targets = HashSet::new();

    for asset in &manifest.assets {
        let valid_id = valid_token(&asset.id, 128);
        let asset_id = valid_id.then_some(asset.id.as_str());
        if !valid_id {
            report.add(
                asset_id,
                FindingCode::InvalidId,
                "asset ID must be a bounded token",
            );
        }
        let id_key = asset.id.to_lowercase();
        if ids.insert(id_key, ()).is_some() {
            report.add(asset_id, FindingCode::DuplicateId, "asset ID is repeated");
        }
        if !valid_token(&asset.creator_tool, 64) {
            report.add(
                asset_id,
                FindingCode::InvalidCreatorTool,
                "creator tool must be a bounded token",
            );
        }

        let source_ok = valid_relative_path(&asset.source);
        let export_ok = valid_relative_path(&asset.export);
        if !source_ok {
            report.add_link(
                asset_id,
                "source",
                FindingCode::InvalidPath,
                "source path is unsafe",
            );
        }
        if !export_ok {
            report.add_link(
                asset_id,
                "export",
                FindingCode::InvalidPath,
                "export path is unsafe",
            );
        }
        if source_ok && export_ok {
            if asset.source.eq_ignore_ascii_case(&asset.export) {
                report.add(
                    asset_id,
                    FindingCode::SameSourceAndExport,
                    "source and export must be separate files",
                );
            }
            if !exports.insert(asset.export.to_lowercase()) {
                report.add(
                    asset_id,
                    FindingCode::DuplicateExport,
                    "export is claimed by more than one asset",
                );
            }
            check_local_file(&root, &asset.source, "source", asset_id, &mut report);
            check_local_file(&root, &asset.export, "export", asset_id, &mut report);
        }
        if let Some(target) = &asset.project_target {
            if !valid_relative_path(target) {
                report.add(
                    asset_id,
                    FindingCode::InvalidPath,
                    "project target path is unsafe",
                );
            } else if !targets.insert(target.to_lowercase()) {
                report.add(
                    asset_id,
                    FindingCode::DuplicateTarget,
                    "project target is claimed by more than one asset",
                );
            }
        }
        match (&asset.source_revision, &asset.export_built_from_revision) {
            (None, None) => report.add(
                asset_id,
                FindingCode::MissingLineage,
                "source/export revision relationship is not recorded",
            ),
            (Some(source), Some(built_from)) => {
                if !valid_token(source, 128) || !valid_token(built_from, 128) {
                    report.add(
                        asset_id,
                        FindingCode::InvalidRevision,
                        "source/export revisions must be bounded tokens",
                    );
                } else if source != built_from {
                    report.add(
                        asset_id,
                        FindingCode::StaleExport,
                        "export does not declare the current source revision",
                    );
                }
            }
            (Some(_), None) | (None, Some(_)) => report.add(
                asset_id,
                FindingCode::IncompleteLineage,
                "source and export revisions must be recorded together",
            ),
        }
        for result_id in &asset.validation_result_ids {
            if !valid_token(result_id, 128) {
                report.add(
                    asset_id,
                    FindingCode::InvalidResultId,
                    "validation result ID must be a bounded token",
                );
            }
        }
    }

    for asset in &manifest.assets {
        let safe_id = valid_token(&asset.id, 128).then_some(asset.id.as_str());
        let mut seen = HashSet::new();
        for related in &asset.related_asset_ids {
            let key = related.to_lowercase();
            if key == asset.id.to_lowercase() {
                report.add(
                    safe_id,
                    FindingCode::SelfRelatedAsset,
                    "asset cannot relate to itself",
                );
            } else if !ids.contains_key(&key) {
                report.add(
                    safe_id,
                    FindingCode::UnknownRelatedAsset,
                    "related asset ID is absent from this manifest",
                );
            } else if !seen.insert(key) {
                report.add(
                    safe_id,
                    FindingCode::DuplicateRelatedAsset,
                    "related asset ID is repeated",
                );
            }
        }
    }

    Ok(report)
}

/// An impact reason names a manifest relationship, never a path or file content.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactReasonKind {
    SourceChanged,
    ExportChanged,
    ProjectTargetChanged,
    RelatedDependency,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImpactReason {
    pub kind: ImpactReasonKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetImpact {
    pub asset_id: String,
    pub reasons: Vec<ImpactReason>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub schema_version: u32,
    pub changed_path_count: usize,
    pub affected_asset_count: usize,
    pub creator_app_execution: ExternalExecutionStatus,
    pub assets: Vec<AssetImpact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactError {
    UnsupportedSchema,
    BoundsExceeded,
    InvalidManifest,
    UnsafeChangedPath,
}

impl fmt::Display for ImpactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedSchema => "asset manifest schema version is unsupported",
            Self::BoundsExceeded => "asset impact input exceeds a count or byte limit",
            Self::InvalidManifest => "asset manifest relationships are invalid",
            Self::UnsafeChangedPath => "changed project-relative path is unsafe",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ImpactError {}

/// Determine assets touched by project-relative path changes. Related links are
/// treated conservatively as undirected dependencies and propagated transitively.
/// The manifest may refer to missing or stale local files; this rechecks
/// structure and relationships without touching the filesystem.
pub fn analyze_impact(
    manifest: &Manifest,
    changed_paths: &[String],
) -> Result<ImpactReport, ImpactError> {
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(ImpactError::UnsupportedSchema);
    }
    check_field_bounds(manifest).map_err(|_| ImpactError::BoundsExceeded)?;
    if serde_json::to_vec(manifest)
        .map_err(|_| ImpactError::BoundsExceeded)?
        .len()
        > MAX_MANIFEST_BYTES
        || changed_paths.len() > MAX_CHANGED_PATHS
    {
        return Err(ImpactError::BoundsExceeded);
    }
    if changed_paths.iter().any(|path| !valid_relative_path(path)) {
        return Err(ImpactError::UnsafeChangedPath);
    }
    let changed: HashSet<_> = changed_paths
        .iter()
        .map(|path| path.to_lowercase())
        .collect();

    let mut by_id = HashMap::new();
    let mut exports = HashSet::new();
    let mut targets = HashSet::new();
    for (index, asset) in manifest.assets.iter().enumerate() {
        if !valid_token(&asset.id, 128)
            || !valid_relative_path(&asset.source)
            || !valid_relative_path(&asset.export)
            || asset.source.eq_ignore_ascii_case(&asset.export)
            || !exports.insert(asset.export.to_lowercase())
            || asset.project_target.as_ref().is_some_and(|target| {
                !valid_relative_path(target) || !targets.insert(target.to_lowercase())
            })
            || by_id.insert(asset.id.to_lowercase(), index).is_some()
        {
            return Err(ImpactError::InvalidManifest);
        }
    }

    let mut neighbors = vec![Vec::new(); manifest.assets.len()];
    for (index, asset) in manifest.assets.iter().enumerate() {
        let mut seen = HashSet::new();
        for related in &asset.related_asset_ids {
            let key = related.to_lowercase();
            let Some(&other) = by_id.get(&key) else {
                return Err(ImpactError::InvalidManifest);
            };
            if !valid_token(related, 128) || other == index || !seen.insert(key) {
                return Err(ImpactError::InvalidManifest);
            }
            neighbors[index].push(other);
            neighbors[other].push(index);
        }
    }
    for entries in &mut neighbors {
        entries.sort_unstable_by(|a, b| {
            manifest.assets[*a]
                .id
                .to_lowercase()
                .cmp(&manifest.assets[*b].id.to_lowercase())
        });
        entries.dedup();
    }

    let mut reasons = vec![Vec::new(); manifest.assets.len()];
    let mut queue = VecDeque::new();
    let mut order: Vec<_> = (0..manifest.assets.len()).collect();
    order.sort_unstable_by(|a, b| {
        manifest.assets[*a]
            .id
            .to_lowercase()
            .cmp(&manifest.assets[*b].id.to_lowercase())
    });
    for &index in &order {
        let asset = &manifest.assets[index];
        for (path, kind) in [
            (&asset.source, ImpactReasonKind::SourceChanged),
            (&asset.export, ImpactReasonKind::ExportChanged),
        ] {
            if changed.contains(&path.to_lowercase()) {
                reasons[index].push(ImpactReason {
                    kind,
                    via_asset_id: None,
                });
            }
        }
        if asset
            .project_target
            .as_ref()
            .is_some_and(|target| changed.contains(&target.to_lowercase()))
        {
            reasons[index].push(ImpactReason {
                kind: ImpactReasonKind::ProjectTargetChanged,
                via_asset_id: None,
            });
        }
        if !reasons[index].is_empty() {
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        for &other in &neighbors[index] {
            if reasons[other].is_empty() {
                reasons[other].push(ImpactReason {
                    kind: ImpactReasonKind::RelatedDependency,
                    via_asset_id: Some(manifest.assets[index].id.clone()),
                });
                queue.push_back(other);
            }
        }
    }
    let mut assets = Vec::new();
    for index in order {
        if !reasons[index].is_empty() {
            assets.push(AssetImpact {
                asset_id: manifest.assets[index].id.clone(),
                reasons: std::mem::take(&mut reasons[index]),
            });
        }
    }
    Ok(ImpactReport {
        schema_version: SCHEMA_VERSION,
        changed_path_count: changed.len(),
        affected_asset_count: assets.len(),
        creator_app_execution: ExternalExecutionStatus::NotChecked,
        assets,
    })
}

fn valid_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

fn valid_relative_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains(['\\', ':'])
        || value.bytes().any(|b| b < 32 || b == 127)
    {
        return false;
    }
    value.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && !segment.ends_with([' ', '.'])
            && !is_windows_device_name(segment)
    })
}

fn is_windows_device_name(segment: &str) -> bool {
    let base = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

fn check_local_file(
    root: &Path,
    relative: &str,
    field: &'static str,
    asset_id: Option<&str>,
    report: &mut ValidationReport,
) {
    let mut path = PathBuf::from(root);
    let parts: Vec<_> = relative.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.add_link(
                    asset_id,
                    field,
                    FindingCode::MissingFile,
                    "asset file is missing",
                );
                return;
            }
            Err(_) => {
                report.add_link(
                    asset_id,
                    field,
                    FindingCode::InaccessibleFile,
                    "asset file is inaccessible",
                );
                return;
            }
        };
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            report.add_link(
                asset_id,
                field,
                FindingCode::LinkedPath,
                "asset path contains a link",
            );
            return;
        }
        if index + 1 < parts.len() && !metadata.is_dir() {
            report.add_link(
                asset_id,
                field,
                FindingCode::NonRegularFile,
                "asset path has a non-directory parent",
            );
            return;
        }
        if index + 1 == parts.len() && !metadata.is_file() {
            report.add_link(
                asset_id,
                field,
                FindingCode::NonRegularFile,
                "asset is not a regular file",
            );
            return;
        }
        if metadata.is_file() && metadata.len() > MAX_TRACKED_FILE_BYTES {
            report.add_link(
                asset_id,
                field,
                FindingCode::FileTooLarge,
                "asset exceeds the tracked file size limit",
            );
            return;
        }
    }
    match fs::canonicalize(&path) {
        Ok(actual) if actual.starts_with(root) => {}
        Ok(_) => report.add_link(
            asset_id,
            field,
            FindingCode::PathOutsideProject,
            "asset resolves outside the selected project",
        ),
        Err(_) => report.add_link(
            asset_id,
            field,
            FindingCode::InaccessibleFile,
            "asset file is inaccessible",
        ),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cross_platform_path_escapes() {
        for path in [
            "../out",
            "a/../out",
            "C:/out",
            "\\\\host\\share",
            "/out",
            "a//b",
        ] {
            assert!(!valid_relative_path(path), "{path}");
        }
        assert!(valid_relative_path("Source/mesh.blend"));
    }

    #[test]
    fn validates_local_links_and_declared_lineage_without_creator_execution() {
        let root = std::env::temp_dir().join(format!(
            "relay-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("source.blend"), b"source").unwrap();
        fs::write(root.join("export.fbx"), b"export").unwrap();
        let mut manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            project_id: "sample".into(),
            assets: vec![AssetRecord {
                id: "mesh_1".into(),
                creator_tool: "blender".into(),
                kind: AssetKind::Mesh,
                source: "source.blend".into(),
                export: "export.fbx".into(),
                project_target: None,
                source_revision: Some("rev1".into()),
                export_built_from_revision: Some("rev1".into()),
                related_asset_ids: vec![],
                validation_result_ids: vec![],
            }],
        };
        let report = validate_manifest(&manifest, &root).unwrap();
        assert!(report.local_checks_passed);
        assert_eq!(
            report.creator_app_execution,
            ExternalExecutionStatus::NotChecked
        );

        manifest.assets[0].export_built_from_revision = Some("rev0".into());
        let report = validate_manifest(&manifest, &root).unwrap();
        assert!(!report.local_checks_passed);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == FindingCode::StaleExport)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_oversized_manifest_before_parsing() {
        let bytes = vec![b' '; MAX_MANIFEST_BYTES + 1];
        assert_eq!(parse_manifest(&bytes).unwrap_err(), ManifestError::TooLarge);
    }

    fn impact_asset(id: &str, related: &[&str]) -> AssetRecord {
        AssetRecord {
            id: id.into(),
            creator_tool: "blender".into(),
            kind: AssetKind::Mesh,
            source: format!("Source/{id}.blend"),
            export: format!("Export/{id}.fbx"),
            project_target: Some(format!("Target/{id}.uasset")),
            source_revision: Some("rev1".into()),
            export_built_from_revision: Some("rev1".into()),
            related_asset_ids: related.iter().map(|id| (*id).into()).collect(),
            validation_result_ids: vec![],
        }
    }

    #[test]
    fn impact_is_deterministic_and_propagates_related_dependencies() {
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            project_id: "sample".into(),
            assets: vec![
                impact_asset("c", &[]),
                impact_asset("a", &["b"]),
                impact_asset("b", &["c"]),
                impact_asset("unrelated", &[]),
            ],
        };
        let paths = vec!["source/A.blend".into(), "Source/a.blend".into()];
        let report = analyze_impact(&manifest, &paths).unwrap();
        assert_eq!(report.changed_path_count, 1);
        assert_eq!(report.affected_asset_count, 3);
        assert_eq!(
            report
                .assets
                .iter()
                .map(|a| a.asset_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            report.assets[0].reasons[0].kind,
            ImpactReasonKind::SourceChanged
        );
        assert_eq!(
            report.assets[1].reasons[0].via_asset_id.as_deref(),
            Some("a")
        );
        assert_eq!(
            report.assets[2].reasons[0].via_asset_id.as_deref(),
            Some("b")
        );
        assert_eq!(
            report.creator_app_execution,
            ExternalExecutionStatus::NotChecked
        );
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("Source/a.blend"));
    }

    #[test]
    fn impact_rejects_unsafe_paths_and_excess_work() {
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            project_id: "sample".into(),
            assets: vec![impact_asset("a", &[])],
        };
        assert_eq!(
            analyze_impact(&manifest, &["../outside".into()]).unwrap_err(),
            ImpactError::UnsafeChangedPath
        );
        assert_eq!(
            analyze_impact(
                &manifest,
                &vec!["Source/a.blend".into(); MAX_CHANGED_PATHS + 1]
            )
            .unwrap_err(),
            ImpactError::BoundsExceeded
        );
        let mut bad = manifest;
        bad.assets[0].related_asset_ids.push("missing".into());
        assert_eq!(
            analyze_impact(&bad, &[]).unwrap_err(),
            ImpactError::InvalidManifest
        );
    }
}
