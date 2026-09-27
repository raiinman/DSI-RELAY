//! Authenticated loopback discovery gateway. It has no public-network listener or shell path.

use relay::client;
use relay_contracts::{registry, CommandRequest, CommandResponse, RequestContext};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const GATEWAY_PROTOCOL: u32 = 1;
pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
pub const MAX_HEADER_BYTES: usize = 4_096;
pub const MAX_BODY_BYTES: usize = 8_192;
pub const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const MAX_LIST_ITEMS: u64 = 20;
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const HOST_TIMEOUT: Duration = Duration::from_secs(5);
const EXPOSED: [&str; 5] = [
    "registry.list@1",
    "registry.describe@1",
    "result.list@1",
    "result.describe@1",
    "result.context@1",
];
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    WeakToken,
    BindFailed,
}

/// The caller generates and rotates a random token; the gateway never persists it.
pub struct GatewayConfig {
    token: Vec<u8>,
    port: u16,
}

impl GatewayConfig {
    pub fn new(token: Vec<u8>, port: u16) -> Result<Self, ConfigError> {
        if !(32..=128).contains(&token.len())
            || !token
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ConfigError::WeakToken);
        }
        Ok(Self { token, port })
    }
}

pub trait RelayTransport: Send + Sync {
    fn call(&self, request: &CommandRequest) -> Result<CommandResponse, ()>;
}

pub struct LocalDaemonTransport;

impl RelayTransport for LocalDaemonTransport {
    fn call(&self, request: &CommandRequest) -> Result<CommandResponse, ()> {
        let state = client::read_state().map_err(|_| ())?;
        client::call(&state, request).map_err(|_| ())
    }
}

pub struct Gateway {
    listener: TcpListener,
    token: Vec<u8>,
    transport: Arc<dyn RelayTransport>,
    host_inflight: Arc<AtomicBool>,
}

impl Gateway {
    pub fn bind(
        config: GatewayConfig,
        transport: Arc<dyn RelayTransport>,
    ) -> Result<Self, ConfigError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
            .map_err(|_| ConfigError::BindFailed)?;
        Ok(Self {
            listener,
            token: config.token,
            transport,
            host_inflight: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .expect("bound listener has address")
    }

    /// Serve one connection; the caller owns lifecycle and must not publish this port.
    pub fn serve_one(&self) -> std::io::Result<()> {
        let (stream, peer) = self.listener.accept()?;
        self.serve_stream(stream, peer)
    }

    /// Serve serially until the local launcher requests shutdown. Each client
    /// failure stays local to that connection; no remote shutdown route exists.
    pub fn serve_until(&self, shutdown: &AtomicBool) -> std::io::Result<()> {
        self.listener.set_nonblocking(true)?;
        while !shutdown.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    let _ = self.serve_stream(stream, peer);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn serve_stream(&self, mut stream: TcpStream, peer: SocketAddr) -> std::io::Result<()> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(READ_TIMEOUT))?;
        if !peer.ip().is_loopback() {
            return write_error(&mut stream, 403, "LOOPBACK_REQUIRED");
        }
        let response = match read_request(&mut stream) {
            Ok(request) => self.handle(request),
            Err(error) => error,
        };
        write_json(&mut stream, response.status, &response.body)
    }

    fn handle(&self, request: HttpRequest) -> HttpResponse {
        let expected_host = format!("127.0.0.1:{}", self.local_addr().port());
        if request.headers.get("host").map(String::as_str) != Some(expected_host.as_str()) {
            return HttpResponse::error(400, "INVALID_HOST");
        }
        if request.headers.contains_key("origin") {
            return HttpResponse::error(403, "ORIGIN_NOT_ALLOWED");
        }
        let Some(authorization) = request.headers.get("authorization") else {
            return HttpResponse::error(401, "AUTH_REQUIRED");
        };
        let supplied = authorization.strip_prefix("Bearer ").unwrap_or("");
        if !constant_time_equal(supplied.as_bytes(), &self.token) {
            return HttpResponse::error(401, "AUTH_REQUIRED");
        }
        if request.method != "POST" {
            return HttpResponse::error(405, "METHOD_NOT_ALLOWED");
        }
        if request.headers.get("content-type").map(String::as_str) != Some("application/json") {
            return HttpResponse::error(415, "JSON_REQUIRED");
        }
        match request.path.as_str() {
            "/v1/negotiate" => self.negotiate(&request.body),
            "/v1/execute" => self.execute(&request.body),
            "/mcp" => self.mcp(&request),
            _ => HttpResponse::error(404, "NOT_FOUND"),
        }
    }

    /// Stateless MCP Streamable HTTP 2026-07-28, with a fixed metadata allowlist.
    fn mcp(&self, request: &HttpRequest) -> HttpResponse {
        let parsed: Value = match serde_json::from_slice(&request.body) {
            Ok(value) => value,
            Err(_) => return mcp_error(400, Value::Null, -32700, "Parse error"),
        };
        let id = parsed.get("id").cloned().unwrap_or(Value::Null);
        if parsed.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !(id.is_string() || id.is_number())
            || id.as_str().is_some_and(|value| value.len() > 128)
        {
            return mcp_error(400, Value::Null, -32600, "Invalid Request");
        }
        let Some(method) = parsed.get("method").and_then(Value::as_str) else {
            return mcp_error(400, id, -32600, "Invalid Request");
        };
        let Some(params) = parsed.get("params").and_then(Value::as_object) else {
            return mcp_error(400, id, -32602, "Invalid params");
        };
        let Some(meta) = params.get("_meta").and_then(Value::as_object) else {
            return mcp_error(400, id, -32602, "Invalid params");
        };
        let body_version = meta
            .get("io.modelcontextprotocol/protocolVersion")
            .and_then(Value::as_str);
        let header_version = request
            .headers
            .get("mcp-protocol-version")
            .map(String::as_str);
        if header_version.is_none()
            || header_version != body_version
            || request.headers.get("mcp-method").map(String::as_str) != Some(method)
        {
            return mcp_error(400, id, -32020, "Header mismatch");
        }
        if header_version != Some(MCP_PROTOCOL_VERSION) {
            let requested = header_version
                .filter(|value| {
                    value.len() <= 32
                        && value
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || byte == b'-')
                })
                .unwrap_or("unsupported");
            return mcp_error_with_data(
                400,
                id,
                -32022,
                "Unsupported protocol version",
                json!({"supported":[MCP_PROTOCOL_VERSION],"requested":requested}),
            );
        }
        if !request.headers.get("accept").is_some_and(|value| {
            value
                .split(',')
                .any(|item| item.trim().split(';').next() == Some("application/json"))
                && value
                    .split(',')
                    .any(|item| item.trim().split(';').next() == Some("text/event-stream"))
        }) || !meta
            .get("io.modelcontextprotocol/clientInfo")
            .is_some_and(|value| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| !name.is_empty() && name.len() <= 128)
                    && value
                        .get("version")
                        .and_then(Value::as_str)
                        .is_some_and(|version| !version.is_empty() && version.len() <= 64)
            })
            || !meta
                .get("io.modelcontextprotocol/clientCapabilities")
                .is_some_and(Value::is_object)
        {
            return mcp_error(400, id, -32602, "Invalid params");
        }
        if method == "tools/call" {
            let name = params.get("name").and_then(Value::as_str);
            if name.is_none()
                || !mcp_name_matches(
                    name.unwrap_or(""),
                    request.headers.get("mcp-name").map(String::as_str),
                )
            {
                return mcp_error(400, id, -32020, "Header mismatch");
            }
        }
        match method {
            "server/discover" => {
                let Some(status) = self.host_status() else {
                    return mcp_error(503, id, -32603, "Host unavailable");
                };
                let capabilities = if status
                    .capabilities
                    .iter()
                    .any(|item| EXPOSED.contains(&item.as_str()))
                {
                    json!({"tools":{}})
                } else {
                    json!({})
                };
                mcp_result(
                    id,
                    json!({
                        "resultType":"complete", "supportedVersions":[MCP_PROTOCOL_VERSION],
                        "capabilities":capabilities,
                        "_meta":{"io.modelcontextprotocol/serverInfo":{"name":"relay-registry-gateway","version":env!("CARGO_PKG_VERSION")}},
                        "instructions":"Read-only RELAY registry and project-scoped result metadata/context on this computer.",
                        "ttlMs":0, "cacheScope":"private"
                    }),
                )
            }
            "tools/list" => {
                if params.get("cursor").is_some() {
                    return mcp_error(400, id, -32602, "Invalid params");
                }
                let Some(status) = self.host_status() else {
                    return mcp_error(503, id, -32603, "Host unavailable");
                };
                let mut tools = Vec::new();
                if status
                    .capabilities
                    .iter()
                    .any(|item| item == "registry.list@1")
                {
                    tools.push(json!({"name":"relay_registry_list","title":"List RELAY commands","description":"List a bounded set of AI-exposed RELAY command definitions by prefix.",
                        "inputSchema":{"type":"object","required":["prefix"],"properties":{"prefix":{"type":"string","minLength":1,"maxLength":64},"limit":{"type":"integer","minimum":1,"maximum":20}},"additionalProperties":false}}));
                }
                if status
                    .capabilities
                    .iter()
                    .any(|item| item == "registry.describe@1")
                {
                    tools.push(json!({"name":"relay_registry_describe","title":"Describe a RELAY command","description":"Describe one AI-exposed RELAY command definition.",
                        "inputSchema":{"type":"object","required":["command"],"properties":{"command":{"type":"string","minLength":1,"maxLength":128},"version":{"type":"integer","minimum":1}},"additionalProperties":false}}));
                }
                if status
                    .capabilities
                    .iter()
                    .any(|item| item == "result.list@1")
                {
                    tools.push(json!({"name":"relay_result_list","title":"List project result metadata","description":"List at most 20 metadata records for one explicit RELAY project; no payload or provenance.",
                        "inputSchema":{"type":"object","required":["project_id","limit"],"properties":{"project_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"},"limit":{"type":"integer","minimum":1,"maximum":20}},"additionalProperties":false}}));
                }
                if status
                    .capabilities
                    .iter()
                    .any(|item| item == "result.describe@1")
                {
                    tools.push(json!({"name":"relay_result_describe","title":"Describe one project result","description":"Return provenance-free metadata only when the result belongs to the named project.",
                        "inputSchema":{"type":"object","required":["project_id","result_id"],"properties":{"project_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"},"result_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"}},"additionalProperties":false}}));
                }
                if status
                    .capabilities
                    .iter()
                    .any(|item| item == "result.context@1")
                    && status
                        .capabilities
                        .iter()
                        .any(|item| item == "result.describe@1")
                {
                    tools.push(json!({"name":"relay_result_context","title":"Read compact project result facts","description":"Read Core-filtered exact facts under a 4096-byte budget after verifying project scope; never returns the full payload.",
                        "inputSchema":{"type":"object","required":["project_id","result_id","max_bytes"],"properties":{"project_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"},"result_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"},"max_bytes":{"type":"integer","minimum":512,"maximum":4096},"required_pointers":{"type":"array","minItems":1,"maxItems":8,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":256}}},"additionalProperties":false}}));
                }
                mcp_result(
                    id,
                    json!({"resultType":"complete","tools":tools,"ttlMs":0,"cacheScope":"private"}),
                )
            }
            "tools/call" => {
                let (command, arguments) = match params.get("name").and_then(Value::as_str) {
                    Some("relay_registry_list") => {
                        let Some(input) = params.get("arguments").and_then(Value::as_object) else {
                            return mcp_error(400, id, -32602, "Invalid params");
                        };
                        if input.keys().any(|key| key != "prefix" && key != "limit") {
                            return mcp_error(400, id, -32602, "Invalid params");
                        }
                        (
                            "registry.list",
                            json!({"surface":"ai","prefix":input.get("prefix"),"limit":input.get("limit").cloned().unwrap_or(json!(20))}),
                        )
                    }
                    Some("relay_registry_describe") => {
                        let Some(input) = params.get("arguments").and_then(Value::as_object) else {
                            return mcp_error(400, id, -32602, "Invalid params");
                        };
                        if input.keys().any(|key| key != "command" && key != "version") {
                            return mcp_error(400, id, -32602, "Invalid params");
                        }
                        let mut args = json!({"command":input.get("command")});
                        if let Some(version) = input.get("version") {
                            args["version"] = version.clone();
                        }
                        ("registry.describe", args)
                    }
                    Some("relay_result_list") => (
                        "result.list",
                        params.get("arguments").cloned().unwrap_or(Value::Null),
                    ),
                    Some("relay_result_describe") => (
                        "result.describe",
                        params.get("arguments").cloned().unwrap_or(Value::Null),
                    ),
                    Some("relay_result_context") => (
                        "result.context",
                        params.get("arguments").cloned().unwrap_or(Value::Null),
                    ),
                    _ => return mcp_error(400, id, -32602, "Unknown tool"),
                };
                if self.prepare_arguments(command, &arguments).is_none() {
                    return mcp_error(400, id, -32602, "Invalid params");
                }
                let body = json!({"gateway_protocol":GATEWAY_PROTOCOL,"command":command,"command_version":1,"arguments":arguments});
                let forwarded = self.execute(body.to_string().as_bytes());
                if forwarded.status != 200 {
                    return mcp_error(503, id, -32603, "Read unavailable");
                }
                if forwarded.body.get("ok").and_then(Value::as_bool) != Some(true) {
                    return mcp_result(
                        id,
                        json!({"resultType":"complete","content":[{"type":"text","text":"RELAY read failed"}],"isError":true}),
                    );
                }
                let Some(result) = forwarded.body.get("result") else {
                    return mcp_error(502, id, -32603, "Invalid host response");
                };
                if !serde_json::to_vec(result)
                    .is_ok_and(|value| value.len() <= MAX_RESPONSE_BYTES / 2)
                {
                    return mcp_error(502, id, -32603, "Host response too large");
                }
                mcp_result(
                    id,
                    json!({"resultType":"complete","content":[{"type":"text","text":"RELAY read completed; read structuredContent."}],"structuredContent":result}),
                )
            }
            _ => mcp_error(404, id, -32601, "Method not found"),
        }
    }

    fn negotiate(&self, body: &[u8]) -> HttpResponse {
        let Ok(input) = serde_json::from_slice::<NegotiationRequest>(body) else {
            return HttpResponse::error(400, "INVALID_REQUEST");
        };
        if input.protocol_min > GATEWAY_PROTOCOL || input.protocol_max < GATEWAY_PROTOCOL {
            return HttpResponse::error(409, "PROTOCOL_INCOMPATIBLE");
        }
        let Some(status) = self.host_status() else {
            return HttpResponse::error(503, "HOST_UNAVAILABLE");
        };
        let commands: Vec<_> = EXPOSED
            .into_iter()
            .filter(|capability| status.capabilities.iter().any(|host| host == capability))
            .collect();
        HttpResponse::ok(json!({
            "gateway_protocol": GATEWAY_PROTOCOL,
            "relay_version": status.version,
            "commands": commands,
            "network_scope": "loopback_only"
        }))
    }

    fn execute(&self, body: &[u8]) -> HttpResponse {
        let Ok(input) = serde_json::from_slice::<ExecuteRequest>(body) else {
            return HttpResponse::error(400, "INVALID_REQUEST");
        };
        if input.gateway_protocol != GATEWAY_PROTOCOL {
            return HttpResponse::error(409, "PROTOCOL_INCOMPATIBLE");
        }
        if !matches!(
            input.command.as_str(),
            "registry.list"
                | "registry.describe"
                | "result.list"
                | "result.describe"
                | "result.context"
        ) || input.command_version != 1
        {
            return HttpResponse::error(403, "COMMAND_NOT_EXPOSED");
        }
        let Ok(spec) = registry::resolve_command(&input.command, Some(input.command_version))
        else {
            return HttpResponse::error(409, "COMMAND_VERSION_INCOMPATIBLE");
        };
        if spec.effect_class != "observe" || spec.permission != "read" {
            return HttpResponse::error(403, "COMMAND_NOT_EXPOSED");
        }
        let Some((forwarded_arguments, project_scope)) =
            self.prepare_arguments(&input.command, &input.arguments)
        else {
            return HttpResponse::error(400, "INVALID_ARGUMENTS");
        };
        let Some(status) = self.host_status() else {
            return HttpResponse::error(503, "HOST_UNAVAILABLE");
        };
        let capability = format!("{}@{}", input.command, input.command_version);
        if !status.capabilities.iter().any(|item| item == &capability) {
            return HttpResponse::error(409, "CAPABILITY_UNAVAILABLE");
        }
        if input.command == "result.context"
            && (!status
                .capabilities
                .iter()
                .any(|item| item == "result.describe@1")
                || !self.result_belongs_to_project(
                    forwarded_arguments["result_id"].as_str().unwrap_or(""),
                    project_scope.as_deref().unwrap_or(""),
                ))
        {
            return HttpResponse::error(403, "PROJECT_SCOPE_DENIED");
        }
        let command_request = make_request(
            input.command.clone(),
            input.command_version,
            forwarded_arguments,
        );
        let Ok(reply) = self.call_host(command_request.clone()) else {
            return HttpResponse::error(503, "HOST_UNAVAILABLE");
        };
        if reply.message_type != "command_result"
            || reply.request_id != command_request.request_id
            || reply.command_version != input.command_version
        {
            return HttpResponse::error(502, "HOST_RESPONSE_INVALID");
        }
        if !reply.ok {
            return HttpResponse::ok(json!({"ok":false,"code":"COMMAND_FAILED"}));
        }
        let Some(result) = reply.result else {
            return HttpResponse::error(502, "HOST_RESPONSE_INVALID");
        };
        if registry::validate_value(&spec.result_schema, &result).is_err()
            || !self.result_matches_scope(
                &input.command,
                &result,
                project_scope.as_deref(),
                &command_request.arguments,
            )
        {
            return HttpResponse::error(502, "HOST_RESPONSE_INVALID");
        }
        let body = json!({"ok":true,"result":result});
        if serde_json::to_vec(&body).is_ok_and(|bytes| bytes.len() <= MAX_RESPONSE_BYTES) {
            HttpResponse::ok(body)
        } else {
            HttpResponse::error(502, "HOST_RESPONSE_TOO_LARGE")
        }
    }

    fn prepare_arguments(
        &self,
        command: &str,
        arguments: &Value,
    ) -> Option<(Value, Option<String>)> {
        if matches!(command, "registry.list" | "registry.describe") {
            return self
                .validate_discovery_arguments(command, arguments)
                .then(|| (arguments.clone(), None));
        }
        let object = arguments.as_object()?;
        let project_id = object.get("project_id")?.as_str()?;
        if !valid_command_token(project_id, 128) {
            return None;
        }
        let mut forwarded = arguments.clone();
        if command == "result.list" {
            let limit = object.get("limit")?.as_u64()?;
            if !(1..=MAX_LIST_ITEMS).contains(&limit) {
                return None;
            }
        } else {
            forwarded.as_object_mut()?.remove("project_id");
            let result_id = forwarded.get("result_id")?.as_str()?;
            if !valid_command_token(result_id, 128) {
                return None;
            }
            if command == "result.context"
                && (forwarded.get("focus_terms").is_some()
                    || !forwarded
                        .get("max_bytes")
                        .and_then(Value::as_u64)
                        .is_some_and(|size| (512..=4096).contains(&size)))
            {
                return None;
            }
        }
        let spec = registry::resolve_command(command, Some(1)).ok()?;
        registry::validate_value(&spec.arguments_schema, &forwarded).ok()?;
        Some((forwarded, Some(project_id.to_string())))
    }

    fn result_belongs_to_project(&self, result_id: &str, project_id: &str) -> bool {
        let request = make_request(
            "result.describe".to_string(),
            1,
            json!({"result_id":result_id}),
        );
        let Ok(reply) = self.call_host(request.clone()) else {
            return false;
        };
        if !reply.ok
            || reply.message_type != "command_result"
            || reply.request_id != request.request_id
            || reply.command_version != 1
        {
            return false;
        }
        let Some(result) = reply.result else {
            return false;
        };
        let Ok(spec) = registry::resolve_command("result.describe", Some(1)) else {
            return false;
        };
        registry::validate_value(&spec.result_schema, &result).is_ok()
            && result["project_id"].as_str() == Some(project_id)
            && result["id"].as_str() == Some(result_id)
    }

    fn result_matches_scope(
        &self,
        command: &str,
        result: &Value,
        project_id: Option<&str>,
        arguments: &Value,
    ) -> bool {
        match command {
            "result.list" => result["results"].as_array().is_some_and(|records| {
                records.len() <= MAX_LIST_ITEMS as usize
                    && records.iter().all(|record| {
                        record["project_id"].as_str() == project_id && safe_metadata(record)
                    })
            }),
            "result.describe" => {
                result["project_id"].as_str() == project_id
                    && result["id"].as_str() == arguments["result_id"].as_str()
                    && safe_metadata(result)
            }
            "result.context" => {
                result["result_id"].as_str() == arguments["result_id"].as_str()
                    && result["result_id"]
                        .as_str()
                        .is_some_and(|id| valid_command_token(id, 128))
                    && result["payload_sha256"].as_str().is_some_and(|hash| {
                        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    && ["source_trust", "source_created_at", "producer_version"]
                        .iter()
                        .all(|key| {
                            result
                                .get(*key)
                                .is_none_or(|value| value.as_str().is_some_and(safe_metadata_token))
                        })
                    && serde_json::to_vec(result).is_ok_and(|bytes| bytes.len() <= 16_384)
                    && result["facts"].as_array().is_some_and(|facts| {
                        facts.iter().all(|fact| {
                            fact["pointer"].as_str().is_some_and(safe_fact_pointer)
                                && match &fact["value"] {
                                    Value::String(value) => safe_metadata_token(value),
                                    Value::Number(_) | Value::Bool(_) => true,
                                    _ => false,
                                }
                        })
                    })
            }
            _ => true,
        }
    }

    fn validate_discovery_arguments(&self, command: &str, arguments: &Value) -> bool {
        let Ok(spec) = registry::resolve_command(command, Some(1)) else {
            return false;
        };
        if registry::validate_value(&spec.arguments_schema, arguments).is_err() {
            return false;
        }
        if command == "registry.list" {
            let Some(prefix) = arguments.get("prefix").and_then(Value::as_str) else {
                return false;
            };
            if !valid_command_token(prefix, 64)
                || arguments.get("surface").and_then(Value::as_str) != Some("ai")
                || !arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .is_some_and(|limit| (1..=MAX_LIST_ITEMS).contains(&limit))
            {
                return false;
            }
        } else {
            let Some(target) = arguments.get("command").and_then(Value::as_str) else {
                return false;
            };
            let version = match arguments.get("version") {
                None => None,
                Some(value) => match value.as_u64().and_then(|value| u32::try_from(value).ok()) {
                    Some(version) => Some(version),
                    None => return false,
                },
            };
            if !valid_command_token(target, 128)
                || !registry::resolve_command(target, version).is_ok_and(|target_spec| {
                    target_spec.surfaces.iter().any(|surface| surface == "ai")
                })
            {
                return false;
            }
        }
        true
    }

    fn host_status(&self) -> Option<HostStatus> {
        let request = make_request("system.status".to_string(), 1, json!({}));
        let response = self.call_host(request.clone()).ok()?;
        if !response.ok
            || response.message_type != "command_result"
            || response.request_id != request.request_id
            || response.command_version != 1
        {
            return None;
        }
        let result = response.result?;
        let version = result.get("version")?.as_str()?.to_string();
        if !version.starts_with("0.1.") || version.len() > 32 {
            return None;
        }
        let advertised = result.get("capabilities")?.as_array()?;
        if advertised.len() > 256 {
            return None;
        }
        let capabilities = advertised
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?;
        Some(HostStatus {
            version,
            capabilities,
        })
    }

    /// One daemon call may occupy one worker for at most the response wait.
    /// A stuck pipe leaves the guard held until that worker exits, so repeated
    /// HTTP requests cannot accumulate blocked daemon threads.
    fn call_host(&self, request: CommandRequest) -> Result<CommandResponse, ()> {
        if self
            .host_inflight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(());
        }
        let transport = Arc::clone(&self.transport);
        let inflight = Arc::clone(&self.host_inflight);
        let (sender, receiver) = mpsc::sync_channel(1);
        if thread::Builder::new()
            .name("relay-gateway-host".to_string())
            .spawn(move || {
                let response = transport.call(&request);
                let _ = sender.send(response);
                inflight.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.host_inflight.store(false, Ordering::Release);
            return Err(());
        }
        receiver.recv_timeout(HOST_TIMEOUT).map_err(|_| ())?
    }
}

struct HostStatus {
    version: String,
    capabilities: Vec<String>,
}

fn make_request(command: String, command_version: u32, arguments: Value) -> CommandRequest {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    CommandRequest {
        request_id: format!("GW-{nanos}-{sequence}"),
        command,
        command_version: Some(command_version),
        arguments,
        idempotency_key: None,
        context: RequestContext {
            client_id: Some("relay-gateway-prototype".to_string()),
            ..RequestContext::default()
        },
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NegotiationRequest {
    protocol_min: u32,
    protocol_max: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecuteRequest {
    gateway_protocol: u32,
    command: String,
    command_version: u32,
    arguments: Value,
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    body: Value,
}

fn mcp_result(id: Value, result: Value) -> HttpResponse {
    let body = json!({"jsonrpc":"2.0","id":id.clone(),"result":result});
    if serde_json::to_vec(&body).is_ok_and(|bytes| bytes.len() <= MAX_RESPONSE_BYTES) {
        HttpResponse::ok(body)
    } else {
        mcp_error(502, id, -32603, "Response too large")
    }
}

fn mcp_name_matches(name: &str, header: Option<&str>) -> bool {
    // These are the only supported tool names. The alternate forms are the
    // transport's required Base64 sentinel encoding of those ASCII names.
    match (name, header) {
        (
            "relay_registry_list",
            Some("relay_registry_list" | "=?base64?cmVsYXlfcmVnaXN0cnlfbGlzdA==?="),
        ) => true,
        (
            "relay_registry_describe",
            Some("relay_registry_describe" | "=?base64?cmVsYXlfcmVnaXN0cnlfZGVzY3JpYmU=?="),
        ) => true,
        (other, Some(value)) if valid_command_token(other, 128) && other == value => true,
        _ => false,
    }
}

fn mcp_error(status: u16, id: Value, code: i32, message: &'static str) -> HttpResponse {
    HttpResponse {
        status,
        body: json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}),
    }
}

fn mcp_error_with_data(
    status: u16,
    id: Value,
    code: i32,
    message: &'static str,
    data: Value,
) -> HttpResponse {
    HttpResponse {
        status,
        body: json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message,"data":data}}),
    }
}

impl HttpResponse {
    fn ok(body: Value) -> Self {
        Self { status: 200, body }
    }
    fn error(status: u16, code: &'static str) -> Self {
        Self {
            status,
            body: json!({ "ok": false, "code": code }),
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, HttpResponse> {
    let mut bytes = Vec::with_capacity(1024);
    let header_end = loop {
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err(HttpResponse::error(431, "HEADERS_TOO_LARGE"));
        }
        let mut chunk = [0u8; 1024];
        let count = stream
            .read(&mut chunk)
            .map_err(|_| HttpResponse::error(400, "INVALID_HTTP"))?;
        if count == 0 {
            return Err(HttpResponse::error(400, "INVALID_HTTP"));
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return Err(HttpResponse::error(413, "REQUEST_TOO_LARGE"));
        }
    };
    if header_end > MAX_HEADER_BYTES {
        return Err(HttpResponse::error(431, "HEADERS_TOO_LARGE"));
    }
    let head = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| HttpResponse::error(400, "INVALID_HTTP"))?;
    let mut lines = head[..head.len() - 4].split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpResponse::error(400, "INVALID_HTTP"))?;
    let parts: Vec<_> = request_line.split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" || parts[0].is_empty() || parts[1].is_empty() {
        return Err(HttpResponse::error(400, "INVALID_HTTP"));
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();
    let mut headers = BTreeMap::new();
    for line in lines {
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(HttpResponse::error(400, "INVALID_HTTP"));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(HttpResponse::error(400, "INVALID_HTTP"));
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(HttpResponse::error(400, "INVALID_HTTP"));
        }
        let key = name.to_ascii_lowercase();
        if headers.insert(key, value.trim().to_string()).is_some() {
            return Err(HttpResponse::error(400, "DUPLICATE_HEADER"));
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(HttpResponse::error(400, "TRANSFER_ENCODING_UNSUPPORTED"));
    }
    let length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| HttpResponse::error(411, "CONTENT_LENGTH_REQUIRED"))?;
    if length > MAX_BODY_BYTES {
        return Err(HttpResponse::error(413, "REQUEST_TOO_LARGE"));
    }
    let already = bytes.len() - header_end;
    if already > length {
        return Err(HttpResponse::error(400, "INVALID_HTTP"));
    }
    while bytes.len() - header_end < length {
        let remaining = length - (bytes.len() - header_end);
        let mut chunk = [0u8; 1024];
        let take = remaining.min(chunk.len());
        let count = stream
            .read(&mut chunk[..take])
            .map_err(|_| HttpResponse::error(400, "INVALID_HTTP"))?;
        if count == 0 {
            return Err(HttpResponse::error(400, "INVALID_HTTP"));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: bytes[header_end..].to_vec(),
    })
}

fn valid_command_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn safe_metadata_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'+')
        })
}

fn safe_metadata(record: &Value) -> bool {
    ["id", "kind", "producer_version", "trust", "created_at"]
        .iter()
        .all(|key| record[*key].as_str().is_some_and(safe_metadata_token))
        && record["project_id"]
            .as_str()
            .is_some_and(|id| valid_command_token(id, 128))
        && record["payload_sha256"].as_str().is_some_and(|hash| {
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn safe_fact_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') || pointer.len() > 256 {
        return false;
    }
    pointer[1..].split('/').all(|segment| {
        let lower = segment.to_ascii_lowercase();
        valid_command_token(segment, 64)
            && ![
                "path",
                "file",
                "root",
                "uri",
                "url",
                "directory",
                "location",
                "secret",
                "token",
                "credential",
                "password",
                "private",
                "auth",
                "cookie",
                "session",
                "bearer",
                "key",
            ]
            .iter()
            .any(|part| lower.contains(part))
    })
}

fn constant_time_equal(supplied: &[u8], expected: &[u8]) -> bool {
    let mut different = supplied.len() ^ expected.len();
    for index in 0..expected.len() {
        different |= (supplied.get(index).copied().unwrap_or(0) ^ expected[index]) as usize;
    }
    different == 0
}

fn write_error(stream: &mut TcpStream, status: u16, code: &'static str) -> std::io::Result<()> {
    write_json(stream, status, &json!({ "ok": false, "code": code }))
}

fn write_json(stream: &mut TcpStream, status: u16, body: &Value) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(body)
        .unwrap_or_else(|_| b"{\"ok\":false,\"code\":\"SERIALIZATION_FAILED\"}".to_vec());
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_contracts::Producer;
    use std::net::Shutdown;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    const TOKEN: &str = "abcdefghijklmnopqrstuvwxyzABCDEF";

    struct FixtureTransport {
        calls: Arc<AtomicUsize>,
    }

    impl RelayTransport for FixtureTransport {
        fn call(&self, request: &CommandRequest) -> Result<CommandResponse, ()> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let result = if request.command == "system.status" {
                json!({
                    "version": "0.1.0",
                    "capabilities": ["registry.list@1", "registry.describe@1", "result.list@1", "result.describe@1", "result.context@1"]
                })
            } else if request.command == "result.list" {
                json!({"results":[{"id":"RES-alpha","project_id":"PRJ-alpha","kind":"CHECK","schema_version":1,
                    "producer_version":"0.1.0","payload_sha256":"0".repeat(64),"payload_bytes":32,
                    "trust":"local","created_at":"2026-09-27T00:00:00Z"}]})
            } else if request.command == "result.describe" {
                json!({"id":"RES-alpha","project_id":"PRJ-alpha","kind":"CHECK","schema_version":1,
                    "producer_version":"0.1.0","payload_sha256":"0".repeat(64),"payload_bytes":32,
                    "trust":"local","created_at":"2026-09-27T00:00:00Z"})
            } else if request.command == "result.context" {
                json!({"result_id":"RES-alpha","payload_sha256":"0".repeat(64),"payload_bytes":32,
                    "source_trust":"local","full_result_command":"result.get",
                    "facts":[{"pointer":"/count","value":3}],"omitted_scalar_count":0,"truncated":false})
            } else {
                json!({"registry_format": 1, "commands": []})
            };
            Ok(CommandResponse::success(
                request,
                1,
                Producer {
                    name: "fixture".into(),
                    version: "0.1.0".into(),
                },
                result,
            ))
        }
    }

    fn exchange(
        body: &str,
        path: &str,
        extra_headers: &str,
        token: Option<&str>,
        host: Option<&str>,
    ) -> (String, usize) {
        let calls = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(FixtureTransport {
            calls: calls.clone(),
        });
        let gateway = Gateway::bind(
            GatewayConfig::new(TOKEN.as_bytes().to_vec(), 0).unwrap(),
            transport,
        )
        .unwrap();
        let address = gateway.local_addr();
        assert_eq!(address.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
        let worker = thread::spawn(move || gateway.serve_one().unwrap());
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let host = host
            .map(str::to_string)
            .unwrap_or_else(|| format!("127.0.0.1:{}", address.port()));
        let auth = token
            .map(|value| format!("Authorization: Bearer {value}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n{extra_headers}\r\n{body}",
            body.len()
        );
        client.write_all(request.as_bytes()).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut output = String::new();
        client.read_to_string(&mut output).unwrap();
        worker.join().unwrap();
        (output, calls.load(Ordering::Relaxed))
    }

    #[test]
    fn rejects_weak_token() {
        assert!(matches!(
            GatewayConfig::new(b"short".to_vec(), 0),
            Err(ConfigError::WeakToken)
        ));
    }

    #[test]
    fn rejects_missing_auth_without_forwarding() {
        let (reply, calls) = exchange("{}", "/v1/negotiate", "", None, None);
        assert!(reply.starts_with("HTTP/1.1 401"));
        assert_eq!(calls, 0);
        assert!(!reply.contains(TOKEN));
    }

    #[test]
    fn rejects_origin_and_host_spoofing() {
        let (origin, calls) = exchange(
            "{}",
            "/v1/negotiate",
            "Origin: https://example.invalid\r\n",
            Some(TOKEN),
            None,
        );
        assert!(origin.starts_with("HTTP/1.1 403"));
        assert_eq!(calls, 0);
        let (host, calls) = exchange(
            "{}",
            "/v1/negotiate",
            "",
            Some(TOKEN),
            Some("example.invalid"),
        );
        assert!(host.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn rejects_duplicate_auth_and_transfer_encoding() {
        let (duplicate, calls) = exchange(
            "{}",
            "/v1/negotiate",
            "Authorization: Bearer another\r\n",
            Some(TOKEN),
            None,
        );
        assert!(duplicate.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);
        let (transfer, calls) = exchange(
            "{}",
            "/v1/negotiate",
            "Transfer-Encoding: chunked\r\n",
            Some(TOKEN),
            None,
        );
        assert!(transfer.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn rejects_write_command_and_unbounded_list() {
        let write = json!({"gateway_protocol":1,"command":"project.register","command_version":1,"arguments":{}}).to_string();
        let (reply, calls) = exchange(&write, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 403"));
        assert_eq!(calls, 0);
        let full_payload = json!({"gateway_protocol":1,"command":"result.get","command_version":1,"arguments":{"result_id":"RES-alpha"}}).to_string();
        let (reply, calls) = exchange(&full_payload, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 403"));
        assert_eq!(calls, 0);
        let list = json!({"gateway_protocol":1,"command":"registry.list","command_version":1,"arguments":{"surface":"ai","limit":200}}).to_string();
        let (reply, calls) = exchange(&list, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn metadata_and_fact_guards_reject_path_shaped_values() {
        let mut metadata = json!({"id":"RES-alpha","project_id":"PRJ-alpha","kind":"CHECK",
            "producer_version":"0.1.0","trust":"local","created_at":"2026-09-27T00:00:00Z",
            "payload_sha256":"0".repeat(64)});
        assert!(safe_metadata(&metadata));
        metadata["kind"] = json!("C:/private/project");
        assert!(!safe_metadata(&metadata));
        assert!(safe_fact_pointer("/status"));
        assert!(!safe_fact_pointer("/project_path"));
        assert!(!safe_metadata_token("C:\\private\\project"));
    }

    #[test]
    fn rejects_describing_a_non_ai_command() {
        let body = json!({"gateway_protocol":1,"command":"registry.describe","command_version":1,"arguments":{"command":"system.shutdown"}}).to_string();
        let (reply, calls) = exchange(&body, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn negotiates_only_matching_version_and_exposed_capabilities() {
        let incompatible = json!({"protocol_min":2,"protocol_max":2}).to_string();
        let (reply, calls) = exchange(&incompatible, "/v1/negotiate", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 409"));
        assert_eq!(calls, 0);
        let compatible = json!({"protocol_min":1,"protocol_max":1}).to_string();
        let (reply, calls) = exchange(&compatible, "/v1/negotiate", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("registry.list@1"));
        assert!(!reply.contains("project.register@1"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn forwards_one_bounded_registry_request() {
        let body = json!({"gateway_protocol":1,"command":"registry.list","command_version":1,"arguments":{"surface":"ai","prefix":"project.","limit":8}}).to_string();
        let (reply, calls) = exchange(&body, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert_eq!(calls, 2); // host capability check plus one shared command
    }

    fn mcp_body(id: u32, method: &str, mut extra: Value) -> String {
        extra["_meta"] = json!({
            "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientInfo": {"name":"fixture","version":"1.0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        });
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":extra}).to_string()
    }

    fn mcp_headers(method: &str, name: Option<&str>) -> String {
        format!(
            "Accept: application/json, text/event-stream\r\nMCP-Protocol-Version: {MCP_PROTOCOL_VERSION}\r\nMcp-Method: {method}\r\n{}",
            name.map(|value| format!("Mcp-Name: {value}\r\n"))
                .unwrap_or_default()
        )
    }

    #[test]
    fn mcp_discovers_and_lists_only_registry_tools() {
        let discover = mcp_body(1, "server/discover", json!({}));
        let (reply, calls) = exchange(
            &discover,
            "/mcp",
            &mcp_headers("server/discover", None),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("\"supportedVersions\":[\"2026-07-28\"]"));
        assert_eq!(calls, 1);
        let list = mcp_body(2, "tools/list", json!({}));
        let (reply, calls) = exchange(
            &list,
            "/mcp",
            &mcp_headers("tools/list", None),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("relay_registry_list"));
        assert!(reply.contains("relay_registry_describe"));
        assert!(!reply.contains("project.register"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn mcp_calls_only_bounded_registry_discovery() {
        let call = mcp_body(
            3,
            "tools/call",
            json!({"name":"relay_registry_list","arguments":{"prefix":"project.","limit":8}}),
        );
        let (reply, calls) = exchange(
            &call,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_registry_list")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("\"structuredContent\""));
        assert_eq!(calls, 2);
        let write = mcp_body(
            4,
            "tools/call",
            json!({"name":"project.register","arguments":{}}),
        );
        let (reply, calls) = exchange(
            &write,
            "/mcp",
            &mcp_headers("tools/call", Some("project.register")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert!(reply.contains("Unknown tool"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn mcp_rejects_header_mismatch_and_unsupported_version() {
        let call = mcp_body(
            5,
            "tools/call",
            json!({"name":"relay_registry_list","arguments":{"prefix":"project."}}),
        );
        let (reply, calls) = exchange(
            &call,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_registry_describe")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert!(reply.contains("-32020"));
        assert_eq!(calls, 0);
        let old = mcp_body(6, "tools/list", json!({})).replace(MCP_PROTOCOL_VERSION, "2025-11-25");
        let headers = mcp_headers("tools/list", None).replace(MCP_PROTOCOL_VERSION, "2025-11-25");
        let (reply, calls) = exchange(&old, "/mcp", &headers, Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert!(reply.contains("-32022"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn mcp_lists_and_reads_only_scoped_result_metadata_and_context() {
        let list = mcp_body(7, "tools/list", json!({}));
        let (reply, calls) = exchange(
            &list,
            "/mcp",
            &mcp_headers("tools/list", None),
            Some(TOKEN),
            None,
        );
        assert!(reply.contains("relay_result_list"));
        assert!(reply.contains("relay_result_describe"));
        assert!(reply.contains("relay_result_context"));
        assert!(!reply.contains("relay_result_get"));
        assert_eq!(calls, 1);

        let list = mcp_body(
            8,
            "tools/call",
            json!({"name":"relay_result_list",
            "arguments":{"project_id":"PRJ-alpha","limit":2}}),
        );
        let (reply, calls) = exchange(
            &list,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_result_list")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("RES-alpha"));
        assert!(!reply.contains("payload_json"));
        assert_eq!(calls, 2);

        let context = mcp_body(
            9,
            "tools/call",
            json!({"name":"relay_result_context",
            "arguments":{"project_id":"PRJ-alpha","result_id":"RES-alpha","max_bytes":1024,
                "required_pointers":["/count"]}}),
        );
        let (reply, calls) = exchange(
            &context,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_result_context")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("structuredContent"));
        assert!(reply.contains("/count"));
        assert!(!reply.contains("payload_json"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn scoped_result_reads_fail_closed_before_cross_project_context() {
        let context = mcp_body(
            10,
            "tools/call",
            json!({"name":"relay_result_context",
            "arguments":{"project_id":"PRJ-other","result_id":"RES-alpha","max_bytes":1024}}),
        );
        let (reply, calls) = exchange(
            &context,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_result_context")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 503"));
        assert!(!reply.contains("/count"));
        assert_eq!(calls, 2);

        let oversize = mcp_body(
            11,
            "tools/call",
            json!({"name":"relay_result_context",
            "arguments":{"project_id":"PRJ-alpha","result_id":"RES-alpha","max_bytes":16384}}),
        );
        let (reply, calls) = exchange(
            &oversize,
            "/mcp",
            &mcp_headers("tools/call", Some("relay_result_context")),
            Some(TOKEN),
            None,
        );
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);

        let unscoped = json!({"gateway_protocol":1,"command":"result.list","command_version":1,
            "arguments":{"limit":2}})
        .to_string();
        let (reply, calls) = exchange(&unscoped, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 400"));
        assert_eq!(calls, 0);

        let mismatch = json!({"gateway_protocol":1,"command":"result.list","command_version":1,
            "arguments":{"project_id":"PRJ-other","limit":2}})
        .to_string();
        let (reply, calls) = exchange(&mismatch, "/v1/execute", "", Some(TOKEN), None);
        assert!(reply.starts_with("HTTP/1.1 502"));
        assert!(!reply.contains("RES-alpha"));
        assert_eq!(calls, 2);
    }

    #[test]
    fn local_server_stops_after_shutdown_signal() {
        let transport = Arc::new(FixtureTransport {
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let gateway = Gateway::bind(
            GatewayConfig::new(TOKEN.as_bytes().to_vec(), 0).unwrap(),
            transport,
        )
        .unwrap();
        let address = gateway.local_addr();
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = shutdown.clone();
        let worker = thread::spawn(move || gateway.serve_until(&signal).unwrap());
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let request = format!(
            "POST /v1/negotiate HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
            address.port()
        );
        client.write_all(request.as_bytes()).unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 401"));
        shutdown.store(true, Ordering::Relaxed);
        worker.join().unwrap();
    }
}
