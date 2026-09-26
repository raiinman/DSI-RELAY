use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Networking::WinSock::{
    closesocket, connect, htons, socket, WSACleanup, WSAGetLastError, WSAStartup,
    AF_INET, INVALID_SOCKET, IPPROTO_TCP, SOCKADDR, SOCKADDR_IN, SOCK_STREAM,
    SOCKET_ERROR, WSADATA,
};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenCapabilities, TokenIsAppContainer, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, OpenProcessToken, TerminateProcess,
    PROCESS_INFORMATION, STARTUPINFOW, CREATE_NO_WINDOW, CREATE_SUSPENDED,
};

#[derive(Debug, Deserialize)]
struct ProbeRequest {
    allowed_input: PathBuf,
    blocked_secret: PathBuf,
    blocked_write: PathBuf,
    read_only_write: PathBuf,
    network_target: String,
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    pid: u32,
    is_app_container: bool,
    capability_count: u32,
    allowed_read_ok: bool,
    allowed_write_ok: bool,
    read_only_write_ok: bool,
    blocked_read_ok: bool,
    blocked_write_ok: bool,
    network_connect_ok: bool,
    network_error_code: Option<i32>,
    parent_secret_visible: bool,
    user_profile_visible: bool,
    child_process_created: bool,
    child_process_error: u32,
    environment_keys: Vec<String>,
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
        "\"{}\" --child-marker",
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

fn write_response(mailbox: &Path, response: &ProbeResponse) {
    let path = mailbox.join("response.json");
    let bytes = serde_json::to_vec_pretty(response).unwrap_or_else(|_| {
        std::process::exit(209);
    });
    if fs::write(path, bytes).is_err() {
        std::process::exit(210);
    }
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


fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child-marker") {
        return;
    }
    let Some(mailbox) = args.get(1).map(PathBuf::from) else {
        std::process::exit(2);
    };
    let panic_path = mailbox.join("panic.txt");
    std::panic::set_hook(Box::new(move |info| {
        let _ = fs::write(&panic_path, info.to_string());
    }));

    let request: ProbeRequest = match fs::read(mailbox.join("request.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        Some(value) => value,
        None => std::process::exit(3),
    };

    let allowed_read_ok = fs::read_to_string(&request.allowed_input).is_ok();
    let allowed_write_ok = fs::write(mailbox.join("worker-write.txt"), b"worker-write").is_ok();
    let read_only_write_ok = fs::write(&request.read_only_write, b"should-fail").is_ok();
    let blocked_read_ok = fs::read_to_string(&request.blocked_secret).is_ok();
    let blocked_write_ok = fs::write(&request.blocked_write, b"escape").is_ok();

    let (network_connect_ok, network_error_code) =
        probe_network(&request.network_target);

    let parent_secret_visible = std::env::var("RELAY_SPIKE11_PARENT_SECRET").is_ok();
    let user_profile_visible = std::env::var("USERPROFILE").is_ok();
    let mut environment_keys: Vec<String> = std::env::vars().map(|(key, _)| key).collect();
    environment_keys.sort();

    let (child_process_created, child_process_error) = try_child_process();

    write_response(
        &mailbox,
        &ProbeResponse {
            pid: std::process::id(),
            is_app_container: is_app_container(),
            capability_count: token_capability_count(),
            allowed_read_ok,
            allowed_write_ok,
            read_only_write_ok,
            blocked_read_ok,
            blocked_write_ok,
            network_connect_ok,
            network_error_code,
            parent_secret_visible,
            user_profile_visible,
            child_process_created,
            child_process_error,
            environment_keys,
        },
    );
}
