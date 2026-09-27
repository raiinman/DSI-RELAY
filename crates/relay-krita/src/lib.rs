//! Bounded Krita asset metadata inspection. No Krita process is started here.

use relay_assets::{FindingCode as AssetFindingCode, Manifest, ManifestError, validate_manifest};
use serde::Serialize;
use std::fs;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_VISIBLE_ASSETS: usize = 64;
pub const MAX_VISIBLE_FINDINGS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryState {
    NotConfigured,
    PresentUnverified,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Untested,
}

#[derive(Debug, Clone, Serialize)]
pub struct KritaCapabilities {
    pub schema_version: u32,
    pub local_manifest_inspection: bool,
    pub declared_format_check: bool,
    /// Krita's published CLI documents export, but RELAY has not enabled it.
    pub cli_export_documented: bool,
    pub binary_state: BinaryState,
    pub native_export: ExecutionState,
    pub native_content_validation: ExecutionState,
}

/// Examine only an explicitly configured path. A present file is not proof that
/// it is Krita, has a compatible version, or can export this project's assets.
pub fn discover(configured_binary: Option<&Path>) -> KritaCapabilities {
    let binary_state = match configured_binary {
        None => BinaryState::NotConfigured,
        Some(path)
            if path
                .extension()
                .and_then(|part| part.to_str())
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe")) =>
        {
            BinaryState::Invalid
        }
        Some(path) => match fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && !is_reparse_point(&metadata) =>
            {
                BinaryState::PresentUnverified
            }
            Ok(_) => BinaryState::Invalid,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BinaryState::Missing,
            Err(_) => BinaryState::Invalid,
        },
    };
    KritaCapabilities {
        schema_version: SCHEMA_VERSION,
        local_manifest_inspection: true,
        declared_format_check: true,
        cli_export_documented: true,
        binary_state,
        native_export: ExecutionState::Untested,
        native_content_validation: ExecutionState::Untested,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredFormat {
    Kra,
    Png,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct KritaAsset {
    pub id: Option<String>,
    pub source_format: DeclaredFormat,
    pub export_format: DeclaredFormat,
    pub declared_revision_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingCode {
    UnsupportedSourceFormat,
    UnsupportedExportFormat,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub asset_id: Option<String>,
    pub code: FindingCode,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenericFinding {
    pub asset_id: Option<String>,
    pub field: Option<&'static str>,
    pub code: AssetFindingCode,
}

#[derive(Debug, Clone, Serialize)]
pub struct KritaReport {
    pub schema_version: u32,
    pub capabilities: KritaCapabilities,
    pub krita_asset_count: usize,
    pub assets: Vec<KritaAsset>,
    pub assets_omitted: usize,
    pub generic_local_checks_passed: bool,
    pub generic_finding_count: usize,
    pub generic_findings: Vec<GenericFinding>,
    pub generic_findings_truncated: bool,
    pub krita_format_finding_count: usize,
    pub format_findings: Vec<Finding>,
    pub format_findings_truncated: bool,
    pub native_workflow: ExecutionState,
}

/// Inspect declared Krita records after the generic manifest has checked
/// project-local file links and lineage. No file content or external process is read.
pub fn inspect(
    manifest: &Manifest,
    project_root: &Path,
    configured_binary: Option<&Path>,
) -> Result<KritaReport, ManifestError> {
    let generic = validate_manifest(manifest, project_root)?;
    let mut report = KritaReport {
        schema_version: SCHEMA_VERSION,
        capabilities: discover(configured_binary),
        krita_asset_count: 0,
        assets: Vec::new(),
        assets_omitted: 0,
        generic_local_checks_passed: generic.local_checks_passed,
        generic_finding_count: generic.finding_count,
        generic_findings: generic
            .findings
            .iter()
            .take(MAX_VISIBLE_FINDINGS)
            .map(|finding| GenericFinding {
                asset_id: finding.asset_id.clone(),
                field: finding.field,
                code: finding.code,
            })
            .collect(),
        generic_findings_truncated: generic.findings_truncated
            || generic.findings.len() > MAX_VISIBLE_FINDINGS,
        krita_format_finding_count: 0,
        format_findings: Vec::new(),
        format_findings_truncated: false,
        native_workflow: ExecutionState::Untested,
    };
    for asset in &manifest.assets {
        if !asset.creator_tool.eq_ignore_ascii_case("krita") {
            continue;
        }
        report.krita_asset_count += 1;
        let source_format = declared_format(&asset.source);
        let export_format = declared_format(&asset.export);
        if source_format != DeclaredFormat::Kra {
            report.add_finding(&asset.id, FindingCode::UnsupportedSourceFormat);
        }
        if export_format != DeclaredFormat::Png {
            report.add_finding(&asset.id, FindingCode::UnsupportedExportFormat);
        }
        if report.assets.len() < MAX_VISIBLE_ASSETS {
            report.assets.push(KritaAsset {
                id: safe_id(&asset.id),
                source_format,
                export_format,
                declared_revision_matches: asset
                    .source_revision
                    .as_ref()
                    .zip(asset.export_built_from_revision.as_ref())
                    .is_some_and(|(source, built_from)| source == built_from),
            });
        } else {
            report.assets_omitted += 1;
        }
    }
    Ok(report)
}

impl KritaReport {
    fn add_finding(&mut self, asset_id: &str, code: FindingCode) {
        self.krita_format_finding_count += 1;
        if self.format_findings.len() < MAX_VISIBLE_FINDINGS {
            self.format_findings.push(Finding {
                asset_id: safe_id(asset_id),
                code,
            });
        } else {
            self.format_findings_truncated = true;
        }
    }
}

fn declared_format(relative: &str) -> DeclaredFormat {
    match Path::new(relative)
        .extension()
        .and_then(|part| part.to_str())
    {
        Some(ext) if ext.eq_ignore_ascii_case("kra") => DeclaredFormat::Kra,
        Some(ext) if ext.eq_ignore_ascii_case("png") => DeclaredFormat::Png,
        _ => DeclaredFormat::Other,
    }
}

fn safe_id(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return None;
    }
    Some(value.to_string())
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
