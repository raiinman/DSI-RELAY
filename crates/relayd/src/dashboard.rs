use crate::parser::ParserHealth;
use crate::runtime_context;
use relay_contracts::{CommandRequest, IpcSecurityState, RequestContext};
use relay_core::policy::ExecutionAuthority;
use relay_core::service::RelayCore;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const INDEX: &str = include_str!("../dashboard/index.html");
const SCRIPT: &str = include_str!("../dashboard/app.js");
const STYLE: &str = include_str!("../dashboard/styles.css");
const MAX_HEADER: usize = 8192;
const MAX_BODY: usize = 32 * 1024;

pub struct DashboardServer {
    address: SocketAddr,
    token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DashboardServer {
    pub fn start(
        core: Arc<RelayCore>,
        started: Instant,
        ipc_security: IpcSecurityState,
        parser_health: ParserHealth,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("dashboard loopback bind failed: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("dashboard listener setup failed: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("dashboard local address failed: {error}"))?;
        let token = crate::security::random_hex(32)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_token = token.clone();
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let reply = match read_request(&mut stream) {
                            Ok(request) => handle_request(
                                request,
                                address,
                                &thread_token,
                                &core,
                                started,
                                &ipc_security,
                                &parser_health,
                            ),
                            Err(_) => response(
                                400,
                                "Bad Request",
                                "text/plain; charset=utf-8",
                                b"Invalid request",
                            ),
                        };
                        let _ = stream.write_all(&reply);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            address,
            token,
            stop,
            thread: Some(thread),
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/#{}", self.address.port(), self.token)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for DashboardServer {
    fn drop(&mut self) {
        self.stop();
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ()> {
    let mut bytes = Vec::with_capacity(2048);
    let header_end = loop {
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
        if bytes.len() >= MAX_HEADER {
            return Err(());
        }
        let mut chunk = [0u8; 2048];
        let count = stream.read(&mut chunk).map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    if header_end > MAX_HEADER {
        return Err(());
    }
    let head = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| ())?
        .to_string();
    let mut lines = head.split("\r\n");
    let line = lines.next().ok_or(())?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(());
    }
    let mut headers = HashMap::new();
    for line in lines {
        let (key, value) = line.split_once(':').ok_or(())?;
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() || headers.insert(key, value.trim().to_string()).is_some() {
            return Err(());
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(());
    }
    let length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().map_err(|_| ()))
        .transpose()?
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err(());
    }
    let body_start = header_end + 4;
    while bytes.len() < body_start + length {
        let mut chunk = [0u8; 2048];
        let count = stream.read(&mut chunk).map_err(|_| ())?;
        if count == 0 || bytes.len() + count > MAX_HEADER + MAX_BODY + 4 {
            return Err(());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(HttpRequest {
        method: parts[0].to_string(),
        path: parts[1].to_string(),
        headers,
        body: bytes[body_start..body_start + length].to_vec(),
    })
}

fn response(status: u16, reason: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        body.len()
    );
    [head.as_bytes(), body].concat()
}

fn json_response(status: u16, reason: &str, value: Value) -> Vec<u8> {
    let body = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    response(status, reason, "application/json; charset=utf-8", &body)
}

fn error(status: u16, reason: &str, code: &str) -> Vec<u8> {
    json_response(
        status,
        reason,
        json!({ "ok": false, "error": { "code": code } }),
    )
}

fn allowed_command(command: &str) -> bool {
    matches!(
        command,
        "system.status"
            | "system.doctor"
            | "diagnostics.summary"
            | "project.list"
            | "project.capabilities"
            | "uefn.static.inspect"
            | "uefn.mcp.discover"
            | "assets.manifest.validate"
            | "assets.krita.inspect"
            | "usage.summary"
            | "transaction.list"
            | "result.list"
    ) && relay_contracts::registry::is_surface_exposed(command, "dashboard")
}

fn handle_request(
    request: HttpRequest,
    address: SocketAddr,
    token: &str,
    core: &RelayCore,
    started: Instant,
    ipc_security: &IpcSecurityState,
    parser_health: &ParserHealth,
) -> Vec<u8> {
    let expected_host = format!("127.0.0.1:{}", address.port());
    if request.headers.get("host").map(String::as_str) != Some(expected_host.as_str()) {
        return error(403, "Forbidden", "DASHBOARD_HOST_REJECTED");
    }
    let expected_origin = format!("http://{expected_host}");
    if request
        .headers
        .get("origin")
        .is_some_and(|value| value != &expected_origin)
    {
        return error(403, "Forbidden", "DASHBOARD_ORIGIN_REJECTED");
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => response(200, "OK", "text/html; charset=utf-8", INDEX.as_bytes()),
        ("GET", "/app.js") => response(
            200,
            "OK",
            "text/javascript; charset=utf-8",
            SCRIPT.as_bytes(),
        ),
        ("GET", "/styles.css") => response(200, "OK", "text/css; charset=utf-8", STYLE.as_bytes()),
        ("POST", "/api/execute") => {
            if !crate::secure_equal(
                request
                    .headers
                    .get("x-relay-dashboard-token")
                    .map(String::as_str)
                    .unwrap_or(""),
                token,
            ) {
                return error(401, "Unauthorized", "DASHBOARD_UNAUTHORIZED");
            }
            if request.headers.get("content-type").map(String::as_str) != Some("application/json") {
                return error(415, "Unsupported Media Type", "DASHBOARD_CONTENT_TYPE");
            }
            let value: Value = match serde_json::from_slice(&request.body) {
                Ok(value) => value,
                Err(_) => return error(400, "Bad Request", "DASHBOARD_BAD_JSON"),
            };
            let Some(command) = value.get("command").and_then(Value::as_str) else {
                return error(400, "Bad Request", "DASHBOARD_COMMAND_REQUIRED");
            };
            if !allowed_command(command) {
                return error(403, "Forbidden", "DASHBOARD_COMMAND_NOT_EXPOSED");
            }
            let arguments = value.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let command_request = CommandRequest {
                request_id: format!("DASH-{}-{now}", std::process::id()),
                command: command.to_string(),
                command_version: Some(1),
                arguments,
                idempotency_key: None,
                context: RequestContext::default(),
            };
            let runtime = runtime_context(
                started,
                ipc_security,
                core,
                parser_health,
                matches!(command, "system.status" | "system.doctor"),
            );
            let authority = ExecutionAuthority::local_user("relay-dashboard");
            let result = core.execute_authorized_with_extension(
                command_request,
                &runtime,
                &authority,
                |request| {
                    crate::uefn::execute(core, request)
                        .or_else(|| crate::assets::execute(core, request))
                        .or_else(|| crate::verse::execute(core, request))
                },
            );
            json_response(
                200,
                "OK",
                serde_json::to_value(result).unwrap_or_else(|_| json!({})),
            )
        }
        _ => error(404, "Not Found", "DASHBOARD_ROUTE_NOT_FOUND"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::service::CoreConfig;

    fn http(
        port: u16,
        method: &str,
        path: &str,
        token: Option<&str>,
        origin: Option<&str>,
        body: &str,
    ) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n",
            body.len()
        );
        if let Some(token) = token {
            head.push_str(&format!(
                "X-Relay-Dashboard-Token: {token}\r\nContent-Type: application/json\r\n"
            ));
        }
        if let Some(origin) = origin {
            head.push_str(&format!("Origin: {origin}\r\n"));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
        let mut result = String::new();
        stream.read_to_string(&mut result).unwrap();
        result
    }

    #[test]
    fn dashboard_command_allowlist_is_read_only() {
        assert!(allowed_command("system.status"));
        assert!(allowed_command("project.list"));
        assert!(!allowed_command("system.shutdown"));
        assert!(!allowed_command("project.register"));
        assert!(!allowed_command("result.put"));
    }

    #[test]
    fn response_has_no_store_and_frame_denial() {
        let raw = String::from_utf8(response(200, "OK", "text/plain", b"ok")).unwrap();
        assert!(raw.contains("Cache-Control: no-store"));
        assert!(raw.contains("X-Frame-Options: DENY"));
    }

    #[test]
    fn live_dashboard_rejects_wrong_token_cross_origin_and_writes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "relayd-dashboard-test-{}-{unique}",
            std::process::id()
        ));
        let core = Arc::new(RelayCore::open(CoreConfig::new(&dir)));
        let ipc = IpcSecurityState {
            transport: "windows_named_pipe".to_string(),
            explicit_dacl: true,
            kernel_acl_verified: true,
            owner_current_user: true,
            acl_ace_count: 1,
            scope: "current_user".to_string(),
            auth_token: true,
        };
        let parser = ParserHealth::new(&Default::default(), false);
        let mut server =
            DashboardServer::start(Arc::clone(&core), Instant::now(), ipc, parser).unwrap();
        let port = server.address.port();
        let status_body = r#"{"command":"system.status","arguments":{}}"#;
        let no_token = http(port, "POST", "/api/execute", None, None, status_body);
        assert!(no_token.starts_with("HTTP/1.1 401"));
        let wrong_origin = http(
            port,
            "POST",
            "/api/execute",
            Some(&server.token),
            Some("http://example.invalid"),
            status_body,
        );
        assert!(wrong_origin.starts_with("HTTP/1.1 403"));
        let write = http(
            port,
            "POST",
            "/api/execute",
            Some(&server.token),
            None,
            r#"{"command":"system.shutdown","arguments":{}}"#,
        );
        assert!(write.contains("DASHBOARD_COMMAND_NOT_EXPOSED"));
        let status = http(
            port,
            "POST",
            "/api/execute",
            Some(&server.token),
            None,
            status_body,
        );
        assert!(status.starts_with("HTTP/1.1 200"));
        assert!(status.contains("\"recovery_state\""));
        server.stop();
        drop(core);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
