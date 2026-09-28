//! Read-only, best-effort project discovery from Codex's private local cache.
//! This cache is not a supported API. Never inspect chats, sessions, or auth data.

use relay_contracts::CommandRequest;
use relay_core::service::ExtensionError;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PROJECTS: usize = 128;
const MAX_ROOTS_PER_PROJECT: usize = 8;
const MAX_CANDIDATES: usize = 32;
const LIMITATIONS: [&str; 3] = [
    "codex_private_cache_best_effort",
    "saved_local_projects_only",
    "current_user_existing_directories_only",
];

pub fn execute(request: &CommandRequest) -> Option<Result<Value, ExtensionError>> {
    (request.command == "codex.projects.discover").then(|| Ok(discover()))
}

fn discover() -> Value {
    let Some(profile) = env::var_os("USERPROFILE") else {
        return unavailable();
    };
    discover_from(&PathBuf::from(profile).join(".codex").join(".codex-global-state.json"))
}

fn unavailable() -> Value {
    json!({
        "source": "codex_local_state",
        "status": "unavailable",
        "candidates": [],
        "limitations": LIMITATIONS,
    })
}

fn discover_from(state_path: &Path) -> Value {
    if !state_path.parent().is_some_and(existing_plain_directory) {
        return unavailable();
    }
    let Ok(metadata) = fs::symlink_metadata(state_path) else {
        return unavailable();
    };
    if !metadata.file_type().is_file() || is_reparse(&metadata) || metadata.len() > MAX_STATE_BYTES {
        return unavailable();
    }
    let Ok(file) = File::open(state_path) else {
        return unavailable();
    };
    let mut bytes = Vec::new();
    if file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes).is_err()
        || bytes.len() as u64 > MAX_STATE_BYTES
    {
        return unavailable();
    }
    let Ok(state) = serde_json::from_slice::<Value>(&bytes) else {
        return unavailable();
    };
    let Some(projects) = state.get("local-projects").and_then(Value::as_object) else {
        return unavailable();
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for (_, project) in projects.iter().take(MAX_PROJECTS) {
        if candidates.len() == MAX_CANDIDATES {
            break;
        }
        if project.get("id").and_then(Value::as_str).is_none() {
            continue;
        }
        let Some(name) = project.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
            continue;
        }
        let Some(roots) = project.get("rootPaths").and_then(Value::as_array) else {
            continue;
        };
        for root in roots.iter().take(MAX_ROOTS_PER_PROJECT) {
            if candidates.len() == MAX_CANDIDATES {
                break;
            }
            let Some(raw) = root.as_str() else { continue };
            if raw.is_empty() || raw.len() > 2048 || raw.chars().any(char::is_control) {
                continue;
            }
            let path = Path::new(raw);
            if !path.is_absolute() || is_codex_mirror(path) || !existing_plain_directory(path) {
                continue;
            }
            let Some(folder_name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if folder_name.is_empty() || folder_name.chars().count() > 120 {
                continue;
            }
            if !seen.insert(raw.to_ascii_lowercase()) {
                continue;
            }
            candidates.push(json!({
                "name": name,
                "folder_name": folder_name,
                "root_path": raw,
            }));
        }
    }
    json!({
        "source": "codex_local_state",
        "status": "available",
        "candidates": candidates,
        "limitations": LIMITATIONS,
    })
}

fn is_codex_mirror(path: &Path) -> bool {
    let mut previous_is_codex = false;
    for component in path.components() {
        let Component::Normal(part) = component else { continue };
        let name = part.to_string_lossy();
        if previous_is_codex && name.eq_ignore_ascii_case(".chatgpt-projects") {
            return true;
        }
        previous_is_codex = name.eq_ignore_ascii_case(".codex");
    }
    false
}

fn existing_plain_directory(path: &Path) -> bool {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        let Ok(metadata) = fs::symlink_metadata(&prefix) else { return false };
        if is_reparse(&metadata) {
            return false;
        }
    }
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !is_reparse(&metadata))
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = env::temp_dir().join(format!("relay-codex-finder-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_only_existing_saved_roots_without_private_fields() {
        let dir = temp_dir();
        let real = dir.join("Project");
        let mirror = dir.join(".codex").join(".chatgpt-projects").join("Mirror");
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&mirror).unwrap();
        let missing = dir.join("Missing");
        let state = dir.join("state.json");
        fs::write(&state, json!({
            "auth": "do-not-read",
            "local-projects": {
                "one": {"id":"one","name":"My game","rootPaths":[real, real, missing, mirror]},
                "two": {"id":"two","name":"Invalid","rootPaths":["relative/path"]}
            }
        }).to_string()).unwrap();
        let result = discover_from(&state);
        relay_contracts::registry::validate_value(
            &relay_contracts::registry::resolve_command("codex.projects.discover", Some(1)).unwrap().result_schema,
            &result,
        ).unwrap();
        assert_eq!(result["status"], "available");
        assert_eq!(result["candidates"].as_array().unwrap().len(), 1);
        assert_eq!(result["candidates"][0]["folder_name"], "Project");
        assert!(!result.to_string().contains("do-not-read"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_missing_malformed_and_oversized_state() {
        let dir = temp_dir();
        let state = dir.join("state.json");
        assert_eq!(discover_from(&state)["status"], "unavailable");
        fs::write(&state, "{").unwrap();
        assert_eq!(discover_from(&state)["status"], "unavailable");
        fs::write(&state, vec![b'x'; MAX_STATE_BYTES as usize + 1]).unwrap();
        assert_eq!(discover_from(&state)["status"], "unavailable");
        fs::remove_dir_all(dir).unwrap();
    }
}
