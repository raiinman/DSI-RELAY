use relay::client;
use relay::onboarding;
use relay::parser_install::{self, InstallOptions};
use relay_contracts::{
    CommandRequest, CommandResponse, LOCAL_HOST_STATE_FORMAT, RequestContext, registry,
};
use serde::Deserialize;
use serde_json::{Value, json};
use relay_support::{SupportInput, ComponentVersion, DiagnosticHealthSummary, DiagnosticCounts};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
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
            LOCAL_HOST_STATE_FORMAT, state.state_format
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
        serde_json::to_string(response).unwrap_or_else(|_| "{\"ok\":false}".to_string())
    );
}

fn print_human(response: &CommandResponse) {
    if !response.ok {
        let error = response.error.as_ref();
        eprintln!(
            "{}: {}",
            error
                .map(|value| value.code.as_str())
                .unwrap_or("RELAY_ERROR"),
            error
                .map(|value| value.message.as_str())
                .unwrap_or("command failed")
        );
        return;
    }
    let result = response.result.as_ref().unwrap_or(&Value::Null);
    println!(
        "{}",
        serde_json::to_string_pretty(result).unwrap_or_else(|_| "null".to_string())
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
    let input: MachineInput =
        serde_json::from_str(&input).map_err(|error| format!("INVALID_MACHINE_INPUT: {error}"))?;

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

fn execute_human(request: CommandRequest, json_output: bool, view: &str) -> Result<i32, String> {
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
            "usage: relay <status|doctor|diagnostics|support-bundle|discover|onboard|dashboard-url|commands|project-list|project-register|uefn-inspect|uefn-audit|uefn-discover|verse-analyze|asset-validate|krita-inspect|parser-install|result-list|result-get|job-get|shutdown|exec>"
                .to_string(),
        );
    };

    match command {
        "support-bundle" => {
            if args.len() != 2 || args[1].starts_with("--") {
                return Err("usage: relay support-bundle <new-output.json>".to_string());
            }
            let status = invoke(&make_request("system.status", json!({}), None))?;
            let summary = invoke(&make_request("diagnostics.summary", json!({}), None))?;
            if !status.ok || !summary.ok {
                return Err("RELAY health summaries are unavailable".to_string());
            }
            let status = status.result.ok_or_else(|| "RELAY status is empty".to_string())?;
            let summary = summary.result.ok_or_else(|| "RELAY diagnostic summary is empty".to_string())?;
            let health = &status["diagnostics"];
            let events = &summary["events"];
            let safe_count = |value: &Value| value.as_u64().unwrap_or(0);
            let bundle = relay_support::bundle_json(SupportInput {
                generated_unix_ms: safe_count(&summary["generated_unix_ms"]),
                relay_version: status["version"].as_str().unwrap_or("unknown").to_string(),
                component_versions: vec![ComponentVersion {
                    component_id: "relay-core".to_string(),
                    version: summary["relay_version"].as_str().unwrap_or("unknown").to_string(),
                }],
                diagnostic_health: DiagnosticHealthSummary {
                    healthy: health["ok"].as_bool().unwrap_or(false),
                    current_bytes: safe_count(&health["current_bytes"]),
                    rotated_files: safe_count(&health["rotated_files"]) as usize,
                    evicted_events: safe_count(&health["evicted_events"]),
                    evicted_files: safe_count(&health["evicted_files"]),
                    recovered_partial_bytes: safe_count(&health["recovered_partial_bytes"]),
                    last_sync_unix_ms: health["last_sync_unix_ms"].as_u64(),
                    detail_active: health["detail_active"].as_bool().unwrap_or(false),
                    last_error_code: summary["error_code"].as_str().map(str::to_string),
                },
                diagnostic_counts: DiagnosticCounts {
                    total_events: safe_count(&events["total"]),
                    invalid_lines: safe_count(&events["invalid_lines"]),
                    incomplete_events: safe_count(&events["incomplete"]),
                    sampled_events: safe_count(&events["sampled"]),
                    untrusted_source_events: 0,
                    retention_evicted_events: safe_count(&health["evicted_events"]),
                    by_code: Vec::new(),
                },
                integrated_run: None,
            }).map_err(|error| format!("support bundle unavailable: {}", error.code))?;
            let mut output = fs::OpenOptions::new().write(true).create_new(true)
                .open(&args[1]).map_err(|_| "output file already exists or cannot be created".to_string())?;
            output.write_all(&bundle).map_err(|_| "support bundle could not be written".to_string())?;
            println!("Privacy-safe support bundle saved: {} bytes. Integrated run: not yet recorded.", bundle.len());
            Ok(0)
        }
        "discover" => {
            let roots: Vec<&String> = args
                .iter()
                .skip(1)
                .filter(|arg| arg.as_str() != "--json")
                .collect();
            if roots.len() > 1 || roots.first().is_some_and(|root| root.starts_with('-')) {
                return Err("usage: relay discover [root] [--json]".to_string());
            }
            let root = roots
                .first()
                .map(|root| PathBuf::from(root.as_str()))
                .unwrap_or(
                    std::env::current_dir()
                        .map_err(|error| format!("current directory unavailable: {error}"))?,
                );
            let report = onboarding::discover(&root)?;
            if has_json_flag(args) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|error| format!("serialize discovery: {error}"))?
                );
            } else {
                println!("{}", onboarding::render_human(&report));
            }
            Ok(0)
        }
        "onboard" => {
            let root = args.get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| "usage: relay onboard <project-folder> [--json]".to_string())?;
            if args.iter().skip(2).any(|value| value != "--json") {
                return Err("usage: relay onboard <project-folder> [--json]".to_string());
            }
            let root = PathBuf::from(root).canonicalize()
                .map_err(|_| "project folder is unavailable".to_string())?;
            if !root.is_dir() {
                return Err("project folder is not a directory".to_string());
            }
            let name = root.file_name().and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "project folder needs a readable name".to_string())?;
            let req_id = request_id();
            let imported = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "project.import".to_string(),
                command_version: Some(1),
                arguments: json!({ "name": name, "root_path": root.to_string_lossy() }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if !imported.ok {
                if has_json_flag(args) { print_machine(&imported); } else { print_human(&imported); }
                return Ok(2);
            }
            let imported_result = imported.result.as_ref()
                .ok_or_else(|| "project import returned no result".to_string())?;
            let project_id = imported_result["id"].as_str()
                .ok_or_else(|| "project import returned no ID".to_string())?.to_string();
            let req_id = request_id();
            let baseline = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "project.index.build".to_string(),
                command_version: Some(1),
                arguments: json!({ "project_id": project_id }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) {
                println!("{}", serde_json::to_string_pretty(&json!({
                    "project_id": project_id,
                    "import": imported,
                    "baseline": baseline
                })).map_err(|error| format!("serialize onboarding: {error}"))?);
            } else if baseline.ok {
                println!("Project {name} connected and indexed. ID: {project_id}");
            } else {
                println!("Project {name} connected as {project_id}, but its first index needs attention.");
                print_human(&baseline);
            }
            Ok(if baseline.ok { 0 } else { 2 })
        }
        "commands" => {
            println!("{}", registry::render_cli_catalog());
            Ok(0)
        }
        "dashboard-url" => {
            let state = load_state()?;
            let status = client::call(&state, &make_request("system.status", json!({}), None))?;
            if !status.ok {
                return Err("RELAY dashboard is unavailable".to_string());
            }
            let path = client::state_dir().join("dashboard.json");
            let bytes =
                fs::read(&path).map_err(|_| "RELAY dashboard is unavailable".to_string())?;
            let info: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "RELAY dashboard state is invalid".to_string())?;
            let url = info["url"]
                .as_str()
                .ok_or("RELAY dashboard state is invalid")?;
            if !url.starts_with("http://127.0.0.1:") || !url.contains("/#") {
                return Err("RELAY dashboard state is invalid".to_string());
            }
            println!("{url}");
            Ok(0)
        }
        "exec" if args.get(1).map(String::as_str) == Some("--stdin") => machine_exec(),
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
        "diagnostics" => execute_human(
            make_request("diagnostics.summary", json!({}), None),
            has_json_flag(args),
            "default",
        ),
        "project-list" => execute_human(
            make_request("project.list", json!({}), None),
            has_json_flag(args),
            "default",
        ),
        "uefn-inspect" => {
            let project_id = args
                .get(1)
                .ok_or_else(|| "uefn-inspect requires a project ID".to_string())?;
            execute_human(
                make_request(
                    "uefn.static.inspect",
                    json!({ "project_id": project_id }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "uefn-audit" => {
            let project_id = args.get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| "uefn-audit requires a project ID".to_string())?;
            let inspection = invoke(&make_request(
                "uefn.static.inspect", json!({ "project_id": project_id }), None,
            ))?;
            if !inspection.ok {
                if has_json_flag(args) { print_machine(&inspection); } else { print_human(&inspection); }
                return Ok(2);
            }
            let payload = inspection.result
                .ok_or_else(|| "UEFN inspection returned no result".to_string())?;
            let req_id = request_id();
            let recorded = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "result.put".to_string(),
                command_version: Some(1),
                arguments: json!({
                    "project_id": project_id,
                    "kind": "UEFN_STATIC_AUDIT",
                    "payload": payload
                }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) { print_machine(&recorded); } else { print_human(&recorded); }
            Ok(if recorded.ok { 0 } else { 2 })
        }
        "uefn-discover" => {
            let port = match args.get(1) {
                Some(value) if value != "--json" => value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or_else(|| "UEFN MCP port must be between 1 and 65535".to_string())?,
                _ => 8000,
            };
            execute_human(
                make_request("uefn.mcp.discover", json!({ "port": port }), None),
                has_json_flag(args),
                "default",
            )
        }
        "verse-analyze" => {
            if args.len() < 4 || args.len() > 6 {
                return Err("usage: relay verse-analyze <project-id> <session-id> <capture-file> [assertions.json] [--json]".to_string());
            }
            let capture_text = fs::read_to_string(&args[3])
                .map_err(|_| "Verse capture could not be read".to_string())?;
            if capture_text.len() > 32768 {
                return Err("Verse capture exceeds the local command size limit".to_string());
            }
            let assertions = if args.get(4).is_some_and(|arg| arg != "--json") {
                let bytes = fs::read(&args[4])
                    .map_err(|_| "assertion file could not be read".to_string())?;
                if bytes.len() > 8192 {
                    return Err("assertion file exceeds the local command size limit".to_string());
                }
                serde_json::from_slice::<Value>(&bytes)
                    .map_err(|_| "assertion file is not valid JSON".to_string())?
            } else {
                json!([])
            };
            execute_human(
                make_request(
                    "runtime.capture.analyze",
                    json!({
                        "project_id": args[1],
                        "session_id": args[2],
                        "source_kind": "imported_log",
                        "source_version": "unverified",
                        "capture_ref": format!("CAP-{}", request_id()),
                        "capture_text": capture_text,
                        "assertions": assertions
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "asset-validate" | "krita-inspect" => {
            let project_id = args.get(1).ok_or_else(|| {
                "asset-validate requires a project ID and manifest file".to_string()
            })?;
            let manifest_path = args.get(2).ok_or_else(|| {
                "asset-validate requires a project ID and manifest file".to_string()
            })?;
            let manifest_json = fs::read_to_string(manifest_path)
                .map_err(|_| "asset manifest could not be read".to_string())?;
            if manifest_json.len() > 32768 {
                return Err("asset manifest exceeds the local command size limit".to_string());
            }
            execute_human(
                make_request(
                    if command == "krita-inspect" { "assets.krita.inspect" } else { "assets.manifest.validate" },
                    json!({
                        "project_id": project_id,
                        "manifest_json": manifest_json
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "parser-install" => {
            if args.len() != 6 || args[5] != "--allow-source-delivery" {
                return Err("usage: relay parser-install <project-id> <manifest.json> <worker.exe> <extensions-comma-separated> --allow-source-delivery".into());
            }
            let message = parser_install::install(InstallOptions {
                project_id: args[1].clone(),
                manifest_path: PathBuf::from(&args[2]),
                worker_path: PathBuf::from(&args[3]),
                source_extensions: args[4].split(',').map(str::to_string).collect(),
                allow_source_delivery: true,
            })?;
            println!("{message}");
            Ok(0)
        }
        "result-list" => {
            let project_id = args.get(1).filter(|value| !value.starts_with("--"));
            let arguments = match project_id {
                Some(project_id) => json!({ "project_id": project_id, "limit": 20 }),
                None => json!({ "limit": 20 }),
            };
            execute_human(
                make_request("result.list", arguments, None),
                has_json_flag(args),
                "default",
            )
        }
        "result-get" => {
            let id = args
                .get(1)
                .ok_or_else(|| "result-get requires result ID".to_string())?;
            execute_human(
                make_request("result.get", json!({ "result_id": id }), None),
                has_json_flag(args),
                "default",
            )
        }
        "job-get" => {
            let id = args
                .get(1)
                .ok_or_else(|| "job-get requires job ID".to_string())?;
            execute_human(
                make_request("job.get", json!({ "job_id": id }), None),
                has_json_flag(args),
                "default",
            )
        }
        "project-register" => {
            let name = args
                .get(1)
                .ok_or_else(|| "project-register requires name".to_string())?;
            let root_uri = args
                .get(2)
                .ok_or_else(|| "project-register requires root URI".to_string())?;
            let explicit_id = args.get(3).filter(|value| !value.starts_with("--"));
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
            execute_human(request, has_json_flag(args), "default")
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
