pub mod registry;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_MIN: u32 = 1;
pub const PROTOCOL_MAX: u32 = 1;
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegator_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub request_id: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_version: Option<u32>,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub context: RequestContext,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Producer {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    #[serde(rename = "type")]
    pub message_type: String,
    pub request_id: String,
    pub command_version: u32,
    pub schema_version: u32,
    pub producer: Producer,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorEnvelope>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub replayed: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}
impl CommandResponse {
    pub fn success(
        request: &CommandRequest,
        command_version: u32,
        producer: Producer,
        result: Value,
    ) -> Self {
        Self {
            message_type: "command_result".to_string(),
            request_id: request.request_id.clone(),
            command_version,
            schema_version: ENVELOPE_SCHEMA_VERSION,
            producer,
            ok: true,
            result: Some(result),
            error: None,
            replayed: false,
        }
    }

    pub fn failure(
        request: &CommandRequest,
        command_version: u32,
        producer: Producer,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message_type: "command_result".to_string(),
            request_id: request.request_id.clone(),
            command_version,
            schema_version: ENVELOPE_SCHEMA_VERSION,
            producer,
            ok: false,
            result: None,
            error: Some(ErrorEnvelope {
                code: code.into(),
                message: message.into(),
            }),
            replayed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloRequest {
    #[serde(rename = "type")]
    pub message_type: String,
    pub auth_token: String,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub client: Producer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResponse {
    #[serde(rename = "type")]
    pub message_type: String,
    pub protocol: u32,
    pub schema_version: u32,
    pub server: Producer,
    pub capabilities: Vec<String>,
}
pub fn negotiate_protocol(client_min: u32, client_max: u32) -> Option<u32> {
    let min = client_min.max(PROTOCOL_MIN);
    let max = client_max.min(PROTOCOL_MAX);
    (min <= max).then_some(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn protocol_negotiation_fails_closed() {
        assert_eq!(negotiate_protocol(1, 1), Some(1));
        assert_eq!(negotiate_protocol(2, 3), None);
    }

    #[test]
    fn response_envelope_is_versioned_and_structured() {
        let request = CommandRequest {
            request_id: "REQ-fixture".to_string(),
            command: "system.echo".to_string(),
            command_version: Some(1),
            arguments: json!({ "value": 1 }),
            idempotency_key: None,
            context: RequestContext::default(),
        };
        let response = CommandResponse::success(
            &request,
            1,
            Producer {
                name: "relay-core".to_string(),
                version: "0.1.0".to_string(),
            },
            json!({ "echo": request.arguments }),
        );
        assert!(response.ok);
        assert_eq!(response.schema_version, 1);
        assert_eq!(response.command_version, 1);
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcSecurityState {
    pub transport: String,
    pub explicit_dacl: bool,
    pub kernel_acl_verified: bool,
    pub owner_current_user: bool,
    pub acl_ace_count: u32,
    pub scope: String,
    pub auth_token: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalHostState {
    pub state_format: u32,
    pub pid: u32,
    pub pipe: String,
    pub auth_token: String,
    pub version: String,
    pub protocol: ProtocolRange,
    pub capabilities: Vec<String>,
    pub recovery_state: String,
    pub storage_schema_version: Option<i64>,
    pub ipc_security: IpcSecurityState,
    pub started_at_unix_ms: u64,
}

pub const LOCAL_HOST_STATE_FORMAT: u32 = 1;
