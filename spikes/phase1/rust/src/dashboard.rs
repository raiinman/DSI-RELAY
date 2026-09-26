use crate::protocol::{dispatch_command, HostContext};
use crate::registry;
use crate::security::random_hex;
use crate::state::DashboardState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const INDEX_TEMPLATE: &str = include_str!("../../node/dashboard/index.html");
const APP_JS: &str = include_str!("../../node/dashboard/app.js");
const STYLES_CSS: &str = include_str!("../../node/dashboard/styles.css");
const MAX_REQUEST_BYTES: usize = 64 * 1024;

pub struct DashboardServer {
    pub state: DashboardState,
    wake_address: SocketAddr,
    handle: Option<JoinHandle<()>>,
}

impl DashboardServer {
    pub fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = TcpStream::connect(self.wake_address);
            let _ = handle.join();
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn secure_equal(actual: &str, expected: &str) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    actual
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn response(status: u16, reason: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let csp = "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: {csp}\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: keep-alive\r\n\r\n",
        body.len()
    );
    [head.as_bytes(), body].concat()
}

fn json_response(status: u16, reason: &str, value: Value) -> Vec<u8> {
    response(
        status,
        reason,
        "application/json; charset=utf-8",
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string()).as_bytes(),
    )
}

fn transport_error(code: &str, message: &str) -> Value {
    json!({
        "type": "dashboard_transport_error",
        "schema_version": crate::SCHEMA_VERSION,
        "producer": { "version": crate::RELAY_VERSION },
        "ok": false,
        "error": { "code": code, "message": message }
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn read_request(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<Option<HttpRequest>, String> {
    loop {
        if let Some(header_end) = find_header_end(buffer) {
            let header_text = String::from_utf8_lossy(&buffer[..header_end]);
            let mut lines = header_text.split("\r\n");
            let request_line = lines.next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            if method.is_empty() || path.is_empty() {
                return Err("invalid HTTP request line".to_string());
            }

            let mut headers = HashMap::new();
            for line in lines {
                if let Some((name, value)) = line.split_once(':') {
                    headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
                }
            }
            let content_length = headers
                .get("content-length")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            if content_length > MAX_REQUEST_BYTES {
                return Err("request too large".to_string());
            }
            let total = header_end + 4 + content_length;
            if buffer.len() < total {
                // Need more body bytes.
            } else {
                let body = buffer[header_end + 4..total].to_vec();
                buffer.drain(..total);
                return Ok(Some(HttpRequest {
                    method,
                    path,
                    headers,
                    body,
                }));
            }
        }

        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => {
                if buffer.is_empty() {
                    return Ok(None);
                }
                return Err("connection closed mid-request".to_string());
            }
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if buffer.len() > MAX_REQUEST_BYTES + 16 * 1024 {
                    return Err("request too large".to_string());
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Ok(None);
            }
            Err(error) => return Err(format!("read HTTP request: {error}")),
        }
    }
}

fn handle_request(
    request: HttpRequest,
    token: &str,
    context: &HostContext,
    index_html: &str,
) -> Vec<u8> {
    match (request.method.as_str(), request.path.as_str()) {
        ("OPTIONS", _) => json_response(
            403,
            "Forbidden",
            transport_error(
                "CROSS_ORIGIN_BLOCKED",
                "Cross-origin dashboard requests are not supported.",
            ),
        ),
        ("GET", "/") => response(
            200,
            "OK",
            "text/html; charset=utf-8",
            index_html.as_bytes(),
        ),
        ("GET", "/app.js") => response(
            200,
            "OK",
            "text/javascript; charset=utf-8",
            APP_JS.as_bytes(),
        ),
        ("GET", "/styles.css") => response(
            200,
            "OK",
            "text/css; charset=utf-8",
            STYLES_CSS.as_bytes(),
        ),
        ("GET", "/healthz") => json_response(
            200,
            "OK",
            json!({
                "ok": true,
                "component": "relay-rust-dashboard-transport",
                "mode": "embedded-host",
                "command_count": registry::surface_command_ids("dashboard").len()
            }),
        ),
        ("POST", "/api/execute") => {
            let supplied = request
                .headers
                .get("x-relay-dashboard-token")
                .map(String::as_str)
                .unwrap_or("");
            if !secure_equal(supplied, token) {
                return json_response(
                    401,
                    "Unauthorized",
                    transport_error(
                        "DASHBOARD_UNAUTHORIZED",
                        "Dashboard session token is missing or invalid.",
                    ),
                );
            }
            if !request
                .headers
                .get("content-type")
                .map(|value| value.to_ascii_lowercase().starts_with("application/json"))
                .unwrap_or(false)
            {
                return json_response(
                    415,
                    "Unsupported Media Type",
                    transport_error(
                        "UNSUPPORTED_MEDIA_TYPE",
                        "Dashboard execution requires application/json.",
                    ),
                );
            }

            let value: Value = match serde_json::from_slice(&request.body) {
                Ok(value) => value,
                Err(_) => {
                    return json_response(
                        400,
                        "Bad Request",
                        transport_error("BAD_REQUEST", "Dashboard request body must be valid JSON."),
                    )
                }
            };
            let Some(command) = value.get("command").and_then(Value::as_str) else {
                return json_response(
                    400,
                    "Bad Request",
                    transport_error("BAD_REQUEST", "command is required."),
                );
            };
            if !registry::is_surface_exposed(command, "dashboard") {
                return json_response(
                    403,
                    "Forbidden",
                    transport_error(
                        "DASHBOARD_COMMAND_NOT_EXPOSED",
                        "This read-only dashboard challenger does not expose that command.",
                    ),
                );
            }
            let request_id = value
                .get("request_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    format!(
                        "dashboard-rust-{}-{}",
                        std::process::id(),
                        context.started.elapsed().as_nanos()
                    )
                });
            let command_version = value
                .get("command_version")
                .and_then(Value::as_u64)
                .map(|value| value as u32);
            let arguments = value.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let result = dispatch_command(
                &request_id,
                command,
                command_version,
                arguments,
                context,
            );
            json_response(200, "OK", result)
        }
        _ => json_response(
            404,
            "Not Found",
            transport_error("NOT_FOUND", "Dashboard route not found."),
        ),
    }
}

fn serve_connection(
    mut stream: TcpStream,
    token: String,
    context: HostContext,
    index_html: String,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_nodelay(true);
    let mut buffer = Vec::with_capacity(4096);

    while !context.shutdown.load(Ordering::SeqCst) {
        let request = match read_request(&mut stream, &mut buffer) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(_) => break,
        };
        let bytes = handle_request(request, &token, &context, &index_html);
        if stream.write_all(&bytes).is_err() || stream.flush().is_err() {
            break;
        }
    }
}

pub fn start(context: HostContext) -> Result<DashboardServer, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("bind dashboard loopback: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("dashboard local address: {error}"))?;
    let url = format!("http://127.0.0.1:{}", address.port());
    let token = random_hex(32)?;
    let index_html = INDEX_TEMPLATE.replace("__RELAY_DASHBOARD_TOKEN__", &token);
    let state = DashboardState {
        mode: "embedded-host".to_string(),
        url,
        commands: registry::surface_command_ids("dashboard"),
    };

    let thread_context = context.clone();
    let wake_address = address;
    let handle = thread::spawn(move || {
        while let Ok((stream, _)) = listener.accept() {
            if thread_context.shutdown.load(Ordering::SeqCst) {
                break;
            }
            let token = token.clone();
            let context = thread_context.clone();
            let html = index_html.clone();
            thread::spawn(move || serve_connection(stream, token, context, html));
        }
    });

    Ok(DashboardServer {
        state,
        wake_address,
        handle: Some(handle),
    })
}
