//! Read-only first-run discovery. A detected file or executable is a lead for
//! onboarding, never evidence that an editor or runtime workflow passed.

use serde::Serialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_DIRECTORIES: usize = 256;
const MAX_ENTRIES: usize = 8192;
const MAX_DEPTH: usize = 2;

/// Summarize shared read-only command results. A UEFN finding is relevant only
/// when the indexed project contains a top-level UEFN marker.
pub fn first_audit(capabilities: &Value, inspection: &Value) -> Value {
    let mut available = Vec::new();
    let mut gaps = Vec::new();
    if let Some(items) = capabilities["capabilities"].as_array() {
        for item in items {
            let (Some(id), Some(state)) = (item["id"].as_str(), item["state"].as_str()) else {
                continue;
            };
            if state == "available" {
                available.push(id.to_string());
            } else {
                gaps.push(json!({
                    "capability": id,
                    "state": state,
                    "detail": item["detail"]
                }));
            }
        }
    }
    let marker_count = inspection["project_marker_count"].as_u64().unwrap_or(0);
    let uefn = if marker_count > 0 {
        Some(json!({
            "marker_state": inspection["marker_state"],
            "marker_count": marker_count,
            "verse_source_count": inspection["verse_source_count"],
            "unreal_asset_count": inspection["unreal_asset_count"],
            "unreal_map_count": inspection["unreal_map_count"],
            "finding_codes": inspection["findings"].as_array().map(|items| items.iter()
                .filter_map(|item| item["code"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),
            "editor_status": "untested",
            "runtime_status": "untested"
        }))
    } else {
        None
    };
    json!({
        "index_status": capabilities["index_status"],
        "content_verification_required": capabilities["content_verification_required"],
        "available_capabilities": available,
        "capability_gaps": gaps,
        "uefn_static": uefn
    })
}

pub fn audit_responses_match_project(
    project_id: &str,
    baseline: &Value,
    capabilities: &Value,
    inspection: &Value,
) -> bool {
    let Some(generation) = baseline["generation"].as_i64() else {
        return false;
    };
    capabilities["project_id"] == project_id
        && capabilities["capabilities"].is_array()
        && capabilities["index_status"].as_str().is_some()
        && inspection["project_id"] == project_id
        && inspection["index_generation"].as_i64() == Some(generation)
        && inspection["project_marker_count"].as_u64().is_some()
}

#[derive(Debug, Serialize)]
pub struct DiscoveryReport {
    pub format_version: u32,
    pub scan_root: String,
    pub projects: Vec<ProjectCandidate>,
    pub tools: Vec<ToolObservation>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectCandidate {
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub marker: String,
    pub workflow_status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ToolObservation {
    pub id: &'static str,
    pub name: &'static str,
    pub detection_status: &'static str,
    pub detection_source: Option<&'static str>,
    pub workflow_status: &'static str,
}

/// Inspect `root` and nearby folders without executing tools or modifying data.
pub fn discover(root: &Path) -> Result<DiscoveryReport, String> {
    if !root.is_dir() {
        return Err("discovery root is not an accessible directory".to_string());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "discovery root could not be resolved".to_string())?;
    let (projects, limitations) = scan_projects(&root);
    Ok(DiscoveryReport {
        format_version: 1,
        scan_root: root.to_string_lossy().into_owned(),
        projects,
        tools: discover_tools(),
        limitations,
    })
}

pub fn render_human(report: &DiscoveryReport) -> String {
    let mut lines = vec![format!("RELAY discovery in {}", report.scan_root)];
    lines.push("Project candidates: ".to_string() + &report.projects.len().to_string());
    for project in &report.projects {
        lines.push(format!(
            "- {} ({}) at {} — project workflow untested",
            project.name, project.kind, project.path
        ));
    }
    if report.projects.is_empty() {
        lines.push("- None found nearby. You can add a project by path.".to_string());
    }
    lines.push("Tools:".to_string());
    for tool in &report.tools {
        let detail = match tool.detection_source {
            Some(source) => format!("detected via {source}; tool use untested"),
            None => "not detected in common locations; availability unknown".to_string(),
        };
        lines.push(format!("- {}: {detail}", tool.name));
    }
    for limitation in &report.limitations {
        lines.push(format!("Scan note: {limitation}"));
    }
    lines.push(
        "Discovery only reads local metadata. Register a project and validate each integration before use."
            .to_string(),
    );
    lines.join("\n")
}

fn scan_projects(root: &Path) -> (Vec<ProjectCandidate>, Vec<String>) {
    let mut projects = Vec::new();
    let mut limitations = Vec::new();
    let mut queue = VecDeque::from([(root.to_path_buf(), 0usize)]);
    let mut directories = 0usize;
    let mut entries = 0usize;

    while let Some((directory, depth)) = queue.pop_front() {
        if directories >= MAX_DIRECTORIES || entries >= MAX_ENTRIES {
            limitations.push(
                "Nearby-folder scan reached its safety limit; some folders were not inspected."
                    .to_string(),
            );
            break;
        }
        directories += 1;
        let listing = match fs::read_dir(&directory) {
            Ok(listing) => listing,
            Err(_) => {
                limitations.push(format!("Could not read {}.", directory.to_string_lossy()));
                continue;
            }
        };
        let mut children = Vec::new();
        let mut markers = Vec::new();
        for entry in listing {
            if entries >= MAX_ENTRIES {
                break;
            }
            entries += 1;
            let Ok(entry) = entry else {
                limitations.push(format!(
                    "Could not read an entry in {}.",
                    directory.to_string_lossy()
                ));
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if depth < MAX_DEPTH && !skip_directory(&name) {
                    children.push(entry.path());
                }
            } else if kind.is_file() {
                if let Some(project_kind) = marker_kind(&name) {
                    markers.push((project_kind, name));
                }
            }
        }
        markers.sort_by(|a, b| (marker_rank(a.0), &a.1).cmp(&(marker_rank(b.0), &b.1)));
        if let Some((kind, marker)) = markers.into_iter().next() {
            let name = directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Project".to_string());
            projects.push(ProjectCandidate {
                name,
                path: directory.to_string_lossy().into_owned(),
                kind,
                marker,
                workflow_status: "untested",
            });
        }
        children.sort();
        queue.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    projects.sort_by(|a, b| a.path.cmp(&b.path));
    (projects, limitations)
}

fn marker_kind(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".uefnproject") {
        Some("UEFN")
    } else if lower.ends_with(".uproject") {
        Some("Unreal")
    } else if lower == "cargo.toml" {
        Some("Rust")
    } else if lower == "package.json" {
        Some("JavaScript")
    } else {
        None
    }
}

fn marker_rank(kind: &str) -> u8 {
    match kind {
        "UEFN" => 0,
        "Unreal" => 1,
        "Rust" => 2,
        _ => 3,
    }
}

fn skip_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | ".svn" | "node_modules" | "target" | "build" | "dist" | ".venv" | "venv"
    )
}

fn discover_tools() -> Vec<ToolObservation> {
    let path_dirs: Vec<PathBuf> =
        env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    let program_files = [
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
    ];
    let mut tools = Vec::new();
    for (id, name, binaries, relative_locations) in [
        (
            "git",
            "Git",
            &["git.exe"][..],
            &["Git/cmd/git.exe", "Git/bin/git.exe"][..],
        ),
        (
            "blender",
            "Blender",
            &["blender.exe"][..],
            &["Blender Foundation/Blender/blender.exe"][..],
        ),
        (
            "krita",
            "Krita",
            &["krita.exe"][..],
            &["Krita (x64)/bin/krita.exe", "Krita/bin/krita.exe"][..],
        ),
        (
            "uefn",
            "Unreal Editor for Fortnite",
            &["UnrealEditorFortnite-Win64-Shipping.exe"][..],
            &[][..],
        ),
    ] {
        let path_found = path_dirs.iter().any(|directory| {
            binaries
                .iter()
                .any(|binary| directory.join(binary).is_file())
        });
        let common_found = !path_found
            && program_files.iter().flatten().any(|base| {
                relative_locations
                    .iter()
                    .any(|relative| Path::new(base).join(relative).is_file())
            });
        let manifest_found =
            id == "uefn" && !path_found && !common_found && epic_manifest_detects_uefn();
        let source = if path_found {
            Some("PATH")
        } else if common_found {
            Some("common installation folder")
        } else if manifest_found {
            Some("Epic installation manifest")
        } else {
            None
        };
        tools.push(ToolObservation {
            id,
            name,
            detection_status: if source.is_some() {
                "detected"
            } else {
                "not_detected"
            },
            detection_source: source,
            workflow_status: "untested",
        });
    }
    tools
}

fn epic_manifest_detects_uefn() -> bool {
    let Some(program_data) = env::var_os("ProgramData") else {
        return false;
    };
    let folder = Path::new(&program_data)
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    let mut inspected = 0usize;
    for entry in entries.flatten() {
        if inspected >= 64 {
            break;
        }
        inspected += 1;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("item") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let name = value["DisplayName"].as_str().unwrap_or("");
        let app = value["AppName"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case("Unreal Editor for Fortnite")
            || app.to_ascii_lowercase().contains("uefn")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn first_audit_reports_capability_gaps_without_inventing_uefn() {
        let capabilities = json!({
            "project_id": "PRJ-one",
            "index_status": "ready",
            "content_verification_required": false,
            "capabilities": [
                {"id":"filesystem.read","state":"available","detail":"readable"},
                {"id":"dependency_graph","state":"unknown","detail":"requires parser"}
            ]
        });
        let inspection = json!({
            "project_id": "PRJ-one", "index_generation": 1,
            "project_marker_count": 0, "marker_state": "missing",
            "findings": [{"code":"UEFN_PROJECT_MARKER_MISSING"}]
        });
        assert!(audit_responses_match_project(
            "PRJ-one",
            &json!({"generation":1}),
            &capabilities,
            &inspection
        ));
        let audit = first_audit(&capabilities, &inspection);
        assert!(audit["uefn_static"].is_null());
        assert_eq!(audit["available_capabilities"], json!(["filesystem.read"]));
        assert_eq!(
            audit["capability_gaps"][0]["capability"],
            "dependency_graph"
        );
        assert_eq!(audit["capability_gaps"][0]["detail"], "requires parser");
    }

    #[test]
    fn first_audit_keeps_uefn_native_workflows_untested() {
        let capabilities = json!({"index_status":"ready", "capabilities":[]});
        let inspection = json!({
            "project_marker_count": 1, "marker_state":"present", "verse_source_count": 3,
            "unreal_asset_count": 2, "unreal_map_count": 1, "findings": []
        });
        let audit = first_audit(&capabilities, &inspection);
        assert_eq!(audit["uefn_static"]["verse_source_count"], 3);
        assert_eq!(audit["uefn_static"]["editor_status"], "untested");
        assert_eq!(audit["uefn_static"]["runtime_status"], "untested");
    }

    #[test]
    fn first_audit_rejects_mismatched_generation() {
        let capabilities =
            json!({"project_id":"PRJ-one", "index_status":"ready", "capabilities":[]});
        let inspection =
            json!({"project_id":"PRJ-one", "index_generation":2, "project_marker_count":0});
        assert!(!audit_responses_match_project(
            "PRJ-one",
            &json!({"generation":1}),
            &capabilities,
            &inspection
        ));
    }

    #[test]
    fn discovers_nearby_uefn_and_skips_generated_tree() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("relay-discovery-{}-{stamp}", std::process::id()));
        let project = root.join("Creator Project");
        let generated = root.join("target").join("fake");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&generated).unwrap();
        fs::write(project.join("Island.uefnproject"), b"{}").unwrap();
        fs::write(generated.join("Cargo.toml"), b"[package]").unwrap();

        let (projects, limitations) = scan_projects(&root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, "UEFN");
        assert_eq!(projects[0].workflow_status, "untested");
        assert!(limitations.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
