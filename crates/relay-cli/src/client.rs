use relay_contracts::{
    CommandRequest, CommandResponse, HelloRequest, HelloResponse,
    LocalHostState, Producer, PROTOCOL_MAX, PROTOCOL_MIN,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND,
    ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, OPEN_EXISTING,
};
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0) };
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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

pub fn read_state() -> Result<LocalHostState, String> {
    let path = state_dir().join("host.json");
    let bytes = fs::read(&path)
        .map_err(|_| "HOST_UNAVAILABLE".to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse host state: {error}"))
}

fn open_client(
    pipe_name: &str,
    timeout_ms: u32,
) -> Result<OwnedHandle, String> {
    let pipe = wide(pipe_name);
    let deadline =
        std::time::Instant::now() + Duration::from_millis(timeout_ms as u64);
    loop {
        let handle = unsafe {
            CreateFileW(
                pipe.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            return Ok(OwnedHandle(handle));
        }

        let error = unsafe { GetLastError() };
        if error == ERROR_PIPE_BUSY
            && std::time::Instant::now() < deadline
        {
            unsafe {
                WaitNamedPipeW(pipe.as_ptr(), 25);
            }
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        if error == ERROR_FILE_NOT_FOUND {
            return Err("HOST_UNAVAILABLE".to_string());
        }
        return Err(format!("open local pipe failed: {error}"));
    }
}

fn read_line(
    handle: HANDLE,
    max_bytes: usize,
) -> Result<String, String> {
    let mut output = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];

    loop {
        let mut read = 0u32;
        let ok = unsafe {
            ReadFile(
                handle,
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                &mut read,
                null_mut(),
            )
        };
        if ok == 0 {
            let error = unsafe { GetLastError() };
            if error == ERROR_BROKEN_PIPE {
                return Err("pipe closed".to_string());
            }
            return Err(format!("read local pipe failed: {error}"));
        }
        if read == 0 {
            return Err("pipe returned zero bytes".to_string());
        }

        for byte in &chunk[..read as usize] {
            if *byte == b'\n' {
                return String::from_utf8(output)
                    .map_err(|error| format!("pipe utf8: {error}"));
            }
            output.push(*byte);
            if output.len() > max_bytes {
                return Err("pipe message too large".to_string());
            }
        }
    }
}

fn write_line(handle: HANDLE, value: &str) -> Result<(), String> {
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(b'\n');
    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                handle,
                bytes[offset..].as_ptr(),
                (bytes.len() - offset) as u32,
                &mut written,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "write local pipe failed: {}",
                unsafe { GetLastError() }
            ));
        }
        if written == 0 {
            return Err("pipe wrote zero bytes".to_string());
        }
        offset += written as usize;
    }
    Ok(())
}

fn write_json<T: serde::Serialize>(
    handle: HANDLE,
    value: &T,
) -> Result<(), String> {
    let text = serde_json::to_string(value)
        .map_err(|error| format!("serialize request: {error}"))?;
    write_line(handle, &text)
}

fn read_value(handle: HANDLE) -> Result<Value, String> {
    let line = read_line(handle, 64 * 1024)?;
    serde_json::from_str(&line)
        .map_err(|error| format!("parse response: {error}"))
}

pub fn call(
    state: &LocalHostState,
    request: &CommandRequest,
) -> Result<CommandResponse, String> {
    let handle = open_client(&state.pipe, 3000)?;
    let hello = HelloRequest {
        message_type: "hello".to_string(),
        auth_token: state.auth_token.clone(),
        protocol_min: PROTOCOL_MIN,
        protocol_max: PROTOCOL_MAX,
        client: Producer {
            name: "relay-cli".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    write_json(handle.raw(), &hello)?;
    let greeting = read_value(handle.raw())?;
    if greeting
        .get("type")
        .and_then(Value::as_str)
        != Some("hello_ok")
    {
        return Err(greeting
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("BAD_HANDSHAKE")
            .to_string());
    }
    let _: HelloResponse = serde_json::from_value(greeting)
        .map_err(|error| format!("parse hello response: {error}"))?;

    write_json(handle.raw(), request)?;
    let value = read_value(handle.raw())?;
    serde_json::from_value(value)
        .map_err(|error| format!("parse command response: {error}"))
}
