use relay_contracts::LocalHostState;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE, GENERIC_WRITE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, WriteFile, CREATE_NEW, FILE_ATTRIBUTE_NORMAL};

fn dashboard_path(dir: &Path) -> PathBuf {
    dir.join("dashboard.json")
}

pub fn write_dashboard_url(dir: &Path, url: &str) -> Result<(), String> {
    let path = dashboard_path(dir);
    let temp = dir.join(format!("dashboard-{}.tmp", crate::security::random_hex(8)?));
    let bytes = serde_json::to_vec(&serde_json::json!({ "url": url }))
        .map_err(|error| format!("serialize dashboard state: {error}"))?;
    let security = crate::security::current_user_pipe_security()?;
    let wide: Vec<u16> = temp.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE,
            0,
            &security.attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("create restricted dashboard state failed: {}", unsafe { GetLastError() }));
    }
    let mut written = 0u32;
    let ok = unsafe { WriteFile(handle, bytes.as_ptr(), bytes.len() as u32, &mut written, null_mut()) };
    unsafe { CloseHandle(handle) };
    if ok == 0 || written as usize != bytes.len() {
        let _ = fs::remove_file(&temp);
        return Err("write restricted dashboard state failed".to_string());
    }
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&temp, &path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("publish dashboard state: {error}")
    })
}

pub fn clear_dashboard_url(dir: &Path) {
    let _ = fs::remove_file(dashboard_path(dir));
}

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
    let temp = path.with_extension(format!("{}.tmp", crate::security::random_hex(8)?));
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("serialize host state: {error}"))?;
    let security = crate::security::current_user_pipe_security()?;
    let wide: Vec<u16> = temp.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE,
            0,
            &security.attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("create restricted host state failed: {}", unsafe { GetLastError() }));
    }
    let result = (|| {
        let verified = crate::security::verify_file_security(handle, &security.sid)?;
        if !verified.query_ok
            || !verified.protected_dacl
            || !verified.owner_is_current_user
            || !verified.current_user_only
            || !verified.current_user_full_control
            || verified.ace_count != 1
        {
            return Err("restricted host state ACL verification failed".to_string());
        }
        let mut written = 0u32;
        let ok = unsafe { WriteFile(handle, bytes.as_ptr(), bytes.len() as u32, &mut written, null_mut()) };
        if ok == 0 || written as usize != bytes.len() {
            return Err("write restricted host state failed".to_string());
        }
        Ok(())
    })();
    unsafe { CloseHandle(handle) };
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("publish host state: {error}")
    })
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
        let mut state = LocalHostState {
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
        let file = fs::File::open(&path).unwrap();
        let sid = crate::security::current_user_sid_string().unwrap();
        let verified = crate::security::verify_file_security(
            std::os::windows::io::AsRawHandle::as_raw_handle(&file),
            &sid,
        ).unwrap();
        assert!(verified.protected_dacl);
        assert!(verified.owner_is_current_user);
        assert!(verified.current_user_only);
        assert!(verified.current_user_full_control);
        assert_eq!(verified.ace_count, 1);
        let restored: LocalHostState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored.pipe, state.pipe);
        drop(file);
        state.auth_token = "replacement-token".to_string();
        write_state(&path, &state).unwrap();
        let replacement: LocalHostState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(replacement.auth_token, state.auth_token);
        let file = fs::File::open(&path).unwrap();
        let verified = crate::security::verify_file_security(
            std::os::windows::io::AsRawHandle::as_raw_handle(&file),
            &sid,
        ).unwrap();
        assert!(verified.current_user_only);
        drop(file);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        clear_state(&path);
        let _ = fs::remove_dir_all(dir);
    }
}
