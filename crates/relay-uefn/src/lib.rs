//! Bounded, read-only UEFN project inspection from RELAY's generic file index.
//!
//! The caller must supply paths from a ready project index after applying the
//! normal project authorization. No file is opened and no editor capability is
//! inferred from a filename.

use serde::Serialize;

/// Maximum number of source paths in a compact command result. Counts remain exact.
pub const MAX_REPORTED_VERSE_PATHS: usize = 256;
const MAX_REPORTED_MARKERS: usize = 8;

#[derive(Debug, Clone)]
pub struct StaticProjectInput {
    pub project_id: String,
    pub index_generation: i64,
    /// Paths use the canonical project index's `/`-separated relative form.
    pub relative_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticInspection {
    pub schema_version: u32,
    pub project_id: String,
    pub index_generation: i64,
    pub rejected_index_path_count: usize,
    pub marker_state: MarkerState,
    pub project_markers: Vec<String>,
    pub project_marker_count: usize,
    pub verse_source_count: usize,
    pub verse_sources: Vec<String>,
    pub verse_sources_omitted: usize,
    pub generated_verse_count: usize,
    pub unreal_asset_count: usize,
    pub unreal_map_count: usize,
    pub findings: Vec<StaticFinding>,
    pub capabilities: StaticCapabilities,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkerState {
    Missing,
    Present,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticFinding {
    pub code: &'static str,
    pub severity: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticCapabilities {
    pub indexed_file_inventory: &'static str,
    pub project_marker_validation: &'static str,
    pub verse_compile: &'static str,
    pub editor_inspection: &'static str,
    pub runtime_telemetry: &'static str,
}

/// Classify indexed filenames only. This does not validate UEFN syntax or content.
pub fn inspect(input: StaticProjectInput) -> StaticInspection {
    let mut markers = Vec::new();
    let mut sources = Vec::new();
    let mut generated_verse_count = 0;
    let mut unreal_asset_count = 0;
    let mut unreal_map_count = 0;
    let mut rejected_index_path_count = 0;

    for path in input.relative_paths {
        // The index supplies paths, but this adapter independently rejects paths
        // outside its project-relative representation before echoing them.
        if !safe_relative_path(&path) {
            rejected_index_path_count += 1;
            continue;
        }
        let lower = path.to_ascii_lowercase();
        if !path.contains('/') && lower.ends_with(".uefnproject") {
            markers.push(path);
        } else if lower.ends_with(".digest.verse") {
            generated_verse_count += 1;
        } else if lower.ends_with(".verse") {
            sources.push(path);
        } else if lower.ends_with(".uasset") {
            unreal_asset_count += 1;
        } else if lower.ends_with(".umap") {
            unreal_map_count += 1;
        }
    }

    markers.sort();
    markers.dedup();
    sources.sort();
    sources.dedup();
    let marker_count = markers.len();
    let source_count = sources.len();
    let marker_state = match marker_count {
        0 => MarkerState::Missing,
        1 => MarkerState::Present,
        _ => MarkerState::Ambiguous,
    };
    markers.truncate(MAX_REPORTED_MARKERS);
    sources.truncate(MAX_REPORTED_VERSE_PATHS);

    let mut findings = match marker_state {
        MarkerState::Missing => vec![StaticFinding {
            code: "UEFN_PROJECT_MARKER_MISSING",
            severity: "warning",
            detail: "No top-level .uefnproject file appears in the ready index.",
        }],
        MarkerState::Ambiguous => vec![StaticFinding {
            code: "UEFN_PROJECT_MARKER_AMBIGUOUS",
            severity: "warning",
            detail: "More than one top-level .uefnproject file appears in the ready index.",
        }],
        MarkerState::Present => Vec::new(),
    };
    if rejected_index_path_count > 0 {
        findings.push(StaticFinding {
            code: "UEFN_INDEX_PATH_REJECTED",
            severity: "warning",
            detail: "One or more indexed paths could not be safely classified; counts may be incomplete.",
        });
    }

    StaticInspection {
        schema_version: 1,
        project_id: input.project_id,
        index_generation: input.index_generation,
        rejected_index_path_count,
        marker_state,
        project_markers: markers,
        project_marker_count: marker_count,
        verse_source_count: source_count,
        verse_sources: sources,
        verse_sources_omitted: source_count.saturating_sub(MAX_REPORTED_VERSE_PATHS),
        generated_verse_count,
        unreal_asset_count,
        unreal_map_count,
        findings,
        capabilities: StaticCapabilities {
            indexed_file_inventory: "available",
            project_marker_validation: "unsupported",
            verse_compile: "not_checked",
            editor_inspection: "not_checked",
            runtime_telemetry: "not_checked",
        },
    }
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && !path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}
