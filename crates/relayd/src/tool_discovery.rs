//! Bounded installation discovery. This reads local metadata only; finding an
//! executable or Epic manifest never proves that a native workflow succeeded.

use relay_contracts::CommandRequest;
use relay_core::service::ExtensionError;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn execute(request: &CommandRequest) -> Option<Result<Value, ExtensionError>> {
    (request.command == "tools.local.discover").then(|| Ok(discover()))
}

fn discover() -> Value {
    let path_dirs: Vec<PathBuf> = env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .take(128)
        .collect();
    let program_files: Vec<PathBuf> = [env::var_os("ProgramFiles"), env::var_os("ProgramFiles(x86)")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .collect();
    let tools = [
        observation("uefn", detect_uefn(&path_dirs, &program_files)),
        observation("blender", detect_blender(&path_dirs, &program_files)),
        observation("krita", detect_krita(&path_dirs, &program_files)),
    ];
    json!({ "scope": "common_locations", "tools": tools })
}

fn observation(id: &'static str, source: Option<&'static str>) -> Value {
    json!({
        "id": id,
        "detection_status": if source.is_some() { "detected" } else { "not_detected" },
        "detection_source": source,
        "workflow_status": "untested"
    })
}

fn on_path(path_dirs: &[PathBuf], binary: &str) -> bool {
    path_dirs.iter().any(|dir| dir.join(binary).is_file())
}

fn detect_uefn(path_dirs: &[PathBuf], program_files: &[PathBuf]) -> Option<&'static str> {
    const BINARY: &str = "UnrealEditorFortnite-Win64-Shipping.exe";
    if on_path(path_dirs, BINARY) {
        return Some("PATH");
    }
    if program_files.iter().any(|base| {
        base.join("Epic Games")
            .join("Fortnite")
            .join("FortniteGame")
            .join("Binaries")
            .join("Win64")
            .join(BINARY)
            .is_file()
    }) {
        return Some("common_installation_folder");
    }
    epic_manifest_detects_uefn().then_some("epic_installation_manifest")
}

fn detect_blender(path_dirs: &[PathBuf], program_files: &[PathBuf]) -> Option<&'static str> {
    if on_path(path_dirs, "blender.exe") {
        return Some("PATH");
    }
    for base in program_files {
        let root = base.join("Blender Foundation");
        if root.join("Blender").join("blender.exe").is_file() {
            return Some("common_installation_folder");
        }
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.take(32).flatten() {
                if entry.file_name().to_string_lossy().starts_with("Blender")
                    && entry.path().join("blender.exe").is_file()
                {
                    return Some("common_installation_folder");
                }
            }
        }
    }
    None
}

fn detect_krita(path_dirs: &[PathBuf], program_files: &[PathBuf]) -> Option<&'static str> {
    if on_path(path_dirs, "krita.exe") {
        return Some("PATH");
    }
    for base in program_files {
        for relative in ["Krita (x64)/bin/krita.exe", "Krita/bin/krita.exe"] {
            if base.join(relative).is_file() {
                return Some("common_installation_folder");
            }
        }
    }
    None
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
    for entry in entries.take(64).flatten() {
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("item") {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
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

    #[test]
    fn result_is_bounded_and_never_claims_native_verification() {
        let result = discover();
        relay_contracts::registry::validate_value(
            &relay_contracts::registry::resolve_command("tools.local.discover", Some(1)).unwrap().result_schema,
            &result,
        )
        .unwrap();
        assert_eq!(result["tools"].as_array().unwrap().len(), 3);
        assert!(result["tools"].as_array().unwrap().iter().all(|tool| tool["workflow_status"] == "untested"));
        assert!(!result.to_string().contains("C:\\"));
    }
}
