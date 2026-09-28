//! Local launcher for the read-only gateway. No token is accepted through
//! arguments or environment variables and no token is printed.

use relay_gateway::{Gateway, GatewayConfig, LocalDaemonTransport, run_stdio};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom, CRYPT_INTEGER_BLOB,
    CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows_sys::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};

const PORT: u16 = 8765;
const AUTH_FILE: &str = "gateway.auth.dpapi";
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalCredential {
    format_version: u32,
    port: u16,
    token: String,
}

struct PublishedCredential(PathBuf);

impl Drop for PublishedCredential {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn credential_path() -> Result<PathBuf, &'static str> {
    let base = env::var_os("LOCALAPPDATA").ok_or("Local app data is unavailable")?;
    Ok(PathBuf::from(base)
        .join("DSI")
        .join("RELAY")
        .join(AUTH_FILE))
}

fn generate_token() -> Result<String, &'static str> {
    let mut random = [0u8; 32];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            random.as_mut_ptr(),
            random.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err("Secure token generation failed");
    }
    let mut token = String::with_capacity(64);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}").map_err(|_| "Secure token generation failed")?;
    }
    Ok(token)
}

fn protect(plaintext: &mut [u8]) -> Result<Vec<u8>, &'static str> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("Local credential protection failed");
    }
    let encrypted =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(encrypted)
}

fn unprotect(encrypted: &mut [u8]) -> Result<Vec<u8>, &'static str> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("Local credential could not be opened");
    }
    let plaintext =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(plaintext)
}

fn publish_credential(token: &str) -> Result<PublishedCredential, &'static str> {
    let path = credential_path()?;
    let folder = path.parent().ok_or("Local app data is unavailable")?;
    fs::create_dir_all(folder).map_err(|_| "Local credential directory is unavailable")?;
    let credential = LocalCredential {
        format_version: 1,
        port: PORT,
        token: token.to_owned(),
    };
    let mut plaintext =
        serde_json::to_vec(&credential).map_err(|_| "Local credential could not be encoded")?;
    let encrypted = protect(&mut plaintext);
    plaintext.fill(0);
    let encrypted = encrypted?;
    // The fixed loopback port is already exclusively bound before this runs.
    // A stale encrypted blob from a crashed prior run can now be replaced.
    if path.exists() {
        fs::remove_file(&path).map_err(|_| "Stale local credential could not be replaced")?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "Local credential could not be created")?;
    if file
        .write_all(&encrypted)
        .and_then(|_| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&path);
        return Err("Local credential could not be saved");
    }
    Ok(PublishedCredential(path))
}

fn read_credential() -> Result<LocalCredential, &'static str> {
    let path = credential_path()?;
    let metadata = fs::metadata(&path).map_err(|_| "Gateway credential is unavailable")?;
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err("Gateway credential is invalid");
    }
    let mut encrypted = fs::read(path).map_err(|_| "Gateway credential is unavailable")?;
    let mut plaintext = unprotect(&mut encrypted)?;
    let credential: LocalCredential =
        serde_json::from_slice(&plaintext).map_err(|_| "Gateway credential is invalid")?;
    plaintext.fill(0);
    if credential.format_version != 1
        || credential.port != PORT
        || credential.token.len() != 64
        || !credential
            .token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Gateway credential is invalid");
    }
    Ok(credential)
}

unsafe extern "system" fn console_handler(event: u32) -> i32 {
    if matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT) {
        SHUTDOWN.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}

fn serve() -> Result<(), &'static str> {
    let token = generate_token()?;
    let config = GatewayConfig::new(token.as_bytes().to_vec(), PORT)
        .map_err(|_| "Gateway configuration is invalid")?;
    let gateway = Gateway::bind(config, Arc::new(LocalDaemonTransport))
        .map_err(|_| "The local gateway port is unavailable")?;
    if unsafe { SetConsoleCtrlHandler(Some(console_handler), 1) } == 0 {
        return Err("Gateway shutdown handler could not be installed");
    }
    let _credential = publish_credential(&token)?;
    println!("RELAY read-only gateway on 127.0.0.1:{PORT}. Press Ctrl+C to stop.");
    gateway
        .serve_until(&SHUTDOWN)
        .map_err(|_| "Gateway listener stopped unexpectedly")
}

fn request_gateway(path: &str, body: &str) -> Result<Value, &'static str> {
    let credential = read_credential()?;
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, credential.port))
        .map_err(|_| "Gateway is not running")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "Gateway connection failed")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "Gateway connection failed")?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        credential.token,
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| "Gateway request failed")?;
    let mut response = Vec::new();
    stream
        .take(68 * 1024 + 1)
        .read_to_end(&mut response)
        .map_err(|_| "Gateway response failed")?;
    if response.len() > 68 * 1024 || !response.starts_with(b"HTTP/1.1 200 ") {
        return Err("Gateway request is unavailable");
    }
    let separator = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .ok_or("Gateway response is invalid")?;
    serde_json::from_slice(&response[separator + 4..]).map_err(|_| "Gateway response is invalid")
}

fn probe() -> Result<(), &'static str> {
    let body = request_gateway("/v1/negotiate", r#"{"protocol_min":1,"protocol_max":1}"#)?;
    if body["gateway_protocol"].as_u64() != Some(1)
        || body["network_scope"].as_str() != Some("loopback_only")
        || !body["commands"].is_array()
    {
        return Err("Gateway response is invalid");
    }
    println!("RELAY local discovery gateway is reachable.");
    Ok(())
}

fn valid_name(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn discover(command: &str, name: &str) -> Result<(), &'static str> {
    let arguments = match command {
        "list" if valid_name(name, 64) => json!({"surface":"ai","prefix":name,"limit":20}),
        "describe" if valid_name(name, 128) => json!({"command":name}),
        _ => return Err("Discovery name must be 1 to 64/128 ASCII name characters"),
    };
    let command = if command == "list" {
        "registry.list"
    } else {
        "registry.describe"
    };
    let request = json!({
        "gateway_protocol": 1,
        "command": command,
        "command_version": 1,
        "arguments": arguments,
    })
    .to_string();
    let body = request_gateway("/v1/execute", &request)?;
    if body["ok"].as_bool() != Some(true) {
        return Err("Gateway discovery request did not succeed");
    }
    let result = body.get("result").ok_or("Gateway response is invalid")?;
    println!(
        "{}",
        serde_json::to_string_pretty(result).map_err(|_| "Gateway response is invalid")?
    );
    Ok(())
}

fn read_project_result(
    command: &str,
    project_id: &str,
    result_id: Option<&str>,
    budget: Option<&str>,
) -> Result<(), &'static str> {
    if !valid_name(project_id, 128) || result_id.is_some_and(|id| !valid_name(id, 128)) {
        return Err("Project and result IDs must be bounded ASCII identifiers");
    }
    let (shared_command, arguments) = match command {
        "results" if result_id.is_none() && budget.is_none() => {
            ("result.list", json!({"project_id":project_id,"limit":20}))
        }
        "result" if result_id.is_some() && budget.is_none() => (
            "result.describe",
            json!({"project_id":project_id,"result_id":result_id}),
        ),
        "context" if result_id.is_some() => {
            let max_bytes = budget
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| (512..=4096).contains(value))
                .ok_or("Context budget must be 512 to 4096 bytes")?;
            (
                "result.context",
                json!({"project_id":project_id,"result_id":result_id,"max_bytes":max_bytes}),
            )
        }
        _ => return Err("Invalid read-only result request"),
    };
    let request = json!({
        "gateway_protocol": 1,
        "command": shared_command,
        "command_version": 1,
        "arguments": arguments,
    })
    .to_string();
    let body = request_gateway("/v1/execute", &request)?;
    if body["ok"].as_bool() != Some(true) {
        return Err("Gateway result read did not succeed");
    }
    let result = body.get("result").ok_or("Gateway response is invalid")?;
    println!(
        "{}",
        serde_json::to_string_pretty(result).map_err(|_| "Gateway response is invalid")?
    );
    Ok(())
}

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => serve(),
        [command] if command == "stdio" => {
            run_stdio(std::io::stdin().lock(), std::io::stdout().lock())
                .map_err(|_| "Local MCP stdio transport failed")
        }
        [command] if command == "probe" => probe(),
        [command, name] if command == "list" || command == "describe" => discover(command, name),
        [command, project_id] if command == "results" => {
            read_project_result(command, project_id, None, None)
        }
        [command, project_id, result_id] if command == "result" => {
            read_project_result(command, project_id, Some(result_id), None)
        }
        [command, project_id, result_id, budget] if command == "context" => {
            read_project_result(command, project_id, Some(result_id), Some(budget))
        }
        _ => Err(
            "usage: relay-gateway [stdio | probe | list PREFIX | describe COMMAND | results PROJECT | result PROJECT RESULT | context PROJECT RESULT BYTES]",
        ),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_round_trips_through_user_dpapi() {
        let token = generate_token().unwrap();
        assert_eq!(token.len(), 64);
        let mut original = token.as_bytes().to_vec();
        let mut encrypted = protect(&mut original).unwrap();
        assert_ne!(encrypted, original);
        let recovered = unprotect(&mut encrypted).unwrap();
        assert_eq!(recovered, original);
    }
}
