//! Bounded connection to Epic's local UEFN MCP server.
//!
//! The private invocation primitive below requires fresh toolset discovery,
//! strict schema validation, and a caller-declared effect. It is not a public
//! RELAY command or authorization decision. Server text stays out of Debug.

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
const MAX_TOOLSETS: usize = 64;
const MAX_TOOLSET_TOOLS: usize = 64;
const MAX_SCHEMA_PROPERTIES: usize = 32;
const MAX_INVOCATION_ARGUMENT_BYTES: usize = 2 * 1024;
const MAX_TOOL_RESULT_BYTES: usize = 64 * 1024;
const MAX_SCHEMA_DEPTH: usize = 4;
const MAX_SAFE_JSON_INTEGER: f64 = 9_007_199_254_740_991.0;

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
    UnsupportedDiscoveryShape,
    InvalidToolArguments,
    UnsupportedToolSchema,
    UnsupportedDispatcherSchema,
    ToolResultTooLarge,
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
            Self::UnsupportedDiscoveryShape => {
                f.write_str("UEFN MCP discovery result shape is not supported")
            }
            Self::InvalidToolArguments => f.write_str("invalid UEFN MCP tool arguments"),
            Self::UnsupportedToolSchema => f.write_str("UEFN MCP tool schema is unsupported"),
            Self::UnsupportedDispatcherSchema => {
                f.write_str("UEFN MCP dispatcher schema is unsupported")
            }
            Self::ToolResultTooLarge => f.write_str("UEFN MCP tool result exceeds limit"),
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
#[derive(Clone)]
pub struct ToolCatalog {
    pub names: Vec<String>,
    pub pages: usize,
    discovery_schemas: [Option<Value>; 3],
}

/// Compact metadata from `list_toolsets`. No descriptions or raw server text
/// are retained, and a localhost MCP response does not prove editor identity.
#[derive(Debug, Clone)]
pub struct ToolsetCatalogSummary {
    pub names: Vec<String>,
    pub server_identity_verified: bool,
}

#[derive(Debug, Clone)]
pub struct ToolsetSchemaSummary {
    pub name: String,
    pub tools: Vec<ToolSchemaSummary>,
    pub server_identity_verified: bool,
}

#[derive(Debug, Clone)]
pub struct ToolSchemaSummary {
    pub name: String,
    pub parameters: Vec<ParameterSummary>,
}

#[derive(Debug, Clone)]
pub struct ParameterSummary {
    pub name: String,
    pub kind: &'static str,
    pub required: bool,
}

impl ToolCatalog {
    pub fn has(&self, name: &str) -> bool {
        self.names.iter().any(|candidate| candidate == name)
    }
}

/// An opaque, bounded result from a read-only discovery meta-tool. Its data is
/// untrusted and must not be written to logs or user-facing diagnostics.
#[derive(Clone)]
pub struct UntrustedDiscoveryResult {
    value: Value,
}

impl UntrustedDiscoveryResult {
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

/// The caller must classify the intended effect before any editor call. This
/// declaration does not grant authority or prove the tool's actual behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredEffect {
    Observe,
    Analyze,
    EditorWrite,
    SessionControl,
}

/// Never derives Debug or Serialize: raw editor text is for an authorized
/// result store, not logs or compact diagnostics.
pub struct UntrustedToolResult {
    value: Value,
    effect: DeclaredEffect,
    bytes: usize,
}

impl UntrustedToolResult {
    pub fn declared_effect(&self) -> DeclaredEffect {
        self.effect
    }

    pub fn byte_len(&self) -> usize {
        self.bytes
    }

    pub fn into_value_for_storage(self) -> Value {
        self.value
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
        // Live Unreal builds can leave serverInfo.name empty. It is display
        // metadata, never editor identity or authority for this client.
        if server_name.len() > 128 {
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
        let mut discovery_schemas = [None, None, None];
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
                let schema_index = match name {
                    "list_toolsets" => Some(0),
                    "describe_toolset" => Some(1),
                    "call_tool" => Some(2),
                    _ => None,
                };
                if let Some(index) = schema_index {
                    if discovery_schemas[index].is_none() {
                        discovery_schemas[index] = tool.get("inputSchema").cloned();
                    }
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
        Ok(ToolCatalog {
            names,
            pages,
            discovery_schemas,
        })
    }

    /// Ask only Epic's advertised `list_toolsets` discovery meta-tool.
    /// Unsupported live schemas or response shapes fail closed.
    pub fn list_toolset_summaries(&mut self) -> Result<ToolsetCatalogSummary, ClientError> {
        let catalog = self.list_tools()?;
        require_empty_arguments(catalog.discovery_schemas[0].as_ref())?;
        let result = self.call_discovery(&catalog, DiscoveryTool::ListToolsets, json!({}))?;
        Ok(ToolsetCatalogSummary {
            names: parse_toolset_names(&result)?,
            server_identity_verified: false,
        })
    }

    /// Summarize one advertised toolset's input schemas. This never invokes
    /// `call_tool` or any editor capability.
    pub fn describe_toolset_summary(
        &mut self,
        toolset_name: &str,
    ) -> Result<ToolsetSchemaSummary, ClientError> {
        if !valid_discovery_name(toolset_name) {
            return Err(ClientError::InvalidDiscoveryArguments);
        }
        let catalog = self.list_tools()?;
        require_empty_arguments(catalog.discovery_schemas[0].as_ref())?;
        let listed = self.call_discovery(&catalog, DiscoveryTool::ListToolsets, json!({}))?;
        if !parse_toolset_names(&listed)?
            .iter()
            .any(|name| name == toolset_name)
        {
            return Err(ClientError::ToolNotAdvertised);
        }
        let args = named_argument(catalog.discovery_schemas[1].as_ref(), toolset_name)?;
        let result = self.call_discovery(&catalog, DiscoveryTool::DescribeToolset, args)?;
        parse_toolset_schema(&result, toolset_name)
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
        match result.get("isError") {
            Some(Value::Bool(true)) => return Err(ClientError::RemoteError),
            Some(Value::Bool(false)) | None => {}
            Some(_) => return Err(ClientError::MalformedResponse),
        }
        Ok(UntrustedDiscoveryResult { value: result })
    }

    /// Private editor-tool primitive for a future authorized workflow. It
    /// re-discovers the toolset on every call and never infers an effect from
    /// a tool name or server prose. The caller's effect declaration is not a
    /// permission grant; Core must authorize it before calling this method.
    pub fn invoke_explicit_tool(
        &mut self,
        toolset_name: &str,
        tool_name: &str,
        arguments: Value,
        effect: DeclaredEffect,
    ) -> Result<UntrustedToolResult, ClientError> {
        if !valid_discovery_name(toolset_name)
            || !valid_discovery_name(tool_name)
            || !arguments.is_object()
            || serde_json::to_vec(&arguments)
                .map_or(true, |bytes| bytes.len() > MAX_INVOCATION_ARGUMENT_BYTES)
        {
            return Err(ClientError::InvalidToolArguments);
        }
        let catalog = self.list_tools()?;
        if !catalog.has("call_tool") {
            return Err(ClientError::ToolNotAdvertised);
        }
        require_empty_arguments(catalog.discovery_schemas[0].as_ref())?;
        let listed = self.call_discovery(&catalog, DiscoveryTool::ListToolsets, json!({}))?;
        if !parse_toolset_names(&listed)?
            .iter()
            .any(|name| name == toolset_name)
        {
            return Err(ClientError::ToolNotAdvertised);
        }
        let describe_args = named_argument(catalog.discovery_schemas[1].as_ref(), toolset_name)?;
        let described =
            self.call_discovery(&catalog, DiscoveryTool::DescribeToolset, describe_args)?;
        let tool_schema = exact_tool_schema(&described, toolset_name, tool_name)?;
        check_supported_tool_schema(&tool_schema, 0)?;
        if !matches_tool_schema(&tool_schema, &arguments) {
            return Err(ClientError::InvalidToolArguments);
        }
        let dispatcher_args = dispatcher_arguments(
            catalog.discovery_schemas[2].as_ref(),
            toolset_name,
            tool_name,
            arguments,
        )?;
        let result = self.request(
            "tools/call",
            json!({"name": "call_tool", "arguments": dispatcher_args}),
        )?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(ClientError::RemoteError);
        }
        let bytes = serde_json::to_vec(&result)
            .map_err(|_| ClientError::MalformedResponse)?
            .len();
        if bytes > MAX_TOOL_RESULT_BYTES {
            return Err(ClientError::ToolResultTooLarge);
        }
        if result.get("content").and_then(Value::as_array).is_none()
            && result
                .get("structuredContent")
                .and_then(Value::as_object)
                .is_none()
        {
            return Err(ClientError::MalformedResponse);
        }
        Ok(UntrustedToolResult {
            value: result,
            effect,
            bytes,
        })
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

fn valid_discovery_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn require_empty_arguments(schema: Option<&Value>) -> Result<(), ClientError> {
    let schema = schema.ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .ok_or(ClientError::UnsupportedDiscoveryShape)?;
        if !required.is_empty() {
            return Err(ClientError::UnsupportedDiscoveryShape);
        }
    }
    Ok(())
}

fn named_argument(schema: Option<&Value>, name: &str) -> Result<Value, ClientError> {
    let schema = schema.ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if properties.is_empty() || properties.len() > 8 {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let required = schema
        .get("required")
        .map(|value| {
            value
                .as_array()
                .ok_or(ClientError::UnsupportedDiscoveryShape)
        })
        .transpose()?;
    let field = match required {
        Some(required) if required.len() == 1 => required[0].as_str(),
        None if properties.len() == 1 => properties.keys().next().map(String::as_str),
        _ => None,
    }
    .filter(|field| valid_discovery_name(field))
    .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if properties
        .get(field)
        .and_then(|property| property.get("type"))
        .and_then(Value::as_str)
        != Some("string")
    {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let mut arguments = serde_json::Map::new();
    arguments.insert(field.to_owned(), Value::String(name.to_owned()));
    Ok(Value::Object(arguments))
}

fn discovery_payload(result: &UntrustedDiscoveryResult) -> Result<Value, ClientError> {
    let value = result.as_value();
    if let Some(structured) = value
        .get("structuredContent")
        .filter(|value| value.is_object())
    {
        return Ok(structured.clone());
    }
    serde_json::from_str(discovery_text(result)?)
        .map_err(|_| ClientError::UnsupportedDiscoveryShape)
}

fn discovery_text(result: &UntrustedDiscoveryResult) -> Result<&str, ClientError> {
    let content = result
        .as_value()
        .get("content")
        .and_then(Value::as_array)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if content.len() != 1 {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let text = content[0]
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| *kind == "text")
        .and_then(|_| content[0].get("text"))
        .and_then(Value::as_str)
        .filter(|text| text.len() <= MAX_BODY_BYTES)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    Ok(text)
}

fn parse_toolset_names(result: &UntrustedDiscoveryResult) -> Result<Vec<String>, ClientError> {
    let payload = match discovery_payload(result) {
        Ok(payload) => payload,
        Err(ClientError::UnsupportedDiscoveryShape)
            if result.as_value().get("structuredContent").is_none() =>
        {
            // Unreal's live list_toolsets returns Markdown entries with prose
            // continuation lines. Retain validated entry names only.
            let text = discovery_text(result)?;
            if !text.starts_with("- ") {
                return Err(ClientError::UnsupportedDiscoveryShape);
            }
            let mut names = Vec::new();
            for line in text.lines() {
                let Some(entry) = line.strip_prefix("- ") else {
                    continue;
                };
                let Some((name, _)) = entry.split_once(": ") else {
                    continue;
                };
                if !valid_discovery_name(name) {
                    continue;
                }
                if names.iter().any(|existing| existing == name) {
                    return Err(ClientError::UnsupportedDiscoveryShape);
                }
                if names.len() == MAX_TOOLSETS {
                    return Err(ClientError::ToolLimit);
                }
                names.push(name.to_owned());
            }
            if names.is_empty() {
                return Err(ClientError::UnsupportedDiscoveryShape);
            }
            return Ok(names);
        }
        Err(error) => return Err(error),
    };
    let toolsets = payload
        .get("toolsets")
        .and_then(Value::as_array)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if toolsets.len() > MAX_TOOLSETS {
        return Err(ClientError::ToolLimit);
    }
    let mut names = Vec::new();
    for item in toolsets {
        let name = item
            .as_str()
            .or_else(|| item.get("name").and_then(Value::as_str))
            .filter(|name| valid_discovery_name(name))
            .ok_or(ClientError::UnsupportedDiscoveryShape)?;
        if names.iter().any(|existing| existing == name) {
            return Err(ClientError::UnsupportedDiscoveryShape);
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

fn parse_toolset_schema(
    result: &UntrustedDiscoveryResult,
    requested_name: &str,
) -> Result<ToolsetSchemaSummary, ClientError> {
    let payload = discovery_payload(result)?;
    let toolset = payload
        .get("toolset")
        .filter(|value| value.is_object())
        .unwrap_or(&payload);
    if toolset.get("name").and_then(Value::as_str) != Some(requested_name) {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let tools = toolset
        .get("tools")
        .and_then(Value::as_array)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    if tools.len() > MAX_TOOLSET_TOOLS {
        return Err(ClientError::ToolLimit);
    }
    let mut summaries = Vec::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| valid_discovery_name(name))
            .ok_or(ClientError::UnsupportedDiscoveryShape)?;
        let schema = tool
            .get("inputSchema")
            .filter(|schema| schema.is_object())
            .ok_or(ClientError::UnsupportedDiscoveryShape)?;
        if schema.get("type").and_then(Value::as_str) != Some("object") {
            return Err(ClientError::UnsupportedDiscoveryShape);
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        if properties.is_some_and(|properties| properties.len() > MAX_SCHEMA_PROPERTIES) {
            return Err(ClientError::ToolLimit);
        }
        let mut required = Vec::new();
        if let Some(items) = schema.get("required") {
            let items = items
                .as_array()
                .ok_or(ClientError::UnsupportedDiscoveryShape)?;
            if items.len() > MAX_SCHEMA_PROPERTIES {
                return Err(ClientError::ToolLimit);
            }
            for item in items {
                let field = item
                    .as_str()
                    .filter(|name| valid_discovery_name(name))
                    .ok_or(ClientError::UnsupportedDiscoveryShape)?;
                required.push(field);
            }
        }
        let mut parameters = Vec::new();
        if let Some(properties) = properties {
            for (field, property) in properties {
                if !valid_discovery_name(field) || !property.is_object() {
                    return Err(ClientError::UnsupportedDiscoveryShape);
                }
                let kind = match property.get("type").and_then(Value::as_str) {
                    Some("string") => "string",
                    Some("integer") => "integer",
                    Some("number") => "number",
                    Some("boolean") => "boolean",
                    Some("array") => "array",
                    Some("object") => "object",
                    _ => "other",
                };
                parameters.push(ParameterSummary {
                    name: field.clone(),
                    kind,
                    required: required.contains(&field.as_str()),
                });
            }
        }
        if required.len() > parameters.len()
            || required
                .iter()
                .any(|field| !parameters.iter().any(|p| p.name == *field))
        {
            return Err(ClientError::UnsupportedDiscoveryShape);
        }
        summaries.push(ToolSchemaSummary {
            name: name.to_owned(),
            parameters,
        });
    }
    Ok(ToolsetSchemaSummary {
        name: requested_name.to_owned(),
        tools: summaries,
        server_identity_verified: false,
    })
}

fn exact_tool_schema(
    result: &UntrustedDiscoveryResult,
    toolset_name: &str,
    tool_name: &str,
) -> Result<Value, ClientError> {
    let payload = discovery_payload(result)?;
    let toolset = payload.get("toolset").unwrap_or(&payload);
    if toolset.get("name").and_then(Value::as_str) != Some(toolset_name) {
        return Err(ClientError::UnsupportedDiscoveryShape);
    }
    let tools = toolset
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| tools.len() <= MAX_TOOLSET_TOOLS)
        .ok_or(ClientError::UnsupportedDiscoveryShape)?;
    let mut names = std::collections::HashSet::new();
    let mut selected = None;
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| valid_discovery_name(name))
            .ok_or(ClientError::UnsupportedDiscoveryShape)?;
        if !names.insert(name) {
            return Err(ClientError::UnsupportedDiscoveryShape);
        }
        if name == tool_name {
            selected = Some(
                tool.get("inputSchema")
                    .filter(|schema| schema.is_object())
                    .ok_or(ClientError::UnsupportedToolSchema)?
                    .clone(),
            );
        }
    }
    selected.ok_or(ClientError::ToolNotAdvertised)
}

fn check_supported_tool_schema(schema: &Value, depth: usize) -> Result<(), ClientError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(ClientError::UnsupportedToolSchema);
    }
    let object = schema
        .as_object()
        .ok_or(ClientError::UnsupportedToolSchema)?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or(ClientError::UnsupportedToolSchema)?;
    let allowed: &[&str] = match kind {
        "object" => &[
            "type",
            "title",
            "description",
            "properties",
            "required",
            "additionalProperties",
        ],
        "array" => &[
            "type",
            "title",
            "description",
            "items",
            "minItems",
            "maxItems",
            "uniqueItems",
        ],
        "string" => &[
            "type",
            "title",
            "description",
            "minLength",
            "maxLength",
            "enum",
            "const",
        ],
        "integer" | "number" => &[
            "type",
            "title",
            "description",
            "minimum",
            "maximum",
            "enum",
            "const",
        ],
        "boolean" => &["type", "title", "description", "enum", "const"],
        _ => return Err(ClientError::UnsupportedToolSchema),
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || ["title", "description"]
            .iter()
            .any(|key| object.get(*key).is_some_and(|value| !value.is_string()))
    {
        return Err(ClientError::UnsupportedToolSchema);
    }
    match kind {
        "object" => {
            if object.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
                return Err(ClientError::UnsupportedToolSchema);
            }
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(ClientError::UnsupportedToolSchema)?;
            if properties.len() > MAX_SCHEMA_PROPERTIES {
                return Err(ClientError::UnsupportedToolSchema);
            }
            let empty_required = Vec::new();
            let required = match object.get("required") {
                Some(value) => value.as_array().ok_or(ClientError::UnsupportedToolSchema)?,
                None => &empty_required,
            };
            if required.len() > MAX_SCHEMA_PROPERTIES {
                return Err(ClientError::UnsupportedToolSchema);
            }
            let mut seen = std::collections::HashSet::new();
            for field in required {
                let name = field
                    .as_str()
                    .filter(|name| valid_discovery_name(name))
                    .ok_or(ClientError::UnsupportedToolSchema)?;
                if !properties.contains_key(name) || !seen.insert(name) {
                    return Err(ClientError::UnsupportedToolSchema);
                }
            }
            for (name, property) in properties {
                if !valid_discovery_name(name) {
                    return Err(ClientError::UnsupportedToolSchema);
                }
                check_supported_tool_schema(property, depth + 1)?;
            }
        }
        "array" => {
            let max = object
                .get("maxItems")
                .and_then(Value::as_u64)
                .filter(|value| *value <= 32)
                .ok_or(ClientError::UnsupportedToolSchema)?;
            let min = object
                .get("minItems")
                .map(Value::as_u64)
                .unwrap_or(Some(0))
                .ok_or(ClientError::UnsupportedToolSchema)?;
            if min > max
                || object
                    .get("uniqueItems")
                    .is_some_and(|value| !value.is_boolean())
            {
                return Err(ClientError::UnsupportedToolSchema);
            }
            check_supported_tool_schema(
                object
                    .get("items")
                    .ok_or(ClientError::UnsupportedToolSchema)?,
                depth + 1,
            )?;
        }
        "string" => {
            let min = object
                .get("minLength")
                .map(Value::as_u64)
                .unwrap_or(Some(0))
                .ok_or(ClientError::UnsupportedToolSchema)?;
            let max = object
                .get("maxLength")
                .map(Value::as_u64)
                .unwrap_or(Some(2048))
                .ok_or(ClientError::UnsupportedToolSchema)?;
            if min > max || max > 2048 {
                return Err(ClientError::UnsupportedToolSchema);
            }
        }
        "integer" | "number" => {
            let min = optional_number(object.get("minimum"))?;
            let max = optional_number(object.get("maximum"))?;
            if min.zip(max).is_some_and(|(min, max)| min > max)
                || [min, max]
                    .into_iter()
                    .flatten()
                    .any(|bound| !bound.is_finite() || bound.abs() > MAX_SAFE_JSON_INTEGER)
            {
                return Err(ClientError::UnsupportedToolSchema);
            }
        }
        "boolean" => {}
        _ => unreachable!(),
    }
    if let Some(values) = object.get("enum") {
        let values = values
            .as_array()
            .filter(|values| !values.is_empty() && values.len() <= 32)
            .ok_or(ClientError::UnsupportedToolSchema)?;
        if values.iter().any(|value| !value_matches_type(kind, value)) {
            return Err(ClientError::UnsupportedToolSchema);
        }
    }
    if object
        .get("const")
        .is_some_and(|value| !value_matches_type(kind, value))
    {
        return Err(ClientError::UnsupportedToolSchema);
    }
    Ok(())
}

fn optional_number(value: Option<&Value>) -> Result<Option<f64>, ClientError> {
    match value {
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or(ClientError::UnsupportedToolSchema),
        None => Ok(None),
    }
}

fn value_matches_type(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        _ => false,
    }
}

fn matches_tool_schema(schema: &Value, value: &Value) -> bool {
    let Some(object) = schema.as_object() else {
        return false;
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        return false;
    };
    if !value_matches_type(kind, value)
        || object
            .get("const")
            .is_some_and(|expected| expected != value)
        || object
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.contains(value))
    {
        return false;
    }
    match kind {
        "object" => {
            let Some(instance) = value.as_object() else {
                return false;
            };
            let Some(properties) = object.get("properties").and_then(Value::as_object) else {
                return false;
            };
            if instance.keys().any(|name| !properties.contains_key(name)) {
                return false;
            }
            if object
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|fields| {
                    fields
                        .iter()
                        .any(|field| !instance.contains_key(field.as_str().unwrap_or("")))
                })
            {
                return false;
            }
            instance.iter().all(|(name, child)| {
                properties
                    .get(name)
                    .is_some_and(|schema| matches_tool_schema(schema, child))
            })
        }
        "array" => {
            let Some(items) = value.as_array() else {
                return false;
            };
            let min = object.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            let max = object.get("maxItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            if items.len() < min || items.len() > max {
                return false;
            }
            if object.get("uniqueItems").and_then(Value::as_bool) == Some(true)
                && items
                    .iter()
                    .enumerate()
                    .any(|(index, item)| items[..index].contains(item))
            {
                return false;
            }
            object
                .get("items")
                .is_some_and(|schema| items.iter().all(|item| matches_tool_schema(schema, item)))
        }
        "string" => {
            let text = value.as_str().unwrap_or_default();
            let length = text.chars().count() as u64;
            length >= object.get("minLength").and_then(Value::as_u64).unwrap_or(0)
                && length
                    <= object
                        .get("maxLength")
                        .and_then(Value::as_u64)
                        .unwrap_or(2048)
        }
        "integer" | "number" => {
            let number = value.as_f64().unwrap_or(f64::NAN);
            if kind == "integer" && (!number.is_finite() || number.abs() > MAX_SAFE_JSON_INTEGER) {
                return false;
            }
            object
                .get("minimum")
                .and_then(Value::as_f64)
                .is_none_or(|min| number >= min)
                && object
                    .get("maximum")
                    .and_then(Value::as_f64)
                    .is_none_or(|max| number <= max)
        }
        "boolean" => true,
        _ => false,
    }
}

fn dispatcher_arguments(
    schema: Option<&Value>,
    toolset: &str,
    tool: &str,
    arguments: Value,
) -> Result<Value, ClientError> {
    let object = schema
        .and_then(Value::as_object)
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    if object.get("type").and_then(Value::as_str) != Some("object")
        || object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "type"
                    | "properties"
                    | "required"
                    | "additionalProperties"
                    | "description"
                    | "title"
            )
        })
    {
        return Err(ClientError::UnsupportedDispatcherSchema);
    }
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| properties.len() == 3)
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    let toolset_key = ["toolset_name", "toolset"]
        .into_iter()
        .find(|key| properties.contains_key(*key))
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    let tool_key = ["tool_name", "tool"]
        .into_iter()
        .find(|key| properties.contains_key(*key))
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    let args_key = ["arguments", "args"]
        .into_iter()
        .find(|key| properties.contains_key(*key))
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    let required = object
        .get("required")
        .and_then(Value::as_array)
        .filter(|required| required.len() == 3)
        .ok_or(ClientError::UnsupportedDispatcherSchema)?;
    if ![toolset_key, tool_key, args_key]
        .iter()
        .all(|key| required.iter().any(|value| value.as_str() == Some(*key)))
        || ![toolset_key, tool_key].iter().all(|key| {
            properties
                .get(*key)
                .and_then(Value::as_object)
                .is_some_and(|schema| {
                    schema.get("type").and_then(Value::as_str) == Some("string")
                        && schema
                            .keys()
                            .all(|key| matches!(key.as_str(), "type" | "description" | "title"))
                })
        })
        || !properties
            .get(args_key)
            .and_then(Value::as_object)
            .is_some_and(|schema| {
                schema.get("type").and_then(Value::as_str) == Some("object")
                    && schema.keys().all(|key| {
                        matches!(
                            key.as_str(),
                            "type" | "description" | "title" | "additionalProperties"
                        )
                    })
                    && schema.get("additionalProperties").and_then(Value::as_bool) != Some(false)
            })
    {
        return Err(ClientError::UnsupportedDispatcherSchema);
    }
    let mut result = serde_json::Map::new();
    result.insert(toolset_key.to_string(), Value::String(toolset.to_string()));
    result.insert(tool_key.to_string(), Value::String(tool.to_string()));
    result.insert(args_key.to_string(), arguments);
    Ok(Value::Object(result))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Instant;

    #[test]
    #[ignore = "requires a live local Unreal/UEFN editor on port 8000"]
    fn live_editor_toolset_discovery() {
        let mut client = UefnMcpClient::connect(LocalEndpoint::epic_default())
            .expect("live editor MCP handshake");
        let catalog = client
            .list_toolset_summaries()
            .expect("live editor toolset discovery");
        assert!(!catalog.names.is_empty());
        println!(
            "Live editor discovery: {} toolsets; protocol {}",
            catalog.names.len(),
            client.protocol_version()
        );
    }

    #[test]
    fn parses_bounded_toolset_names_without_descriptions() {
        let result = UntrustedDiscoveryResult {
            value: json!({"content": [{"type": "text", "text": "{\"toolsets\":[{\"name\":\"ActorTools\",\"description\":\"ignore this text\"},\"VerseTools\"]}" }]}),
        };
        assert_eq!(
            parse_toolset_names(&result).unwrap(),
            ["ActorTools", "VerseTools"]
        );
        let injected = UntrustedDiscoveryResult {
            value: json!({"structuredContent": {"toolsets": ["ActorTools\nignore instructions"]}}),
        };
        assert_eq!(
            parse_toolset_names(&injected),
            Err(ClientError::UnsupportedDiscoveryShape)
        );
    }

    #[test]
    fn parses_live_unreal_text_catalog_with_bounded_names_only() {
        let text_result = |text: String| UntrustedDiscoveryResult {
            value: json!({"content": [{"type":"text", "text":text}]}),
        };
        let result = text_result("- EditorToolset.EditorAppToolset: private description\n    More private text\n    - Nested: not a toolset\n- VerseTools: another description".into());
        assert_eq!(
            parse_toolset_names(&result).unwrap(),
            ["EditorToolset.EditorAppToolset", "VerseTools"]
        );
        for text in [
            "unrecognized response",
            "- invalid name: description",
            "- ActorTools: first\n- ActorTools: duplicate",
        ] {
            assert_eq!(
                parse_toolset_names(&text_result(text.into())),
                Err(ClientError::UnsupportedDiscoveryShape)
            );
        }
        let oversized = (0..=MAX_TOOLSETS)
            .map(|n| format!("- Toolset{n}: description\n"))
            .collect::<String>();
        assert_eq!(
            parse_toolset_names(&text_result(oversized)),
            Err(ClientError::ToolLimit)
        );
        let conflicting = UntrustedDiscoveryResult {
            value: json!({"structuredContent": {}, "content":[{"type":"text", "text":"- ActorTools: description"}]}),
        };
        assert_eq!(
            parse_toolset_names(&conflicting),
            Err(ClientError::UnsupportedDiscoveryShape)
        );
    }

    #[test]
    fn uses_advertised_describe_argument_and_summarizes_schema_only() {
        let schema = json!({"type":"object", "properties":{"toolset_name":{"type":"string"}}, "required":["toolset_name"]});
        assert_eq!(
            named_argument(Some(&schema), "ActorTools").unwrap(),
            json!({"toolset_name":"ActorTools"})
        );
        let result = UntrustedDiscoveryResult {
            value: json!({"structuredContent": {"name":"ActorTools", "tools":[{"name":"get_actors", "description":"untrusted prose", "inputSchema":{"type":"object", "properties":{"limit":{"type":"integer"}}, "required":["limit"]}}]}}),
        };
        let summary = parse_toolset_schema(&result, "ActorTools").unwrap();
        assert_eq!(summary.tools.len(), 1);
        assert_eq!(summary.tools[0].name, "get_actors");
        assert_eq!(summary.tools[0].parameters[0].kind, "integer");
        assert!(summary.tools[0].parameters[0].required);
        assert!(!summary.server_identity_verified);
    }

    fn fake_tool_schema() -> Value {
        json!({
            "type": "object",
            "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 4}},
            "required": ["limit"],
            "additionalProperties": false
        })
    }

    fn fake_request(stream: &mut TcpStream) -> Value {
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut byte = [0u8; 1];
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
            assert!(head.len() <= 8192);
        }
        let header = std::str::from_utf8(&head).unwrap();
        let length = header
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .unwrap();
        assert!(length <= MAX_REQUEST_BYTES);
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn fake_server(
        schema: Value,
        expected_requests: usize,
    ) -> (LocalEndpoint, thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(6);
            while requests.len() < expected_requests {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("fake MCP accept failed: {error}"),
                };
                let request = fake_request(&mut stream);
                let method = request.get("method").and_then(Value::as_str).unwrap();
                let id = request.get("id").cloned();
                let (status, body) = match method {
                    "initialize" => (
                        200,
                        json!({"jsonrpc":"2.0", "id":id, "result":{
                            "protocolVersion": OFFERED_VERSION,
                            "serverInfo": {"name":"", "version":"fixture"},
                            "capabilities": {"tools": {}}
                        }}),
                    ),
                    "notifications/initialized" => (202, Value::Null),
                    "tools/list" => (
                        200,
                        json!({"jsonrpc":"2.0", "id":id, "result":{
                            "tools": [
                                {"name":"list_toolsets", "inputSchema":{"type":"object", "properties":{}, "required":[], "additionalProperties":false}},
                                {"name":"describe_toolset", "inputSchema":{"type":"object", "properties":{"toolset_name":{"type":"string"}}, "required":["toolset_name"], "additionalProperties":false}},
                                {"name":"call_tool", "inputSchema":{"type":"object", "properties":{
                                    "toolset_name":{"type":"string"}, "tool_name":{"type":"string"},
                                    "arguments":{"type":"object", "additionalProperties":true}
                                }, "required":["toolset_name","tool_name","arguments"], "additionalProperties":false}}
                            ]
                        }}),
                    ),
                    "tools/call" => {
                        let name = request
                            .pointer("/params/name")
                            .and_then(Value::as_str)
                            .unwrap();
                        let result = match name {
                            "list_toolsets" => {
                                json!({"structuredContent":{"toolsets":["ActorTools"]}})
                            }
                            "describe_toolset" => {
                                json!({"structuredContent":{"name":"ActorTools", "tools":[{"name":"get_actors", "inputSchema":schema}]}})
                            }
                            "call_tool" => {
                                json!({"content":[{"type":"text", "text":"private editor fixture text"}]})
                            }
                            _ => panic!("unexpected fake tool call"),
                        };
                        (200, json!({"jsonrpc":"2.0", "id":id, "result":result}))
                    }
                    _ => panic!("unexpected fake MCP method"),
                };
                requests.push(request);
                let payload = if status == 202 {
                    Vec::new()
                } else {
                    serde_json::to_vec(&body).unwrap()
                };
                let headers = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                stream.write_all(headers.as_bytes()).unwrap();
                stream.write_all(&payload).unwrap();
            }
            requests
        });
        (LocalEndpoint::new(port, "/mcp").unwrap(), worker)
    }

    #[test]
    fn explicit_invocation_uses_fresh_toolset_schema_and_keeps_result_opaque() {
        let (endpoint, server) = fake_server(fake_tool_schema(), 6);
        let mut client = UefnMcpClient::connect(endpoint).unwrap();
        let result = client
            .invoke_explicit_tool(
                "ActorTools",
                "get_actors",
                json!({"limit":2}),
                DeclaredEffect::Observe,
            )
            .unwrap();
        assert_eq!(result.declared_effect(), DeclaredEffect::Observe);
        assert!(result.byte_len() < MAX_TOOL_RESULT_BYTES);
        assert_eq!(
            result.into_value_for_storage()["content"][0]["text"],
            "private editor fixture text"
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.last().unwrap()["params"]["name"], "call_tool");
        assert_eq!(
            requests.last().unwrap()["params"]["arguments"]["toolset_name"],
            "ActorTools"
        );
        assert_eq!(
            requests.last().unwrap()["params"]["arguments"]["tool_name"],
            "get_actors"
        );
        assert_eq!(
            requests.last().unwrap()["params"]["arguments"]["arguments"]["limit"],
            2
        );
    }

    #[test]
    fn invalid_arguments_fail_before_editor_dispatch() {
        let (endpoint, server) = fake_server(fake_tool_schema(), 5);
        let mut client = UefnMcpClient::connect(endpoint).unwrap();
        assert!(matches!(
            client.invoke_explicit_tool(
                "ActorTools",
                "get_actors",
                json!({"limit":"many"}),
                DeclaredEffect::Observe
            ),
            Err(ClientError::InvalidToolArguments)
        ));
        let requests = server.join().unwrap();
        assert!(
            !requests
                .iter()
                .any(|request| request.pointer("/params/name") == Some(&json!("call_tool")))
        );
    }

    #[test]
    fn unsupported_schema_fails_before_editor_dispatch() {
        let mut schema = fake_tool_schema();
        schema["properties"]["limit"]["pattern"] = json!(".*");
        let (endpoint, server) = fake_server(schema, 5);
        let mut client = UefnMcpClient::connect(endpoint).unwrap();
        assert!(matches!(
            client.invoke_explicit_tool(
                "ActorTools",
                "get_actors",
                json!({"limit":2}),
                DeclaredEffect::Observe
            ),
            Err(ClientError::UnsupportedToolSchema)
        ));
        let requests = server.join().unwrap();
        assert!(
            !requests
                .iter()
                .any(|request| request.pointer("/params/name") == Some(&json!("call_tool")))
        );
    }

    #[test]
    fn unadvertised_tool_fails_before_editor_dispatch() {
        let (endpoint, server) = fake_server(fake_tool_schema(), 5);
        let mut client = UefnMcpClient::connect(endpoint).unwrap();
        assert!(matches!(
            client.invoke_explicit_tool(
                "ActorTools",
                "delete_all_actors",
                json!({}),
                DeclaredEffect::EditorWrite
            ),
            Err(ClientError::ToolNotAdvertised)
        ));
        let requests = server.join().unwrap();
        assert!(
            !requests
                .iter()
                .any(|request| request.pointer("/params/name") == Some(&json!("call_tool")))
        );
    }

    #[test]
    fn ambiguous_dispatcher_shape_fails_closed() {
        let schema = json!({"type":"object", "properties":{
            "toolset_name":{"type":"string"}, "tool_name":{"type":"string"},
            "arguments":{"type":"object"}
        }, "required":["toolset_name","tool_name","arguments"]});
        assert!(matches!(
            dispatcher_arguments(Some(&schema), "ActorTools", "get_actors", json!({})),
            Err(ClientError::UnsupportedDispatcherSchema)
        ));
    }
}
