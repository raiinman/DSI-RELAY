use relay::client;
use relay_contracts::{CommandRequest, LOCAL_HOST_STATE_FORMAT, RequestContext};
use serde_json::{Value, json};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const EXISTING_HOST_RETRY: Duration = Duration::from_secs(3);
const AUTH_MISMATCH: &str = "RELAY's local connection has a mismatched session key";
const DASHBOARD_NOT_READY: &str = "RELAY's app window is not ready yet";

fn validated_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, token)) = rest.split_once("/#") else {
        return false;
    };
    port.parse::<u16>().is_ok_and(|port| port > 0)
        && token.len() == 64
        && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn live_dashboard_url() -> Result<Option<String>, String> {
    let state = match client::read_state() {
        Ok(state) => state,
        Err(error) if error == "HOST_UNAVAILABLE" => return Ok(None),
        Err(error) => return Err(format!("RELAY host state is invalid: {error}")),
    };
    if state.state_format != LOCAL_HOST_STATE_FORMAT {
        return Err("RELAY engine state is from an incompatible version".to_string());
    }
    let status = client::call(
        &state,
        &CommandRequest {
            request_id: format!("LAUNCH-status-{}", std::process::id()),
            command: "system.status".to_string(),
            command_version: Some(1),
            arguments: json!({}),
            idempotency_key: None,
            context: RequestContext::default(),
        },
    );
    match status {
        Err(error) if error == "HOST_UNAVAILABLE" => return Ok(None),
        Err(error) if error == "UNAUTHORIZED" => return Err(AUTH_MISMATCH.to_string()),
        Err(error) => return Err(format!("RELAY engine is unreachable: {error}")),
        Ok(response) if !response.ok => {
            return Err("RELAY engine rejected its health check".to_string());
        }
        Ok(_) => {}
    }
    let path = client::state_dir().join("dashboard.json");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DASHBOARD_NOT_READY.to_string());
        }
        Err(_) => return Err("RELAY app link could not be read".to_string()),
    };
    let info: Value =
        serde_json::from_slice(&bytes).map_err(|_| "RELAY app link is invalid".to_string())?;
    let url = info["url"].as_str().ok_or("RELAY app link is invalid")?;
    if !validated_url(url) {
        return Err("RELAY app link is invalid".to_string());
    }
    Ok(Some(url.to_string()))
}

fn existing_dashboard_url() -> Result<Option<String>, String> {
    let deadline = Instant::now() + EXISTING_HOST_RETRY;
    loop {
        match live_dashboard_url() {
            Err(error) if error == AUTH_MISMATCH || error == DASHBOARD_NOT_READY => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "{error}. Close running RELAY engines and open RELAY again; if this continues, restart Windows"
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

fn spawn_engine() -> Result<Child, String> {
    let executable = std::env::current_exe()
        .map_err(|_| "RELAY cannot locate its installed files".to_string())?;
    let sibling = executable.with_file_name("relayd.exe");
    if !sibling.is_file() {
        return Err("RELAY engine is missing beside relay.exe; reinstall RELAY".to_string());
    }
    Command::new(&sibling)
        .current_dir(sibling.parent().ok_or("RELAY install folder is invalid")?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| "RELAY engine could not start; check the installation".to_string())
}

fn edge_executable() -> Option<PathBuf> {
    ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("Microsoft/Edge/Application/msedge.exe"))
        .find(|path| path.is_file())
}

fn open_browser(url: &str) -> Result<(), String> {
    if let Some(edge) = edge_executable() {
        if Command::new(edge)
            .arg(format!("--app={url}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        return Err("RELAY is running, but Windows could not open your browser; run `relay dashboard-url` to get the local link".to_string());
    }
    Ok(())
}

pub fn launch() -> Result<(), String> {
    if let Some(url) = existing_dashboard_url()? {
        return open_browser(&url);
    }
    let mut child = spawn_engine()?;
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(url) = existing_dashboard_url()? {
            return open_browser(&url);
        }
        if child
            .try_wait()
            .map_err(|_| "RELAY engine status could not be checked")?
            .is_some()
        {
            return Err("RELAY engine stopped during startup. Another RELAY engine may already be running; close it and try again, or run `relay doctor` for details".to_string());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(
        "RELAY engine did not become ready within 30 seconds; run `relay doctor` for details"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::validated_url;

    #[test]
    fn only_exact_loopback_dashboard_url_is_accepted() {
        let valid = format!("http://127.0.0.1:4321/#{}", "a".repeat(64));
        assert!(validated_url(&valid));
        assert!(!validated_url(&valid.replace("127.0.0.1", "evil.example")));
        assert!(!validated_url(&valid.replace("/#", "/path/#")));
        assert!(!validated_url(&valid.replace(":4321", ":0")));
        assert!(!validated_url(&valid.replace(&"a".repeat(64), "wrong")));
    }
}
