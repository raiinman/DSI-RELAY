use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Networking::WinSock::{
    closesocket, connect, htons, socket, WSACleanup, WSAGetLastError,
    WSAStartup, AF_INET, INVALID_SOCKET, IPPROTO_TCP, SOCKADDR,
    SOCKADDR_IN, SOCK_STREAM, SOCKET_ERROR, WSADATA,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenCapabilities, TokenIsAppContainer,
    TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, OpenProcessToken,
    TerminateProcess, PROCESS_INFORMATION, STARTUPINFOW,
    CREATE_NO_WINDOW, CREATE_SUSPENDED,
};

#[derive(Debug, Deserialize)]
struct WorkerRequest {
    request_id: String,
    adapter_id: String,
    adapter_version: String,
    protocol: u32,
    capabilities: Vec<String>,
    command: String,
    command_version: u32,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Serialize)]
struct WorkerResponse {
    #[serde(rename = "type")]
    message_type: &'static str,
    request_id: String,
    adapter_id: String,
    adapter_version: String,
    protocol: u32,
    pid: u32,
    capabilities: Vec<String>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn is_app_container() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut value = 0u32;
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenIsAppContainer,
            &mut value as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
            &mut returned,
        );
        CloseHandle(token);
        ok != 0 && value != 0
    }
}

fn token_capability_count() -> u32 {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return 0;
        }

        let mut needed = 0u32;
        let _ = GetTokenInformation(
            token,
            TokenCapabilities,
            null_mut(),
            0,
            &mut needed,
        );
        if needed < std::mem::size_of::<u32>() as u32 {
            CloseHandle(token);
            return 0;
        }

        let mut buffer = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            token,
            TokenCapabilities,
            buffer.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return 0;
        }
        *(buffer.as_ptr() as *const u32)
    }
}

fn try_child_process() -> (bool, u32) {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return (false, 0),
    };
    let mut command_line = wide(&format!(
        r#""{}" --child-marker"#,
        exe.to_string_lossy()
    ));
    let mut startup = STARTUPINFOW::default();
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut process = PROCESS_INFORMATION::default();
    let ok = unsafe {
        CreateProcessW(
            null(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_NO_WINDOW | CREATE_SUSPENDED,
            null(),
            null(),
            &startup,
            &mut process,
        )
    };
    if ok == 0 {
        return (false, unsafe { GetLastError() });
    }

    unsafe {
        TerminateProcess(process.hProcess, 0);
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    (true, 0)
}

fn probe_network(target: &str) -> (bool, Option<i32>) {
    let Some((host, port_text)) = target.rsplit_once(':') else {
        return (false, None);
    };
    let octets: Vec<u8> = host
        .split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect();
    let Ok(port) = port_text.parse::<u16>() else {
        return (false, None);
    };
    if octets.len() != 4 {
        return (false, None);
    }

    unsafe {
        let mut data = WSADATA::default();
        let startup = WSAStartup(0x0202, &mut data);
        if startup != 0 {
            return (false, Some(startup));
        }

        let sock = socket(AF_INET as i32, SOCK_STREAM, IPPROTO_TCP);
        if sock == INVALID_SOCKET {
            let error = WSAGetLastError();
            WSACleanup();
            return (false, Some(error));
        }

        let mut address = SOCKADDR_IN::default();
        address.sin_family = AF_INET;
        address.sin_port = htons(port);
        address.sin_addr.S_un.S_un_b.s_b1 = octets[0];
        address.sin_addr.S_un.S_un_b.s_b2 = octets[1];
        address.sin_addr.S_un.S_un_b.s_b3 = octets[2];
        address.sin_addr.S_un.S_un_b.s_b4 = octets[3];

        let connected = connect(
            sock,
            &address as *const SOCKADDR_IN as *const SOCKADDR,
            std::mem::size_of::<SOCKADDR_IN>() as i32,
        );
        let result = if connected == SOCKET_ERROR {
            (false, Some(WSAGetLastError()))
        } else {
            (true, None)
        };
        closesocket(sock);
        WSACleanup();
        result
    }
}

fn write_response(mailbox: &Path, response: &WorkerResponse) {
    let bytes = serde_json::to_vec_pretty(response)
        .unwrap_or_else(|_| std::process::exit(210));
    if fs::write(mailbox.join("response.json"), bytes).is_err() {
        std::process::exit(211);
    }
}

fn fixture_mode(arguments: &Value) -> &str {
    arguments
        .get("_fixture_mode")
        .and_then(Value::as_str)
        .unwrap_or("normal")
}

fn security_probe(arguments: &Value, mailbox: &Path) -> Value {
    let blocked_secret = arguments
        .get("blocked_secret")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let blocked_write = arguments
        .get("blocked_write")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let read_only_write = arguments
        .get("read_only_write")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let network_target = arguments
        .get("network_target")
        .and_then(Value::as_str)
        .unwrap_or("1.1.1.1:443");

    let blocked_read_ok = blocked_secret
        .as_deref()
        .map(|path| fs::read_to_string(path).is_ok())
        .unwrap_or(false);
    let blocked_write_ok = blocked_write
        .as_deref()
        .map(|path| fs::write(path, b"escape").is_ok())
        .unwrap_or(false);
    let read_only_write_ok = read_only_write
        .as_deref()
        .map(|path| fs::write(path, b"should-fail").is_ok())
        .unwrap_or(false);
    let allowed_write_ok =
        fs::write(mailbox.join("worker-write.txt"), b"worker-write").is_ok();
    let (network_connect_ok, network_error_code) =
        probe_network(network_target);
    let (child_process_created, child_process_error) = try_child_process();

    let mut allocation = Vec::<u8>::new();
    let memory_reservation_succeeded =
        allocation.try_reserve_exact(128 * 1024 * 1024).is_ok();

    let mut environment_keys: Vec<String> =
        std::env::vars().map(|(key, _)| key).collect();
    environment_keys.sort();

    json!({
        "is_app_container": is_app_container(),
        "capability_count": token_capability_count(),
        "allowed_mailbox_write_ok": allowed_write_ok,
        "blocked_read_ok": blocked_read_ok,
        "blocked_write_ok": blocked_write_ok,
        "read_only_write_ok": read_only_write_ok,
        "network_connect_ok": network_connect_ok,
        "network_error_code": network_error_code,
        "parent_secret_visible":
            std::env::var("RELAY_ADAPTER_PARENT_SECRET").is_ok(),
        "user_profile_visible": std::env::var("USERPROFILE").is_ok(),
        "child_process_created": child_process_created,
        "child_process_error": child_process_error,
        "memory_reservation_succeeded": memory_reservation_succeeded,
        "environment_keys": environment_keys
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child-marker") {
        return;
    }
    let Some(mailbox) = args.get(1).map(PathBuf::from) else {
        std::process::exit(2);
    };

    let request: WorkerRequest = match fs::read(mailbox.join("request.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        Some(value) => value,
        None => std::process::exit(3),
    };

    if request.command.trim().is_empty() || request.command_version == 0 {
        std::process::exit(4);
    }

    let mode = fixture_mode(&request.arguments).to_string();
    match mode.as_str() {
        "crash" => std::process::exit(17),
        "hang" => {
            thread::sleep(Duration::from_secs(5));
            return;
        }
        "invalid_json" => {
            let _ = fs::write(mailbox.join("response.json"), b"not-json");
            return;
        }
        _ => {}
    }

    let mut adapter_id = request.adapter_id.clone();
    let mut adapter_version = request.adapter_version.clone();
    let mut protocol = request.protocol;
    let mut capabilities = request.capabilities.clone();

    if mode == "identity_mismatch" {
        adapter_id = "different.adapter".to_string();
    }
    if mode == "version_mismatch" {
        adapter_version = "999.0.0".to_string();
    }
    if mode == "bad_protocol" {
        protocol = 999;
    }
    if mode == "extra_capability" {
        capabilities.push("undeclared.extra".to_string());
    }

    let (ok, result, error) = if request.command == "adapter.dependencies.parse" {
        match request.arguments["content_utf8"]
            .as_str()
            .and_then(|content| serde_json::from_str::<Value>(content).ok())
        {
            Some(document) => (
                true,
                Some(json!({
                    "source_path": if document["mode"] == "bad_source" {
                        "different.txt"
                    } else {
                        request.arguments["source_path"].as_str().unwrap_or("")
                    },
                    "source_sha256": request.arguments["source_sha256"],
                    "targets": document.get("targets").cloned().unwrap_or(json!([]))
                })),
                None,
            ),
            None => (
                false,
                None,
                Some(json!({
                    "code": "ADAPTER_PARSE_FAILED",
                    "message": "fixture dependency document is invalid"
                })),
            ),
        }
    } else { match mode.as_str() {
        "bad_result" => (
            true,
            Some(json!({ "not_echo": true })),
            None,
        ),
        "undeclared_error" => (
            false,
            None,
            Some(json!({
                "code": "WORKER_ESCALATE",
                "message": "synthetic undeclared adapter error"
            })),
        ),
        "security_probe" => (
            true,
            Some(json!({
                "echo": security_probe(&request.arguments, &mailbox)
            })),
            None,
        ),
        _ => (
            true,
            Some(json!({
                "echo": request.arguments
            })),
            None,
        ),
    }};

    write_response(
        &mailbox,
        &WorkerResponse {
            message_type: "adapter_result",
            request_id: request.request_id,
            adapter_id,
            adapter_version,
            protocol,
            pid: std::process::id(),
            capabilities,
            ok,
            result,
            error,
        },
    );
}
