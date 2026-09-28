//! Local MCP stdio transport for desktop clients. Only JSON-RPC goes to stdout.

use super::{Gateway, LocalDaemonTransport, MAX_BODY_BYTES, MAX_RESPONSE_BYTES, RelayTransport};
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::sync::Arc;

const CURRENT_VERSION: &str = "2025-11-25";
const SUPPORTED_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2024-11-05"];

/// Serve a single local MCP client until it closes stdin. The adapter never
/// writes startup messages, credentials, or diagnostics to stdout.
pub fn run_stdio<R: Read, W: Write>(input: R, output: W) -> io::Result<()> {
    serve(input, output, Arc::new(LocalDaemonTransport))
}

fn serve<R: Read, W: Write>(
    mut input: R,
    mut output: W,
    transport: Arc<dyn RelayTransport>,
) -> io::Result<()> {
    let gateway = Gateway::for_stdio(transport);
    let mut initialized = false;
    let mut ready = false;
    while let Some(line) = read_line(&mut input)? {
        let body = match line {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => value,
                Err(_) => {
                    write_response(&mut output, &rpc_error(Value::Null, -32700, "Parse error"))?;
                    continue;
                }
            },
            Err(()) => {
                write_response(
                    &mut output,
                    &rpc_error(Value::Null, -32600, "Message too large"),
                )?;
                continue;
            }
        };
        let Some(object) = body.as_object() else {
            write_response(
                &mut output,
                &rpc_error(Value::Null, -32600, "Invalid Request"),
            )?;
            continue;
        };
        let id = object.get("id").cloned();
        let valid_id = id.as_ref().is_some_and(|value| {
            value.is_number() || value.as_str().is_some_and(|value| value.len() <= 128)
        });
        let method = object.get("method").and_then(Value::as_str);
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
            write_response(
                &mut output,
                &rpc_error(Value::Null, -32600, "Invalid Request"),
            )?;
            continue;
        }
        let method = method.unwrap_or("");
        if method == "notifications/initialized" && id.is_none() {
            if initialized && object.get("params").is_none_or(Value::is_object) {
                ready = true;
            }
            continue;
        }
        if !valid_id {
            // Ignore unrelated client notifications, and reject malformed requests.
            if id.is_some() {
                write_response(
                    &mut output,
                    &rpc_error(Value::Null, -32600, "Invalid Request"),
                )?;
            }
            continue;
        }
        let id = id.unwrap_or(Value::Null);
        let response = match method {
            "initialize" if !initialized => {
                let params = object.get("params").and_then(Value::as_object);
                let requested = params
                    .and_then(|params| params.get("protocolVersion"))
                    .and_then(Value::as_str);
                let valid_client = params.is_some_and(|params| {
                    params.get("capabilities").is_some_and(Value::is_object)
                        && params.get("clientInfo").is_some_and(|info| {
                            info.get("name")
                                .and_then(Value::as_str)
                                .is_some_and(|value| !value.is_empty() && value.len() <= 128)
                                && info
                                    .get("version")
                                    .and_then(Value::as_str)
                                    .is_some_and(|value| !value.is_empty() && value.len() <= 64)
                        })
                });
                if !valid_client || !requested.is_some_and(valid_version) {
                    rpc_error(id, -32602, "Invalid params")
                } else {
                    initialized = true;
                    let version = requested
                        .filter(|version| SUPPORTED_VERSIONS.contains(version))
                        .unwrap_or(CURRENT_VERSION);
                    json!({"jsonrpc":"2.0","id":id,"result":{
                        "protocolVersion":version,
                        "capabilities":{"tools":{}},
                        "serverInfo":{"name":"dsi-relay-local","version":env!("CARGO_PKG_VERSION")},
                        "instructions":"Read-only local RELAY registry and project-scoped result metadata. Supply an explicit project ID for result tools."
                    }})
                }
            }
            "initialize" => rpc_error(id, -32600, "Already initialized"),
            _ if !ready => rpc_error(id, -32002, "Not initialized"),
            "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
            "tools/list" => {
                let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
                if !params.as_object().is_some_and(|params| {
                    params.is_empty()
                        || (params.len() == 1 && params.get("_meta").is_some_and(Value::is_object))
                }) {
                    rpc_error(id, -32602, "Invalid params")
                } else {
                    gateway.legacy_tool_request(id, method, params)
                }
            }
            "tools/call" => {
                let params = object.get("params").cloned().unwrap_or(Value::Null);
                if !params.as_object().is_some_and(|params| {
                    (params.len() == 2
                        || (params.len() == 3 && params.get("_meta").is_some_and(Value::is_object)))
                        && params.get("name").and_then(Value::as_str).is_some()
                        && params.get("arguments").is_some_and(Value::is_object)
                }) {
                    rpc_error(id, -32602, "Invalid params")
                } else {
                    gateway.legacy_tool_request(id, method, params)
                }
            }
            _ => rpc_error(id, -32601, "Method not found"),
        };
        write_response(&mut output, &response)?;
    }
    Ok(())
}

fn valid_version(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn rpc_error(id: Value, code: i32, message: &'static str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn write_response<W: Write>(output: &mut W, response: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(response).map_err(io::Error::other)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return write_response(
            output,
            &rpc_error(
                response.get("id").cloned().unwrap_or(Value::Null),
                -32603,
                "Response too large",
            ),
        );
    }
    output.write_all(&bytes)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn read_line<R: Read>(input: &mut R) -> io::Result<Option<Result<Vec<u8>, ()>>> {
    let mut data = Vec::new();
    let mut overflow = false;
    let mut saw_any = false;
    loop {
        let mut byte = [0u8; 1];
        let count = input.read(&mut byte)?;
        if count == 0 {
            return if saw_any { Ok(Some(Err(()))) } else { Ok(None) };
        }
        saw_any = true;
        if byte[0] == b'\n' {
            if data.last() == Some(&b'\r') {
                data.pop();
            }
            return Ok(Some(if overflow || data.is_empty() {
                Err(())
            } else {
                Ok(data)
            }));
        }
        if data.len() < MAX_BODY_BYTES {
            data.push(byte[0]);
        } else {
            overflow = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_contracts::{CommandRequest, CommandResponse, Producer};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(Arc<AtomicUsize>);
    impl RelayTransport for Fixture {
        fn call(&self, request: &CommandRequest) -> Result<CommandResponse, ()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            let result = match request.command.as_str() {
                "system.status" => {
                    json!({"version":"0.1.0","capabilities":["registry.list@1","registry.describe@1","result.list@1","result.describe@1","result.context@1"]})
                }
                "result.describe" => {
                    json!({"id":"RES-one","project_id":"PRJ-one","kind":"CHECK","schema_version":1,"producer_version":"0.1.0","payload_sha256":"0".repeat(64),"payload_bytes":32,"trust":"local","created_at":"2026-09-27T00:00:00Z"})
                }
                "result.context" => {
                    json!({"result_id":"RES-one","payload_sha256":"0".repeat(64),"payload_bytes":32,"source_trust":"local","full_result_command":"result.get","facts":[{"pointer":"/count","value":3}],"omitted_scalar_count":0,"truncated":false})
                }
                _ => json!({"registry_format":1,"commands":[]}),
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

    fn run(messages: &[Value]) -> (Vec<Value>, usize) {
        let input = messages
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let calls = Arc::new(AtomicUsize::new(0));
        let mut output = Vec::new();
        serve(
            input.as_bytes(),
            &mut output,
            Arc::new(Fixture(calls.clone())),
        )
        .unwrap();
        let responses = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (responses, calls.load(Ordering::Relaxed))
    }

    fn init(version: &str) -> Value {
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})
    }

    #[test]
    fn negotiates_legacy_versions_and_keeps_stdout_json_only() {
        for version in SUPPORTED_VERSIONS {
            let (responses, calls) = run(&[
                init(version),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            ]);
            assert_eq!(responses.len(), 2);
            assert_eq!(responses[0]["result"]["protocolVersion"], version);
            assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 5);
            assert!(responses[1]["result"].get("resultType").is_none());
            assert_eq!(calls, 1);
        }
        let (responses, _) = run(&[init("2030-01-01")]);
        assert_eq!(responses[0]["result"]["protocolVersion"], CURRENT_VERSION);
    }

    #[test]
    fn rejects_preinit_unknown_tools_and_oversized_frames() {
        let (responses, calls) = run(&[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}),
            init(CURRENT_VERSION),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run_shell","arguments":{}}}),
        ]);
        assert_eq!(responses[0]["error"]["code"], -32002);
        assert_eq!(responses[2]["error"]["code"], -32602);
        assert_eq!(calls, 0);
        let giant = format!("{}\n", "x".repeat(MAX_BODY_BYTES + 1));
        let mut output = Vec::new();
        serve(
            giant.as_bytes(),
            &mut output,
            Arc::new(Fixture(Arc::new(AtomicUsize::new(0)))),
        )
        .unwrap();
        let reply: Value = serde_json::from_slice(&output[..output.len() - 1]).unwrap();
        assert_eq!(reply["error"]["message"], "Message too large");
    }

    #[test]
    fn scoped_context_and_invalid_scope_do_not_return_payload() {
        let calls = [
            init(CURRENT_VERSION),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"relay_result_context","arguments":{"project_id":"PRJ-one","result_id":"RES-one","max_bytes":1024}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"relay_result_context","arguments":{"result_id":"RES-one","max_bytes":1024}}}),
        ];
        let (responses, count) = run(&calls);
        assert_eq!(
            responses[1]["result"]["structuredContent"]["facts"][0]["value"],
            3
        );
        assert!(responses[1].to_string().contains("/count"));
        assert!(!responses[1].to_string().contains("payload_json"));
        assert_eq!(responses[2]["error"]["code"], -32602);
        assert_eq!(count, 3);
    }

    #[test]
    fn cross_project_context_fails_closed() {
        let (responses, count) = run(&[
            init(CURRENT_VERSION),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"relay_result_context",
                "arguments":{"project_id":"PRJ-other","result_id":"RES-one","max_bytes":1024}
            }}),
        ]);
        assert!(responses[1].get("error").is_some());
        assert!(!responses[1].to_string().contains("/count"));
        assert_eq!(count, 2); // status and ownership check, no context fetch
    }

    #[test]
    fn read_only_registry_call_has_legacy_text_fallback() {
        let (responses, count) = run(&[
            init(CURRENT_VERSION),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"relay_registry_list","arguments":{"prefix":"project.","limit":2},"_meta":{}
            }}),
        ]);
        let result = &responses[1]["result"];
        assert_eq!(result["structuredContent"]["registry_format"], 1);
        let fallback: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(fallback, result["structuredContent"]);
        assert_eq!(count, 2);
    }

    #[test]
    fn malformed_handshake_does_not_unlock_tools() {
        let (responses, calls) = run(&[
            init("2025-99"),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ]);
        assert_eq!(responses[0]["error"]["code"], -32602);
        assert_eq!(responses[1]["error"]["code"], -32002);
        assert_eq!(calls, 0);
    }
}
