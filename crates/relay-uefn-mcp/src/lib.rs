//! Bounded, read-only connection to Epic's local UEFN MCP server.
//!
//! This client implements the legacy Streamable HTTP lifecycle only. It never
//! calls an editor tool or the `call_tool` dispatcher. Server responses are
//! untrusted data and must not be copied into command diagnostics.

use serde_json::{Value, json};
use std::fmt;
use std::time::Duration;
use ureq::Agent;

const OFFERED_VERSION: &str = "2025-11-25";
const SUPPORTED_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
const MAX_BODY_BYTES: usize = 256 * 1024;
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const MAX_SESSION_ID_BYTES: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 128;
const MAX_TOOL_PAGES: usize = 4;
const MAX_TOOLS: usize = 128;

/// Explicit local endpoint. Host, scheme, and redirect policy cannot be changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpoint {
    port: u16,
    path: String,
}

impl LocalEndpoint {
    pub fn new(port: u16, path: &str) -> Result<Self, ClientError> {
        if port == 0
            || !path.starts_with('/')
            || path.len() > 128
            || path.contains("//")
            || path.split('/').any(|part| part == "." || part == "..")
            || !path.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
            })
        {
            return Err(ClientError::InvalidEndpoint);
        }
        Ok(Self {
            port,
            path: path.to_owned(),
        })
    }

    pub fn epic_default() -> Self {
        Self {
            port: 8000,
            path: "/mcp".to_owned(),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.port, self.path)
    }
}

/// Error categories are deliberately free of remote text and local paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    InvalidEndpoint,
    Transport,
    HttpStatus(u16),
    ResponseTooLarge,
    MalformedResponse,
    UnsupportedContentType,
    UnsupportedProtocol,
    UnexpectedServer,
    ToolsUnavailable,
    ToolLimit,
    ToolNotAdvertised,
    RemoteError,
    InvalidDiscoveryArguments,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint => f.write_str("invalid local UEFN MCP endpoint"),
            Self::Transport => f.write_str("UEFN MCP transport unavailable"),
            Self::HttpStatus(status) => write!(f, "UEFN MCP HTTP status {status}"),
            Self::ResponseTooLarge => f.write_str("UEFN MCP response exceeds limit"),
            Self::MalformedResponse => f.write_str("malformed UEFN MCP response"),
            Self::UnsupportedContentType => {
                f.write_str("unsupported UEFN MCP response content type")
            }
            Self::UnsupportedProtocol => f.write_str(
                "UEFN MCP protocol version unsupported; stateless MCP is not implemented",
            ),
            Self::UnexpectedServer => f.write_str("local MCP server is not UEFN Unreal MCP"),
            Self::ToolsUnavailable => f.write_str("UEFN MCP tools capability unavailable"),
            Self::ToolLimit => f.write_str("UEFN MCP tool catalog exceeds limit"),
            Self::ToolNotAdvertised => f.write_str("UEFN MCP discovery tool is not advertised"),
            Self::RemoteError => f.write_str("UEFN MCP server reported an error"),
            Self::InvalidDiscoveryArguments => f.write_str("invalid UEFN MCP discovery arguments"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Known read-only discovery meta-tools documented by Epic. No editor tool
/// name or argument schema is assumed by this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryTool {
    ListToolsets,
    DescribeToolset,
}

impl DiscoveryTool {
    fn name(self) -> &'static str {
        match self {
            Self::ListToolsets => "list_toolsets",
            Self::DescribeToolset => "describe_toolset",
        }
    }
}

/// Names are returned for exact capability matching, never as command text.
#[derive(Debug, Clone)]
pub struct ToolCatalog {
    pub names: Vec<String>,
    pub pages: usize,
}

impl ToolCatalog {
    pub fn has(&self, name: &str) -> bool {
        self.names.iter().any(|candidate| candidate == name)
    }
}

/// An opaque, bounded result from a read-only discovery meta-tool. Its data is
/// untrusted and must not be written to logs or user-facing diagnostics.
#[derive(Debug, Clone)]
pub struct UntrustedDiscoveryResult {
    value: Value,
}

impl UntrustedDiscoveryResult {
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

/// A connected legacy MCP session. The caller should keep it serialized.
pub struct UefnMcpClient {
    endpoint: LocalEndpoint,
    agent: Agent,
    protocol_version: &'static str,
    session_id: Option<String>,
    next_id: u64,
}

impl UefnMcpClient {
    pub fn connect(endpoint: LocalEndpoint) -> Result<Self, ClientError> {
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .timeout_connect(Some(Duration::from_secs(2)))
            .timeout_recv_body(Some(Duration::from_secs(5)))
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .proxy(None)
            .build();
        let mut client = Self {
            endpoint,
            agent: config.into(),
            protocol_version: OFFERED_VERSION,
            session_id: None,
            next_id: 1,
        };
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": OFFERED_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "relay", "version": env!("CARGO_PKG_VERSION")}
            }
        });
        let response = client.post(&request, false, true)?;
        let result = rpc_result(&response.value, 1)?;
        let version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or(ClientError::MalformedResponse)?;
        client.protocol_version = SUPPORTED_VERSIONS
            .iter()
            .copied()
            .find(|known| *known == version)
            .ok_or(ClientError::UnsupportedProtocol)?;
        let server_name = result
            .pointer("/serverInfo/name")
            .and_then(Value::as_str)
            .ok_or(ClientError::MalformedResponse)?;
        if server_name.is_empty() || server_name.len() > 128 {
            return Err(ClientError::MalformedResponse);
        }
        if !result
            .get("capabilities")
            .and_then(|caps| caps.get("tools"))
            .is_some_and(Value::is_object)
        {
            return Err(ClientError::ToolsUnavailable);
        }
        client.session_id = response.session_id;
        client.post(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            true,
            false,
        )?;
        client.next_id = 2;
        Ok(client)
    }

    pub fn protocol_version(&self) -> &'static str {
        self.protocol_version
    }

    /// Enumerate advertised tool names, including Epic's compact search tools.
    /// Pagination is bounded and no descriptions or schemas enter diagnostics.
    pub fn list_tools(&mut self) -> Result<ToolCatalog, ClientError> {
        let mut names = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        loop {
            if pages == MAX_TOOL_PAGES {
                return Err(ClientError::ToolLimit);
            }
            pages += 1;
            let params = match &cursor {
                Some(cursor) => json!({"cursor": cursor}),
                None => json!({}),
            };
            let result = self.request("tools/list", params)?;
            let tools = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or(ClientError::MalformedResponse)?;
            if names.len().saturating_add(tools.len()) > MAX_TOOLS {
                return Err(ClientError::ToolLimit);
            }
            for tool in tools {
                let name = tool
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| valid_tool_name(name))
                    .ok_or(ClientError::MalformedResponse)?;
                if !names.iter().any(|existing| existing == name) {
                    names.push(name.to_owned());
                }
            }
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() && next.len() <= 256 => {
                    if cursor.as_deref() == Some(next) {
                        return Err(ClientError::MalformedResponse);
                    }
                    cursor = Some(next.to_owned());
                }
                Some(_) => return Err(ClientError::MalformedResponse),
                None => break,
            }
        }
        Ok(ToolCatalog { names, pages })
    }

    /// Invoke only one of Epic's documented discovery meta-tools. The caller
    /// supplies arguments from the live advertised schema; none are invented.
    pub fn call_discovery(
        &mut self,
        catalog: &ToolCatalog,
        tool: DiscoveryTool,
        arguments: Value,
    ) -> Result<UntrustedDiscoveryResult, ClientError> {
        if !catalog.has(tool.name()) {
            return Err(ClientError::ToolNotAdvertised);
        }
        let object = arguments
            .as_object()
            .ok_or(ClientError::InvalidDiscoveryArguments)?;
        if object.len() > 8 || serde_json::to_vec(&arguments).map_or(true, |v| v.len() > 2048) {
            return Err(ClientError::InvalidDiscoveryArguments);
        }
        let result = self.request(
            "tools/call",
            json!({"name": tool.name(), "arguments": arguments}),
        )?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(ClientError::RemoteError);
        }
        Ok(UntrustedDiscoveryResult { value: result })
    }

    fn request(&mut self, method: &'static str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ClientError::MalformedResponse)?;
        let response = self.post(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
            false,
            false,
        )?;
        Ok(rpc_result(&response.value, id)?.clone())
    }

    fn post(
        &self,
        message: &Value,
        notification: bool,
        initializing: bool,
    ) -> Result<HttpResult, ClientError> {
        let body = serde_json::to_vec(message).map_err(|_| ClientError::MalformedResponse)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ClientError::InvalidDiscoveryArguments);
        }
        let mut request = self
            .agent
            .post(self.endpoint.url())
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .header("Accept-Encoding", "identity");
        if !initializing {
            request = request.header("MCP-Protocol-Version", self.protocol_version);
        }
        if let Some(session) = &self.session_id {
            request = request.header("Mcp-Session-Id", session);
        }
        let mut response = request
            .send(body.as_slice())
            .map_err(|_| ClientError::Transport)?;
        let status = response.status().as_u16();
        if notification {
            return if status == 202 {
                Ok(HttpResult {
                    value: Value::Null,
                    session_id: None,
                })
            } else {
                Err(ClientError::HttpStatus(status))
            };
        }
        let session_id = if initializing {
            response
                .headers()
                .get("Mcp-Session-Id")
                .map(|value| value.to_str().map_err(|_| ClientError::MalformedResponse))
                .transpose()?
                .map(|value| {
                    if value.is_empty()
                        || value.len() > MAX_SESSION_ID_BYTES
                        || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
                    {
                        Err(ClientError::MalformedResponse)
                    } else {
                        Ok(value.to_owned())
                    }
                })
                .transpose()?
        } else {
            None
        };
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES as u64)
            .read_to_vec()
            .map_err(|_| ClientError::ResponseTooLarge)?;
        if status != 200 {
            if content_type.starts_with("application/json")
                && serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|value| value.pointer("/error/code").and_then(Value::as_i64))
                    == Some(-32022)
            {
                return Err(ClientError::UnsupportedProtocol);
            }
            return Err(ClientError::HttpStatus(status));
        }
        let value = if content_type.starts_with("application/json") {
            serde_json::from_slice::<Value>(&bytes).map_err(|_| ClientError::MalformedResponse)?
        } else if content_type.starts_with("text/event-stream") {
            let wanted_id = message
                .get("id")
                .and_then(Value::as_u64)
                .ok_or(ClientError::MalformedResponse)?;
            extract_sse_response(&bytes, wanted_id)?
        } else {
            return Err(ClientError::UnsupportedContentType);
        };
        Ok(HttpResult { value, session_id })
    }
}

struct HttpResult {
    value: Value,
    session_id: Option<String>,
}

fn rpc_result(response: &Value, id: u64) -> Result<&Value, ClientError> {
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || response.get("id").and_then(Value::as_u64) != Some(id)
    {
        return Err(ClientError::MalformedResponse);
    }
    if response.get("error").is_some() {
        if response.pointer("/error/code").and_then(Value::as_i64) == Some(-32022) {
            return Err(ClientError::UnsupportedProtocol);
        }
        return Err(ClientError::RemoteError);
    }
    response.get("result").ok_or(ClientError::MalformedResponse)
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TOOL_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\' && byte != b'"')
}

fn extract_sse_response(bytes: &[u8], id: u64) -> Result<Value, ClientError> {
    let stream = std::str::from_utf8(bytes).map_err(|_| ClientError::MalformedResponse)?;
    let mut data = String::new();
    for line in stream.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !data.is_empty() {
                let value: Value =
                    serde_json::from_str(&data).map_err(|_| ClientError::MalformedResponse)?;
                if value.get("id").and_then(Value::as_u64) == Some(id) {
                    return Ok(value);
                }
                data.clear();
            }
        } else if let Some(content) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(content.trim_start_matches(' '));
        }
    }
    Err(ClientError::MalformedResponse)
}
