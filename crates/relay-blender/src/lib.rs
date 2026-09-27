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
result = {'format_version': 1, 'blender_version': bpy.app.version_string,
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
pub struct MeshSummary {
    pub mesh_index: usize,
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
}

#[derive(Debug, Deserialize, Serialize)]
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
        source: if detected {
            Some(source)
        } else {
            None
        },
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
    let text = String::from_utf8_lossy(&output);
    let Some(payload) = text
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(RESULT_PREFIX))
    else {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender did not return a RELAY mesh result.".to_string();
        return report;
    };
    let Ok(native) = serde_json::from_str::<NativeObservation>(payload) else {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender returned an invalid mesh result.".to_string();
        return report;
    };
    if native.format_version != 1
        || native.inspected_meshes.len() > 64
        || native.examples.len() > 16
    {
        report.status = ValidationStatus::ToolError;
        report.message = "Blender returned an incompatible mesh result.".to_string();
        return report;
    }
    report.blender_version = Some(native.blender_version);
    report.total_meshes = Some(native.total_meshes);
    report.inspected_meshes = native.inspected_meshes;
    report.issue_count = Some(native.issue_count);
    report.examples = native.examples;
    report.status = if native.incomplete {
        ValidationStatus::Incomplete
    } else if native.issue_count > 0 {
        ValidationStatus::Failed
    } else {
        ValidationStatus::Passed
    };
    report.message = match report.status {
        ValidationStatus::Incomplete => {
            "The bounded geometry check did not cover every mesh element.".to_string()
        }
        ValidationStatus::Failed => "The bounded geometry check found mesh issues.".to_string(),
        _ => "The bounded read-only geometry checks passed.".to_string(),
    };
    report
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
}
