use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub mode: String,
    pub url: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcSecurityState {
    pub explicit_dacl: bool,
    #[serde(default)]
    pub kernel_acl_verified: bool,
    #[serde(default)]
    pub acl_ace_count: u32,
    #[serde(default)]
    pub owner_current_user: bool,
    pub scope: String,
    pub auth_token: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostState {
    pub pid: u32,
    pub pipe: String,
    pub auth_token: String,
    pub version: String,
    pub protocol: ProtocolRange,
    pub capabilities: Vec<String>,
    pub recovery_state: String,
    #[serde(default)]
    pub storage_schema_version: Option<i64>,
    pub ipc_security: IpcSecurityState,
    pub dashboard: Option<DashboardState>,
    pub started_at_unix_ms: u128,
}

pub fn state_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("RELAY_STATE_DIR") {
        return PathBuf::from(value);
    }
    if let Some(value) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(value).join("DSI").join("RELAY").join("phase1-rust");
    }
    std::env::temp_dir().join("DSI").join("RELAY").join("phase1-rust")
}

pub fn state_path() -> PathBuf {
    state_dir().join("host-rust.json")
}

pub fn db_path() -> PathBuf {
    if let Some(value) = std::env::var_os("RELAY_DB_PATH") {
        return PathBuf::from(value);
    }
    state_dir().join("relay.sqlite3")
}

pub fn write_state(path: &Path, state: &HostState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create state dir: {error}"))?;
    }
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| format!("serialize state: {error}"))?;
    fs::write(&temp, bytes).map_err(|error| format!("write state: {error}"))?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp, path).map_err(|error| format!("publish state: {error}"))
}

pub fn read_state(path: &Path) -> Result<HostState, String> {
    let bytes = fs::read(path).map_err(|error| format!("read state: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse state: {error}"))
}

pub fn clear_state(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip() {
        let dir = std::env::temp_dir().join(format!("relay-rust-state-test-{}", std::process::id()));
        let path = dir.join("state.json");
        let state = HostState {
            pid: 1,
            pipe: r"\\.\pipe\fixture".to_string(),
            auth_token: "abc".to_string(),
            version: "test".to_string(),
            protocol: ProtocolRange { min: 1, max: 1 },
            capabilities: vec!["system.status@1".to_string()],
            recovery_state: "Healthy".to_string(),
            storage_schema_version: Some(1),
            ipc_security: IpcSecurityState {
                explicit_dacl: true,
                kernel_acl_verified: true,
                acl_ace_count: 1,
                owner_current_user: true,
                scope: "current_user".to_string(),
                auth_token: true,
            },
            dashboard: None,
            started_at_unix_ms: 1,
        };
        write_state(&path, &state).unwrap();
        let restored = read_state(&path).unwrap();
        assert_eq!(restored.pipe, state.pipe);
        clear_state(&path);
        let _ = fs::remove_dir_all(dir);
    }
}
