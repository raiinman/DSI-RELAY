use crate::security::PipeSecurity;
use serde_json::Value;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    pub fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn create_server(pipe_name: &str, security: &PipeSecurity) -> Result<OwnedHandle, String> {
    let pipe = wide(pipe_name);
    let handle = unsafe {
        CreateNamedPipeW(
            pipe.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            &security.attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("CreateNamedPipeW failed: {}", unsafe { GetLastError() }));
    }
    Ok(OwnedHandle(handle))
}

pub fn wait_for_client(handle: HANDLE) -> Result<(), String> {
    let connected = unsafe { ConnectNamedPipe(handle, null_mut()) };
    if connected != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    if error == ERROR_PIPE_CONNECTED {
        Ok(())
    } else {
        Err(format!("ConnectNamedPipe failed: {error}"))
    }
}

pub fn disconnect(handle: HANDLE) {
    unsafe {
        FlushFileBuffers(handle);
        DisconnectNamedPipe(handle);
    }
}

pub fn read_line(handle: HANDLE, max_bytes: usize) -> Result<String, String> {
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
            return Err(format!("ReadFile failed: {error}"));
        }
        if read == 0 {
            return Err("pipe returned zero bytes".to_string());
        }

        for byte in &chunk[..read as usize] {
            if *byte == b'\n' {
                return String::from_utf8(output).map_err(|error| format!("pipe utf8: {error}"));
            }
            output.push(*byte);
            if output.len() > max_bytes {
                return Err("pipe message too large".to_string());
            }
        }
    }
}

pub fn write_line(handle: HANDLE, value: &str) -> Result<(), String> {
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
            return Err(format!("WriteFile failed: {}", unsafe { GetLastError() }));
        }
        if written == 0 {
            return Err("WriteFile wrote zero bytes".to_string());
        }
        offset += written as usize;
    }
    Ok(())
}

pub fn write_json(handle: HANDLE, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string(value).map_err(|error| format!("serialize pipe json: {error}"))?;
    write_line(handle, &text)
}

pub fn read_json(handle: HANDLE) -> Result<Value, String> {
    let line = read_line(handle, 64 * 1024)?;
    serde_json::from_str(&line).map_err(|error| format!("parse pipe json: {error}"))
}

fn open_client(pipe_name: &str, timeout_ms: u32) -> Result<OwnedHandle, String> {
    let pipe = wide(pipe_name);
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms as u64);

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
        if error == ERROR_PIPE_BUSY && std::time::Instant::now() < deadline {
            unsafe {
                WaitNamedPipeW(pipe.as_ptr(), 25);
            }
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        if error == ERROR_FILE_NOT_FOUND {
            return Err("HOST_UNAVAILABLE".to_string());
        }
        return Err(format!("CreateFileW failed: {error}"));
    }
}

pub fn call(
    pipe_name: &str,
    auth_token: &str,
    command: &str,
    arguments: Value,
    timeout_ms: u32,
) -> Result<Value, String> {
    call_versioned(
        pipe_name,
        auth_token,
        command,
        None,
        arguments,
        timeout_ms,
    )
}

pub fn call_versioned(
    pipe_name: &str,
    auth_token: &str,
    command: &str,
    command_version: Option<u32>,
    arguments: Value,
    timeout_ms: u32,
) -> Result<Value, String> {
    let handle = open_client(pipe_name, timeout_ms)?;

    let hello = serde_json::json!({
        "type": "hello",
        "auth_token": auth_token,
        "protocol_min": crate::PROTOCOL_MIN,
        "protocol_max": crate::PROTOCOL_MAX,
        "client": { "name": "relay-rust-cli", "version": crate::RELAY_VERSION }
    });
    write_json(handle.raw(), &hello)?;
    let greeting = read_json(handle.raw())?;
    if greeting.get("type").and_then(Value::as_str) != Some("hello_ok") {
        return Err(greeting
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("BAD_HANDSHAKE")
            .to_string());
    }

    let request_id = format!(
        "rust-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let message = serde_json::json!({
        "type": "command",
        "request_id": request_id,
        "schema_version": crate::SCHEMA_VERSION,
        "command": command,
        "command_version": command_version,
        "arguments": arguments
    });
    write_json(handle.raw(), &message)?;
    read_json(handle.raw())
}
