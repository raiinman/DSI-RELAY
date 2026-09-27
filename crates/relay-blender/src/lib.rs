//! Bounded, read-only Blender mesh inspection.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_BLEND_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_INSPECTED_MESHES: usize = 64;
const MAX_REPORTED_MESHES: usize = 4096;
const MAX_SCANNED_ELEMENTS: usize = 200_000;
const MAX_REPORTED_ELEMENTS: usize = 50_000_000;
const MAX_EXAMPLES: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(30);
const RESULT_PREFIX: &str = "RELAY_MESH_JSON:";

// This program is fixed by RELAY. Mesh.validate() is deliberately not called:
// Blender documents that it may correct or remove geometry.
const INSPECT_SCRIPT: &str = r#"
import bpy, itertools, json, math
limit_meshes = 64
limit_elements = 200000
found = []
issue_count = 0
examples = []
incomplete = len(bpy.data.meshes) > limit_meshes
for mesh_index, mesh in enumerate(itertools.islice(bpy.data.meshes, limit_meshes)):
    vertex_count = len(mesh.vertices)
    edge_count = len(mesh.edges)
    face_count = len(mesh.polygons)
    if max(vertex_count, edge_count, face_count) > limit_elements:
        incomplete = True
    for vertex in itertools.islice(mesh.vertices, limit_elements):
        if not all(math.isfinite(float(component)) for component in vertex.co):
            issue_count += 1
            if len(examples) < 16:
                examples.append({'mesh_index': mesh_index, 'code': 'non_finite_vertex'})
    for edge in itertools.islice(mesh.edges, limit_elements):
        if edge.vertices[0] == edge.vertices[1]:
            issue_count += 1
            if len(examples) < 16:
                examples.append({'mesh_index': mesh_index, 'code': 'self_edge'})
    for face in itertools.islice(mesh.polygons, limit_elements):
        if face.loop_total < 3 or not math.isfinite(float(face.area)) or face.area <= 0:
            issue_count += 1
            if len(examples) < 16:
                examples.append({'mesh_index': mesh_index, 'code': 'degenerate_face'})
    found.append({'mesh_index': mesh_index, 'vertices': vertex_count, 'edges': edge_count, 'faces': face_count})
result = {'format_version': 1, 'blender_version': '.'.join(str(part) for part in bpy.app.version),
          'total_meshes': len(bpy.data.meshes), 'inspected_meshes': found,
          'issue_count': issue_count, 'examples': examples, 'incomplete': incomplete}
print('RELAY_MESH_JSON:' + json.dumps(result, separators=(',', ':')), flush=True)
"#;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Detected,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCapability {
    pub status: ToolStatus,
    pub executable: Option<PathBuf>,
    pub source: Option<&'static str>,
    pub workflow_status: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Failed,
    Incomplete,
    Unavailable,
    InvalidInput,
    ToolError,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub status: ValidationStatus,
    pub scope: &'static str,
    pub message: String,
    pub blender_version: Option<String>,
    pub total_meshes: Option<usize>,
    pub inspected_meshes: Vec<MeshSummary>,
    pub issue_count: Option<usize>,
    pub examples: Vec<MeshIssue>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeshSummary {
    pub mesh_index: usize,
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeshIssue {
    pub mesh_index: usize,
    pub code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeObservation {
    format_version: u32,
    blender_version: String,
    total_meshes: usize,
    inspected_meshes: Vec<MeshSummary>,
    issue_count: usize,
    examples: Vec<MeshIssue>,
    incomplete: bool,
}

/// Locate an executable candidate without running it. An invalid explicit path
/// stays unavailable rather than falling back to a different installation.
pub fn discover(explicit: Option<&Path>) -> ToolCapability {
    if let Some(path) = explicit {
        let expected_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("blender.exe") || name == "blender");
        return capability(
            (expected_name && path.is_file()).then(|| path.to_path_buf()),
            "explicit path",
        );
    }
    let candidates = ["blender.exe", "blender"];
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for name in candidates {
            let path = directory.join(name);
            if path.is_file() {
                return capability(Some(path), "PATH");
            }
        }
    }
    for base in [
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    {
        let folder = Path::new(&base).join("Blender Foundation");
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        let mut entries: Vec<_> = entries
            .flatten()
            .take(32)
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for folder in entries {
            for name in candidates {
                let path = folder.join(name);
                if path.is_file() {
                    return capability(Some(path), "common installation folder");
                }
            }
        }
    }
    capability(None, "")
}

fn capability(path: Option<PathBuf>, source: &'static str) -> ToolCapability {
    let detected = path.is_some();
    ToolCapability {
        status: if detected {
            ToolStatus::Detected
        } else {
            ToolStatus::Unavailable
        },
        executable: path,
        source: if detected { Some(source) } else { None },
        workflow_status: "untested",
    }
}

/// Run fixed, read-only geometry checks using a local Blender installation.
/// No project file is changed. A limited scan is explicitly incomplete.
pub fn validate_blend(file: &Path, explicit_blender: Option<&Path>) -> ValidationReport {
    let started = Instant::now();
    let mut report = empty_report();
    if file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("blend"))
        != Some(true)
    {
        report.status = ValidationStatus::InvalidInput;
        report.message = "Choose a local .blend file.".to_string();
        return report;
    }
    let Ok(metadata) = fs::metadata(file) else {
        report.status = ValidationStatus::InvalidInput;
        report.message = "The .blend file cannot be read.".to_string();
        return report;
    };
    if !metadata.is_file() || metadata.len() > MAX_BLEND_BYTES {
        report.status = ValidationStatus::InvalidInput;
        report.message =
            "The .blend file is not a regular file or exceeds the 1 GiB inspection limit."
                .to_string();
        return report;
    }
    let capability = discover(explicit_blender);
    let Some(executable) = capability.executable else {
        report.status = ValidationStatus::Unavailable;
        report.message =
            "Blender was not found. Select its executable or install Blender to run this workflow."
                .to_string();
        return report;
    };
    let file = match file.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            report.status = ValidationStatus::InvalidInput;
            report.message = "The .blend file path could not be resolved.".to_string();
            return report;
        }
    };
    let mut child = match Command::new(executable)
        .args(["--factory-startup", "--background", "--disable-autoexec"])
        .arg(file)
        .args(["--python-exit-code", "13", "--python-expr", INSPECT_SCRIPT])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            report.status = ValidationStatus::ToolError;
            report.message = "Blender could not be started.".to_string();
            return report;
        }
    };
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let out_reader = thread::spawn(move || read_bounded(stdout));
    let err_reader = thread::spawn(move || read_bounded(stderr));
    let mut timed_out = false;
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break Some(exit),
            Ok(None) if started.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Err(_) => {
                let _ = child.kill();
                break child.wait().ok();
            }
        }
    };
    let (output, out_overflow) = out_reader.join().unwrap_or_default();
    let (_, err_overflow) = err_reader.join().unwrap_or_default();
    report.elapsed_ms = started.elapsed().as_millis();
    if timed_out {
        report.status = ValidationStatus::Incomplete;
        report.message = "Blender exceeded the 30-second inspection limit.".to_string();
        return report;
    }
    if out_overflow || err_overflow {
        report.status = ValidationStatus::Incomplete;
        report.message = "Blender produced more output than the inspection limit.".to_string();
        return report;
    }
    if exit.is_none_or(|exit| !exit.success()) {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender exited before inspection completed.".to_string();
        return report;
    }
    let Ok(text) = std::str::from_utf8(&output) else {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender returned an invalid mesh result.".to_string();
        return report;
    };
    let mut payloads = text
        .lines()
        .filter_map(|line| line.strip_prefix(RESULT_PREFIX));
    let Some(payload) = payloads.next() else {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender did not return a RELAY mesh result.".to_string();
        return report;
    };
    if payloads.next().is_some() {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender returned multiple mesh results.".to_string();
        return report;
    }
    let Ok(native) = parse_native_observation(payload) else {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender returned an invalid mesh result.".to_string();
        return report;
    };
    report.blender_version = Some(native.blender_version);
    report.total_meshes = Some(native.total_meshes);
    report.inspected_meshes = native.inspected_meshes;
    report.issue_count = Some(native.issue_count);
    report.examples = native.examples;
    report.status = if native.incomplete {
        ValidationStatus::Incomplete
    } else if native.total_meshes == 0 {
        ValidationStatus::Failed
    } else if native.issue_count > 0 {
        ValidationStatus::Failed
    } else {
        ValidationStatus::Passed
    };
    report.message = match report.status {
        ValidationStatus::Incomplete => {
            "The bounded geometry check did not cover every mesh element.".to_string()
        }
        ValidationStatus::Failed if native.total_meshes == 0 => {
            "The file contains no mesh data to inspect.".to_string()
        }
        ValidationStatus::Failed => "The bounded geometry check found mesh issues.".to_string(),
        _ => "The bounded read-only geometry checks passed.".to_string(),
    };
    report
}

fn parse_native_observation(payload: &str) -> Result<NativeObservation, ()> {
    let native: NativeObservation = serde_json::from_str(payload).map_err(|_| ())?;
    if native.format_version != 1
        || !valid_numeric_version(&native.blender_version)
        || native.total_meshes > MAX_REPORTED_MESHES
        || native.inspected_meshes.len() != native.total_meshes.min(MAX_INSPECTED_MESHES)
        || native.examples.len() > MAX_EXAMPLES
        || native.examples.len() != native.issue_count.min(MAX_EXAMPLES)
        || native.issue_count > MAX_INSPECTED_MESHES * MAX_SCANNED_ELEMENTS * 3
    {
        return Err(());
    }
    let mut scan_limit_hit = native.total_meshes > MAX_INSPECTED_MESHES;
    let mut max_possible_issues = 0usize;
    for (expected_index, mesh) in native.inspected_meshes.iter().enumerate() {
        if mesh.mesh_index != expected_index
            || [mesh.vertices, mesh.edges, mesh.faces]
                .into_iter()
                .any(|count| count > MAX_REPORTED_ELEMENTS)
        {
            return Err(());
        }
        scan_limit_hit |= [mesh.vertices, mesh.edges, mesh.faces]
            .into_iter()
            .any(|count| count > MAX_SCANNED_ELEMENTS);
        max_possible_issues += [mesh.vertices, mesh.edges, mesh.faces]
            .into_iter()
            .map(|count| count.min(MAX_SCANNED_ELEMENTS))
            .sum::<usize>();
    }
    if native.incomplete != scan_limit_hit || native.issue_count > max_possible_issues {
        return Err(());
    }
    for issue in &native.examples {
        if issue.mesh_index >= native.inspected_meshes.len()
            || !matches!(
                issue.code.as_str(),
                "non_finite_vertex" | "self_edge" | "degenerate_face"
            )
        {
            return Err(());
        }
    }
    Ok(native)
}

fn valid_numeric_version(version: &str) -> bool {
    if version.len() > 11 {
        return false;
    }
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 3 && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn empty_report() -> ValidationReport {
    ValidationReport {
        status: ValidationStatus::Incomplete,
        scope: "bounded_read_only_geometry_checks",
        message: String::new(),
        blender_version: None,
        total_meshes: None,
        inspected_meshes: Vec::new(),
        issue_count: None,
        examples: Vec::new(),
        elapsed_ms: 0,
    }
}

fn read_bounded(mut stream: impl Read) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut overflow = false;
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let room = MAX_OUTPUT_BYTES.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..read.min(room)]);
                overflow |= read > room;
            }
        }
    }
    (kept, overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_blender_is_unavailable_without_launch() {
        let root = env::temp_dir().join(format!("relay-blender-missing-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let blend = root.join("mesh.blend");
        fs::write(&blend, b"fixture").unwrap();
        let report = validate_blend(&blend, Some(&root.join("missing-blender.exe")));
        assert_eq!(report.status, ValidationStatus::Unavailable);
        assert!(report.blender_version.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_blend_input_before_tool_discovery() {
        let report = validate_blend(Path::new("mesh.py"), None);
        assert_eq!(report.status, ValidationStatus::InvalidInput);
    }

    #[test]
    fn external_observation_rejects_paths_and_inconsistent_counts() {
        let valid = serde_json::json!({
            "format_version": 1,
            "blender_version": "4.5.3",
            "total_meshes": 1,
            "inspected_meshes": [{"mesh_index": 0, "vertices": 3, "edges": 3, "faces": 1}],
            "issue_count": 0,
            "examples": [],
            "incomplete": false
        });
        assert!(parse_native_observation(&valid.to_string()).is_ok());

        let mut cases = Vec::new();
        let mut path_version = valid.clone();
        path_version["blender_version"] = serde_json::json!("C:\\private\\4.5.3");
        cases.push(path_version);
        let mut text_version = valid.clone();
        text_version["blender_version"] = serde_json::json!("4.5.3 private details");
        cases.push(text_version);
        let mut path_issue = valid.clone();
        path_issue["issue_count"] = serde_json::json!(1);
        path_issue["examples"] = serde_json::json!([{"mesh_index":0,"code":"C:\\private\\error"}]);
        cases.push(path_issue);
        let mut false_complete = valid.clone();
        false_complete["inspected_meshes"][0]["vertices"] = serde_json::json!(200001);
        cases.push(false_complete);
        let mut wrong_index = valid.clone();
        wrong_index["inspected_meshes"][0]["mesh_index"] = serde_json::json!(9);
        cases.push(wrong_index);
        let mut impossible_issue_count = valid.clone();
        impossible_issue_count["issue_count"] = serde_json::json!(8);
        impossible_issue_count["examples"] =
            serde_json::json!([{"mesh_index":0,"code":"self_edge"}]);
        cases.push(impossible_issue_count);
        let mut too_many_meshes = valid.clone();
        too_many_meshes["total_meshes"] = serde_json::json!(4097);
        cases.push(too_many_meshes);
        let mut extra_text = valid;
        extra_text["inspected_meshes"][0]["private_path"] = serde_json::json!("secret");
        cases.push(extra_text);
        for value in cases {
            assert!(parse_native_observation(&value.to_string()).is_err());
        }
    }
}
