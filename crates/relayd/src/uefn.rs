use relay_contracts::CommandRequest;
use relay_core::service::{ExtensionError, RelayCore};
use relay_uefn::StaticProjectInput;
use serde_json::Value;

/// Host integration invoked only after Core command validation and authority checks.
pub fn execute(
    core: &RelayCore,
    request: &CommandRequest,
) -> Option<Result<Value, ExtensionError>> {
    if request.command == "uefn.mcp.describe_toolset" {
        let port = request.arguments.get("port").and_then(Value::as_u64).unwrap_or(8000) as u16;
        let toolset_name = request.arguments["toolset_name"].as_str().expect("registry validates name");
        let endpoint = match relay_uefn_mcp::LocalEndpoint::new(port, "/mcp") {
            Ok(endpoint) => endpoint,
            Err(_) => return Some(Err(ExtensionError::new("UEFN_ENDPOINT_INVALID", "UEFN local endpoint is invalid"))),
        };
        let mut client = match relay_uefn_mcp::UefnMcpClient::connect(endpoint) {
            Ok(client) => client,
            Err(error) => return Some(Ok(toolset_description_unavailable(toolset_name, &error))),
        };
        return Some(Ok(match client.describe_toolset_summary(toolset_name) {
            Ok(summary) => serde_json::json!({
                "state": "discovered",
                "reason_code": serde_json::Value::Null,
                "toolset_name": summary.name,
                "tools": summary.tools.iter().map(|tool| serde_json::json!({
                    "name": tool.name,
                    "parameters": tool.parameters.iter().map(|parameter| serde_json::json!({
                        "name": parameter.name,
                        "kind": parameter.kind,
                        "required": parameter.required
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>(),
                "editor_identity_verified": false,
                "live_workflow_status": "untested"
            }),
            Err(error) => toolset_description_unavailable(toolset_name, &error),
        }));
    }
    if request.command == "uefn.mcp.toolsets" {
        let port = request.arguments.get("port").and_then(Value::as_u64).unwrap_or(8000) as u16;
        let endpoint = match relay_uefn_mcp::LocalEndpoint::new(port, "/mcp") {
            Ok(endpoint) => endpoint,
            Err(_) => return Some(Err(ExtensionError::new("UEFN_ENDPOINT_INVALID", "UEFN local endpoint is invalid"))),
        };
        let mut client = match relay_uefn_mcp::UefnMcpClient::connect(endpoint) {
            Ok(client) => client,
            Err(error) => return Some(Ok(toolsets_unavailable(&error))),
        };
        return Some(Ok(match client.list_toolset_summaries() {
            Ok(summary) => serde_json::json!({
                "state": "discovered",
                "reason_code": serde_json::Value::Null,
                "toolset_count": summary.names.len(),
                "toolset_names": summary.names,
                "editor_identity_verified": false,
                "live_workflow_status": "untested"
            }),
            Err(error) => toolsets_unavailable(&error),
        }));
    }
    if request.command == "uefn.mcp.discover" {
        let port = request
            .arguments
            .get("port")
            .and_then(Value::as_u64)
            .unwrap_or(8000) as u16;
        let endpoint = match relay_uefn_mcp::LocalEndpoint::new(port, "/mcp") {
            Ok(endpoint) => endpoint,
            Err(_) => {
                return Some(Err(ExtensionError::new(
                    "UEFN_ENDPOINT_INVALID",
                    "UEFN local endpoint is invalid",
                )));
            }
        };
        let mut client = match relay_uefn_mcp::UefnMcpClient::connect(endpoint) {
            Ok(client) => client,
            Err(error) => return Some(Ok(discovery_unavailable(&error))),
        };
        let protocol_version = client.protocol_version().to_string();
        return Some(Ok(match client.list_tools() {
            Ok(catalog) => serde_json::json!({
                "state": "discovered",
                "reason_code": serde_json::Value::Null,
                "protocol_version": protocol_version,
                "tool_count": catalog.names.len(),
                "discovery_tools_advertised": catalog.has("list_toolsets") && catalog.has("describe_toolset"),
                "editor_identity_verified": false,
                "live_workflow_status": "untested"
            }),
            Err(error) => discovery_unavailable(&error),
        }));
    }
    if request.command != "uefn.static.inspect" {
        return None;
    }
    let project_id = request.arguments["project_id"]
        .as_str()
        .expect("shared registry validates project_id");
    Some(core.ready_index_snapshot(project_id).and_then(|snapshot| {
        let inspection = relay_uefn::inspect(StaticProjectInput {
            project_id: project_id.to_string(),
            index_generation: snapshot.generation,
            relative_paths: snapshot.relative_paths,
        });
        serde_json::to_value(inspection).map_err(|_| {
            ExtensionError::new(
                "UEFN_SERIALIZATION_FAILED",
                "UEFN inspection result is unavailable",
            )
        })
    }))
}

fn toolset_description_unavailable(name: &str, error: &relay_uefn_mcp::ClientError) -> Value {
    serde_json::json!({
        "state": "unavailable",
        "reason_code": discovery_unavailable(error)["reason_code"],
        "toolset_name": name,
        "tools": [],
        "editor_identity_verified": false,
        "live_workflow_status": "untested"
    })
}

fn toolsets_unavailable(error: &relay_uefn_mcp::ClientError) -> Value {
    serde_json::json!({
        "state": "unavailable",
        "reason_code": discovery_unavailable(error)["reason_code"],
        "toolset_names": [],
        "toolset_count": 0,
        "editor_identity_verified": false,
        "live_workflow_status": "untested"
    })
}

fn discovery_unavailable(error: &relay_uefn_mcp::ClientError) -> Value {
    let reason_code = match error {
        relay_uefn_mcp::ClientError::UnsupportedProtocol => "UEFN_MCP_PROTOCOL_UNSUPPORTED",
        relay_uefn_mcp::ClientError::Transport | relay_uefn_mcp::ClientError::HttpStatus(_) => {
            "UEFN_MCP_UNAVAILABLE"
        }
        relay_uefn_mcp::ClientError::ToolsUnavailable => "UEFN_MCP_TOOLS_UNAVAILABLE",
        relay_uefn_mcp::ClientError::ToolLimit => "UEFN_MCP_TOOL_LIMIT",
        _ => "UEFN_MCP_DISCOVERY_FAILED",
    };
    serde_json::json!({
        "state": "unavailable",
        "reason_code": reason_code,
        "protocol_version": serde_json::Value::Null,
        "tool_count": serde_json::Value::Null,
        "discovery_tools_advertised": false,
        "editor_identity_verified": false,
        "live_workflow_status": "untested"
    })
}
