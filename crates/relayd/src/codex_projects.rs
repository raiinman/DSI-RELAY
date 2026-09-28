//! Read-only, best-effort local folder discovery from Codex's private metadata.
//! These files are not a supported API. Never read chat text, sessions, or auth data.

use relay_contracts::CommandRequest;
use relay_core::service::ExtensionError;
use rusqlite::{Connection, OpenFlags};
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
const MAX_TASK_DATABASE_BYTES: u64 = 256 * 1024 * 1024;
const WORKSPACE_LIMITATIONS: [&str; 3] = [
    "private_metadata_best_effort",
    "local_folders_only",
    "cloud_chats_not_indexed",
];
const LIMITATIONS: [&str; 3] = [
    "codex_private_cache_best_effort",
    "saved_local_projects_only",
    "current_user_existing_directories_only",
];

pub fn execute(request: &CommandRequest) -> Option<Result<Value, ExtensionError>> {
    match request.command.as_str() {
        "codex.projects.discover" => Some(Ok(discover())),
        "codex.workspaces.discover" => Some(Ok(discover_workspaces())),
        _ => None,
    }
}

fn discover_workspaces() -> Value {
    let Some(profile) = env::var_os("USERPROFILE") else {
        return unavailable_workspaces();
    };
    let codex_dir = PathBuf::from(profile).join(".codex");
    discover_workspaces_from(
        &codex_dir.join(".codex-global-state.json"),
        &codex_dir.join("state_5.sqlite"),
    )
}

fn unavailable_workspaces() -> Value {
    json!({
        "source": "codex_local_metadata",
        "status": "unavailable",
        "candidates": [],
        "limitations": WORKSPACE_LIMITATIONS,
    })
}

fn discover_workspaces_from(state_path: &Path, database_path: &Path) -> Value {
    let saved = discover_from(state_path);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    if let Some(projects) = saved["candidates"].as_array() {
        for project in projects.iter().take(MAX_CANDIDATES / 2) {
            let Some(root) = project["root_path"].as_str() else { continue };
            seen.insert(root.to_ascii_lowercase());
            let mut candidate = project.clone();
            candidate["origin"] = json!("saved_project");
            candidates.push(candidate);
        }
    }
    let recent = recent_task_folders(database_path);
    if let Some(folders) = recent.as_ref() {
        for raw in folders {
            if candidates.len() == MAX_CANDIDATES { break; }
            if !seen.insert(raw.to_ascii_lowercase()) { continue; }
            let path = Path::new(raw);
            let Some(folder_name) = path.file_name().and_then(|name| name.to_str()) else { continue };
            candidates.push(json!({
                "name": folder_name,
                "folder_name": folder_name,
                "root_path": raw,
                "origin": "recent_task_folder",
            }));
        }
    }
    json!({
        "source": "codex_local_metadata",
        "status": if saved["status"] == "available" || recent.is_some() { "available" } else { "unavailable" },
        "candidates": candidates,
        "limitations": WORKSPACE_LIMITATIONS,
    })
}

fn recent_task_folders(database_path: &Path) -> Option<Vec<String>> {
    if !database_path.parent().is_some_and(existing_plain_directory) { return None; }
    let metadata = fs::symlink_metadata(database_path).ok()?;
    if !metadata.file_type().is_file() || is_reparse(&metadata) || metadata.len() > MAX_TASK_DATABASE_BYTES {
        return None;
    }
    let connection = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    connection.busy_timeout(std::time::Duration::from_millis(50)).ok()?;
    let mut statement = connection.prepare(
        "SELECT cwd FROM threads WHERE cwd IS NOT NULL AND cwd <> '' AND archived = 0 GROUP BY cwd ORDER BY MAX(updated_at_ms) DESC LIMIT 128"
    ).ok()?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0)).ok()?;
    let mut folders = Vec::new();
    let mut seen = HashSet::new();
    let profile = database_path.parent()
        .filter(|parent| parent.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(".codex")))
        .and_then(Path::parent);
    for raw in rows.flatten() {
        if folders.len() == MAX_CANDIDATES { break; }
        let Some(raw) = normalize_task_path(&raw) else { continue };
        if raw.len() > 2048 || raw.chars().any(char::is_control) { continue; }
        let path = Path::new(&raw);
        if !path.is_absolute() || is_codex_internal(path) ||
            profile.is_some_and(|home| home.starts_with(path)) ||
            !existing_plain_directory(path) { continue; }
        let Some(folder) = path.file_name().and_then(|name| name.to_str()) else { continue };
        if folder.is_empty() || folder.chars().count() > 120 || folder.chars().any(char::is_control) { continue; }
        if seen.insert(raw.to_ascii_lowercase()) { folders.push(raw); }
    }
    Some(folders)
}

fn normalize_task_path(raw: &str) -> Option<String> {
    #[cfg(windows)]
    if let Some(disk_path) = raw.strip_prefix(r"\\?\") {
        let bytes = disk_path.as_bytes();
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' ||
            (bytes[2] != b'\\' && bytes[2] != b'/') {
            return None;
        }
        return Some(disk_path.to_string());
    }
    Some(raw.to_string())
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

fn is_codex_internal(path: &Path) -> bool {
    path.components().any(|component| matches!(component, Component::Normal(name) if name.to_string_lossy().eq_ignore_ascii_case(".codex")))
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

    #[test]
    fn recent_task_folders_are_discovered_without_reading_chat_text() {
        let dir = temp_dir();
        let saved_folder = dir.join("Saved");
        let task_folder = dir.join("Recent task");
        let mirror = dir.join(".codex").join(".chatgpt-projects").join("cloud");
        fs::create_dir_all(&saved_folder).unwrap();
        fs::create_dir_all(&task_folder).unwrap();
        fs::create_dir_all(&mirror).unwrap();
        let state = dir.join("state.json");
        fs::write(&state, json!({
            "local-projects": {"one": {"id":"one", "name":"Saved game", "rootPaths":[saved_folder]}}
        }).to_string()).unwrap();
        let database = dir.join(".codex").join("state_5.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch("CREATE TABLE threads (cwd TEXT, archived INTEGER, updated_at_ms INTEGER, title TEXT)").unwrap();
        let task_raw = if cfg!(windows) {
            format!(r"\\?\{}", task_folder.display())
        } else {
            task_folder.to_string_lossy().into_owned()
        };
        for (path, when, title) in [
            (task_raw, 3, "private task text"),
            (saved_folder.to_string_lossy().into_owned(), 2, "duplicate"),
            (mirror.to_string_lossy().into_owned(), 1, "cloud mirror"),
            (dir.to_string_lossy().into_owned(), 0, "home root"),
        ] {
            connection.execute("INSERT INTO threads VALUES (?1, 0, ?2, ?3)",
                rusqlite::params![path, when, title]).unwrap();
        }
        for when in 10..140 {
            connection.execute("INSERT INTO threads VALUES (?1, 0, ?2, 'private duplicate')",
                rusqlite::params![mirror.to_str().unwrap(), when]).unwrap();
        }
        drop(connection);
        let result = discover_workspaces_from(&state, &database);
        relay_contracts::registry::validate_value(
            &relay_contracts::registry::resolve_command("codex.workspaces.discover", Some(1)).unwrap().result_schema,
            &result,
        ).unwrap();
        assert_eq!(result["status"], "available");
        assert_eq!(result["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(result["candidates"][0]["origin"], "saved_project");
        assert_eq!(result["candidates"][1]["origin"], "recent_task_folder");
        assert!(!result.to_string().contains("private task text"));
        assert!(!result.to_string().contains("cloud mirror"));
        fs::remove_dir_all(dir).unwrap();
    }
}
