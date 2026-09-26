use relay_contracts::LocalHostState;
use std::fs;
use std::path::{Path, PathBuf};

pub fn state_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("RELAY_STATE_DIR") {
        return PathBuf::from(value);
    }
    if let Some(value) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(value)
            .join("DSI")
            .join("RELAY");
    }
    std::env::temp_dir()
        .join("DSI")
        .join("RELAY")
}

pub fn state_path() -> PathBuf {
    state_dir().join("host.json")
}

pub fn write_state(
    path: &Path,
    state: &LocalHostState,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create state dir: {error}"))?;
    }
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("serialize host state: {error}"))?;
    fs::write(&temp, bytes)
        .map_err(|error| format!("write host state: {error}"))?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("publish host state: {error}"))
}

pub fn clear_state(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_contracts::{
        IpcSecurityState, ProtocolRange, LOCAL_HOST_STATE_FORMAT,
    };

    #[test]
    fn state_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "relayd-state-test-{}",
            std::process::id()
        ));
        let path = dir.join("host.json");
        let state = LocalHostState {
            state_format: LOCAL_HOST_STATE_FORMAT,
            pid: 1,
            pipe: r"\\.\pipe\fixture".to_string(),
            auth_token: "fixture-token".to_string(),
            version: "0.1.0".to_string(),
            protocol: ProtocolRange { min: 1, max: 1 },
            capabilities: vec!["system.status@1".to_string()],
            recovery_state: "Healthy".to_string(),
            storage_schema_version: Some(2),
            ipc_security: IpcSecurityState {
                transport: "windows_named_pipe".to_string(),
                explicit_dacl: true,
                kernel_acl_verified: true,
                owner_current_user: true,
                acl_ace_count: 1,
                scope: "current_user".to_string(),
                auth_token: true,
            },
            started_at_unix_ms: 1,
        };
        write_state(&path, &state).unwrap();
        let restored: LocalHostState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored.pipe, state.pipe);
        clear_state(&path);
        let _ = fs::remove_dir_all(dir);
    }
}
