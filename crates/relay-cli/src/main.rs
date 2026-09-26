use relay::client;
use relay_contracts::{
    registry, CommandRequest, CommandResponse, RequestContext,
    LOCAL_HOST_STATE_FORMAT,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineInput {
    #[serde(default)]
    request_id: Option<String>,
    command: String,
    #[serde(default)]
    command_version: Option<u32>,
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    idempotency_key: Option<String>,
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("REQ-{}-{nanos}", std::process::id())
}

fn make_request(
    command: impl Into<String>,
    arguments: Value,
    idempotency_key: Option<String>,
) -> CommandRequest {
    CommandRequest {
        request_id: request_id(),
        command: command.into(),
        command_version: Some(1),
        arguments,
        idempotency_key,
        context: RequestContext::default(),
    }
}
fn load_state() -> Result<relay_contracts::LocalHostState, String> {
    let state = client::read_state()?;
    if state.state_format != LOCAL_HOST_STATE_FORMAT {
        return Err(format!(
            "HOST_STATE_INCOMPATIBLE: expected {}, got {}",
            LOCAL_HOST_STATE_FORMAT,
            state.state_format
        ));
    }
    Ok(state)
}

fn invoke(request: &CommandRequest) -> Result<CommandResponse, String> {
    let state = load_state()?;
    client::call(&state, request)
}

fn print_machine(response: &CommandResponse) {
    println!(
        "{}",
        serde_json::to_string(response)
            .unwrap_or_else(|_| "{\"ok\":false}".to_string())
    );
}

fn print_human(response: &CommandResponse) {
    if !response.ok {
        let error = response.error.as_ref();
        eprintln!(
            "{}: {}",
            error.map(|value| value.code.as_str())
                .unwrap_or("RELAY_ERROR"),
            error.map(|value| value.message.as_str())
                .unwrap_or("command failed")
        );
        return;
    }
    let result = response.result.as_ref().unwrap_or(&Value::Null);
    println!(
        "{}",
        serde_json::to_string_pretty(result)
            .unwrap_or_else(|_| "null".to_string())
    );
}
fn print_status(response: &CommandResponse) {
    if !response.ok {
        print_human(response);
        return;
    }
    let result = response.result.as_ref().unwrap();
    println!(
        "RELAY {}",
        result["recovery_state"].as_str().unwrap_or("Unknown")
    );
    println!("PID: {}", result["pid"]);
    println!(
        "Storage: {}",
        if result["storage"]["ok"].as_bool() == Some(true) {
            "healthy"
        } else {
            "degraded"
        }
    );
    println!(
        "Diagnostics: {}",
        if result["diagnostics"]["ok"].as_bool() == Some(true) {
            "healthy"
        } else {
            "degraded"
        }
    );
}

fn print_doctor(response: &CommandResponse) {
    if !response.ok {
        print_human(response);
        return;
    }
    let result = response.result.as_ref().unwrap();
    println!(
        "{}",
        result["summary"].as_str().unwrap_or("RELAY diagnostics")
    );
    if let Some(checks) = result["checks"].as_array() {
        for check in checks {
            println!(
                "- {}: {} — {}",
                check["id"].as_str().unwrap_or("check"),
                check["status"].as_str().unwrap_or("unknown"),
                check["detail"].as_str().unwrap_or("")
            );
        }
    }
}
fn machine_exec() -> Result<i32, String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("read stdin: {error}"))?;
    let input: MachineInput = serde_json::from_str(&input)
        .map_err(|error| format!("INVALID_MACHINE_INPUT: {error}"))?;

    let request = CommandRequest {
        request_id: input.request_id.unwrap_or_else(request_id),
        command: input.command,
        command_version: input.command_version,
        arguments: input.arguments,
        idempotency_key: input.idempotency_key,
        context: RequestContext::default(),
    };
    let response = invoke(&request)?;
    print_machine(&response);
    Ok(if response.ok { 0 } else { 2 })
}

fn has_json_flag(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

fn execute_human(
    request: CommandRequest,
    json_output: bool,
    view: &str,
) -> Result<i32, String> {
    let response = invoke(&request)?;
    if json_output {
        print_machine(&response);
    } else {
        match view {
            "status" => print_status(&response),
            "doctor" => print_doctor(&response),
            _ => print_human(&response),
        }
    }
    Ok(if response.ok { 0 } else { 2 })
}
fn run(args: &[String]) -> Result<i32, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(
            "usage: relay <status|doctor|commands|project-list|project-register|result-get|job-get|shutdown|exec>"
                .to_string(),
        );
    };

    match command {
        "commands" => {
            println!("{}", registry::render_cli_catalog());
            Ok(0)
        }
        "exec" if args.get(1).map(String::as_str) == Some("--stdin") => {
            machine_exec()
        }
        "status" => execute_human(
            make_request("system.status", json!({}), None),
            has_json_flag(args),
            "status",
        ),
        "doctor" => execute_human(
            make_request("system.doctor", json!({}), None),
            has_json_flag(args),
            "doctor",
        ),
        "project-list" => execute_human(
            make_request("project.list", json!({}), None),
            has_json_flag(args),
            "default",
        ),
        "result-get" => {
            let id = args.get(1)
                .ok_or_else(|| "result-get requires result ID".to_string())?;
            execute_human(
                make_request(
                    "result.get",
                    json!({ "result_id": id }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "job-get" => {
            let id = args.get(1)
                .ok_or_else(|| "job-get requires job ID".to_string())?;
            execute_human(
                make_request(
                    "job.get",
                    json!({ "job_id": id }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "project-register" => {
            let name = args.get(1)
                .ok_or_else(|| "project-register requires name".to_string())?;
            let root_uri = args.get(2)
                .ok_or_else(|| "project-register requires root URI".to_string())?;
            let explicit_id = args.get(3)
                .filter(|value| !value.starts_with("--"));
            let mut arguments = serde_json::Map::new();
            arguments.insert("name".to_string(), json!(name));
            arguments.insert("root_uri".to_string(), json!(root_uri));
            if let Some(id) = explicit_id {
                arguments.insert("id".to_string(), json!(id));
            }
            let req_id = request_id();
            let request = CommandRequest {
                request_id: req_id.clone(),
                command: "project.register".to_string(),
                command_version: Some(1),
                arguments: Value::Object(arguments),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            };
            execute_human(
                request,
                has_json_flag(args),
                "default",
            )
        }
        "shutdown" => execute_human(
            make_request("system.shutdown", json!({}), None),
            has_json_flag(args),
            "default",
        ),
        other => Err(format!("unknown relay command: {other}")),
    }
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let machine = args.first().map(String::as_str) == Some("exec");
            if machine {
                eprintln!(
                    "{}",
                    serde_json::to_string(&json!({
                        "ok": false,
                        "error": {
                            "code": "RELAY_CLI_ERROR",
                            "message": error
                        }
                    }))
                    .unwrap_or_else(|_| "{\"ok\":false}".to_string())
                );
            } else {
                eprintln!("{error}");
            }
            std::process::exit(1);
        }
    }
}
