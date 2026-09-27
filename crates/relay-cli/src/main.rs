use relay::client;
use relay::onboarding;
use relay::parser_install::{self, InstallOptions};
mod launch;
use relay_contracts::{
    CommandRequest, CommandResponse, LOCAL_HOST_STATE_FORMAT, RequestContext, registry,
};
use relay_support::{ComponentVersion, DiagnosticCounts, DiagnosticHealthSummary, SupportInput};
use serde::Deserialize;
use serde_json::{Value, json};
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
    println!(
        "Background automation: {}",
        result["automation_mode"].as_str().unwrap_or("unavailable")
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

fn onboard_step(request: CommandRequest, step: &str) -> Result<Value, Value> {
    match invoke(&request) {
        Ok(response) if response.ok => response
            .result
            .ok_or_else(|| json!({"step": step, "code": "COMMAND_RESULT_MISSING"})),
        Ok(response) => Err(json!({
            "step": step,
            "code": response.error.as_ref().map(|error| error.code.as_str()).unwrap_or("COMMAND_FAILED")
        })),
        Err(_) => Err(json!({"step": step, "code": "TRANSPORT_UNAVAILABLE"})),
    }
}

fn print_onboard_report(report: &Value, machine: bool) -> Result<(), String> {
    if machine {
        println!(
            "{}",
            serde_json::to_string(report)
                .map_err(|error| format!("serialize onboarding: {error}"))?
        );
        return Ok(());
    }
    let id = report["project_id"].as_str().unwrap_or("unknown");
    println!("Project connected. ID: {id}");
    if report["status"] != "complete" {
        println!(
            "First audit incomplete at {} ({}). The project remains connected.",
            report["failure"]["step"].as_str().unwrap_or("unknown step"),
            report["failure"]["code"]
                .as_str()
                .unwrap_or("command failed")
        );
        return Ok(());
    }
    println!(
        "First index: {} files, {} bytes (generation {}).",
        report["index"]["file_count"],
        report["index"]["total_bytes"],
        report["index"]["generation"]
    );
    let audit = &report["first_audit"];
    println!(
        "Index: {}. Available capabilities: {}.",
        audit["index_status"].as_str().unwrap_or("unknown"),
        audit["available_capabilities"]
            .as_array()
            .map_or(0, Vec::len)
    );
    if let Some(gaps) = audit["capability_gaps"].as_array() {
        for gap in gaps {
            println!(
                "- Capability gap: {} ({}) — {}",
                gap["capability"].as_str().unwrap_or("unknown"),
                gap["state"].as_str().unwrap_or("unknown"),
                gap["detail"].as_str().unwrap_or("needs inspection")
            );
        }
    }
    if let Some(uefn) = audit["uefn_static"].as_object() {
        println!(
            "UEFN marker: {}. Indexed Verse sources: {}. Editor and runtime workflows: untested.",
            uefn["marker_state"].as_str().unwrap_or("unknown"),
            uefn["verse_source_count"]
        );
    } else {
        println!("No UEFN project marker appears in the index.");
    }
    println!("This first audit uses indexed metadata; it does not validate creator-app workflows.");
    Ok(())
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
            "usage: relay <launch|status|doctor|diagnostics|pause|resume|support-bundle|discover|onboard|dashboard-url|commands|project-list|project-register|project-archive|project-restore|project-removal-plan|project-removal-get|project-removal-list|project-removal-approve|project-removal-reject|project-remove|check-catalog-put|check-catalog-get|check-add-file|plan-checks|run-checks|context-compile|task-context|uefn-inspect|uefn-audit|uefn-discover|uefn-toolsets|uefn-describe|verse-analyze|verse-record|verse-file-analyze|verse-file-record|asset-validate|asset-impact|blender-mesh-check|blender-mesh-record|krita-inspect|krita-export|krita-reconcile|parser-install|result-list|result-get|job-list|job-get|shutdown|exec>"
                .to_string(),
        );
    };

    match command {
        "launch" => {
            if args.len() != 1 {
                return Err("usage: relay launch".to_string());
            }
            launch::launch()?;
            Ok(0)
        }
        "support-bundle" => {
            if !matches!(args.len(), 2 | 4)
                || args[1].starts_with("--")
                || (args.len() == 4
                    && (args[2] != "--integrated-report" || args[3].starts_with("--")))
            {
                return Err("usage: relay support-bundle <new-output.json> [--integrated-report <report.json>]".to_string());
            }
            let integrated_run = if args.len() == 4 {
                let metadata = fs::symlink_metadata(&args[3])
                    .map_err(|_| "integrated report is unavailable".to_string())?;
                if !metadata.is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.len() > relay_support::MAX_INTEGRATED_REPORT_BYTES as u64
                {
                    return Err("integrated report is invalid or too large".to_string());
                }
                let mut report = Vec::new();
                fs::File::open(&args[3])
                    .map_err(|_| "integrated report could not be read".to_string())?
                    .take((relay_support::MAX_INTEGRATED_REPORT_BYTES + 1) as u64)
                    .read_to_end(&mut report)
                    .map_err(|_| "integrated report could not be read".to_string())?;
                Some(
                    relay_support::summarize_integrated_report(&report)
                        .map_err(|error| format!("integrated report rejected: {}", error.code))?,
                )
            } else {
                None
            };
            let status = invoke(&make_request("system.status", json!({}), None))?;
            let summary = invoke(&make_request("diagnostics.summary", json!({}), None))?;
            if !status.ok || !summary.ok {
                return Err("RELAY health summaries are unavailable".to_string());
            }
            let status = status
                .result
                .ok_or_else(|| "RELAY status is empty".to_string())?;
            let summary = summary
                .result
                .ok_or_else(|| "RELAY diagnostic summary is empty".to_string())?;
            let health = &status["diagnostics"];
            let events = &summary["events"];
            let safe_count = |value: &Value| value.as_u64().unwrap_or(0);
            let bundle = relay_support::bundle_json(SupportInput {
                generated_unix_ms: safe_count(&summary["generated_unix_ms"]),
                relay_version: status["version"].as_str().unwrap_or("unknown").to_string(),
                component_versions: vec![ComponentVersion {
                    component_id: "relay-core".to_string(),
                    version: summary["relay_version"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string(),
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
                integrated_run,
            })
            .map_err(|error| format!("support bundle unavailable: {}", error.code))?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args[1])
                .map_err(|_| "output file already exists or cannot be created".to_string())?;
            output
                .write_all(&bundle)
                .map_err(|_| "support bundle could not be written".to_string())?;
            println!("Privacy-safe support bundle saved: {} bytes.", bundle.len());
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
            let root = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| "usage: relay onboard <project-folder> [--json]".to_string())?;
            if args.iter().skip(2).any(|value| value != "--json") {
                return Err("usage: relay onboard <project-folder> [--json]".to_string());
            }
            let root = PathBuf::from(root)
                .canonicalize()
                .map_err(|_| "project folder is unavailable".to_string())?;
            if !root.is_dir() {
                return Err("project folder is not a directory".to_string());
            }
            let name = root
                .file_name()
                .and_then(|value| value.to_str())
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
                if has_json_flag(args) {
                    print_machine(&imported);
                } else {
                    print_human(&imported);
                }
                return Ok(2);
            }
            let imported_result = imported
                .result
                .as_ref()
                .ok_or_else(|| "project import returned no result".to_string())?;
            let project_id = imported_result["id"]
                .as_str()
                .ok_or_else(|| "project import returned no ID".to_string())?
                .to_string();
            let mut report = json!({
                "format_version": 1,
                "project_id": project_id,
                "status": "partial",
                "index": null,
                "first_audit": null,
                "failure": null
            });
            let req_id = request_id();
            let baseline = onboard_step(
                CommandRequest {
                    request_id: req_id.clone(),
                    command: "project.index.build".to_string(),
                    command_version: Some(1),
                    arguments: json!({ "project_id": project_id }),
                    idempotency_key: Some(format!("CLI-{req_id}")),
                    context: RequestContext::default(),
                },
                "baseline",
            );
            let baseline = match baseline {
                Ok(value) => value,
                Err(failure) => {
                    report["failure"] = failure;
                    print_onboard_report(&report, has_json_flag(args))?;
                    return Ok(2);
                }
            };
            report["index"] = json!({
                "generation": baseline["generation"],
                "file_count": baseline["file_count"],
                "total_bytes": baseline["total_bytes"],
                "symlinks_skipped": baseline["symlinks_skipped"],
                "elapsed_ms": baseline["elapsed_ms"]
            });
            let capabilities = match onboard_step(
                make_request(
                    "project.capabilities",
                    json!({"project_id": project_id}),
                    None,
                ),
                "capabilities",
            ) {
                Ok(value) => value,
                Err(failure) => {
                    report["failure"] = failure;
                    print_onboard_report(&report, has_json_flag(args))?;
                    return Ok(2);
                }
            };
            let inspection = match onboard_step(
                make_request(
                    "uefn.static.inspect",
                    json!({"project_id": project_id}),
                    None,
                ),
                "static_inspection",
            ) {
                Ok(value) => value,
                Err(failure) => {
                    report["failure"] = failure;
                    print_onboard_report(&report, has_json_flag(args))?;
                    return Ok(2);
                }
            };
            if !onboarding::audit_responses_match_project(
                &project_id,
                &baseline,
                &capabilities,
                &inspection,
            ) {
                report["failure"] = json!({
                    "step": "first_audit",
                    "code": "AUDIT_RESPONSE_MISMATCH"
                });
                print_onboard_report(&report, has_json_flag(args))?;
                return Ok(2);
            }
            report["first_audit"] = onboarding::first_audit(&capabilities, &inspection);
            report["status"] = json!("complete");
            print_onboard_report(&report, has_json_flag(args))?;
            Ok(0)
        }
        "commands" => {
            println!("{}", registry::render_cli_catalog());
            Ok(0)
        }
        "dashboard-url" => {
            let url = launch::live_dashboard_url()?.ok_or("RELAY dashboard is unavailable")?;
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
        "pause" | "resume" => {
            if args.iter().skip(1).any(|arg| arg != "--json") {
                return Err(format!("usage: relay {command} [--json]"));
            }
            execute_human(
                make_request(format!("automation.{command}"), json!({}), None),
                has_json_flag(args),
                "default",
            )
        }
        "project-list" => {
            if args
                .iter()
                .skip(1)
                .any(|arg| arg != "--json" && arg != "--all")
            {
                return Err("usage: relay project-list [--all] [--json]".to_string());
            }
            execute_human(
                make_request(
                    "project.list",
                    json!({ "include_inactive": args.iter().any(|arg| arg == "--all") }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "check-catalog-put" => {
            if !matches!(args.len(), 4 | 5)
                || args[1..4].iter().any(|arg| arg.starts_with("--"))
                || (args.len() == 5 && args[4] != "--json")
            {
                return Err("usage: relay check-catalog-put <project-id> <expected-revision> <catalog.json> [--json]".to_string());
            }
            let expected_revision = args[2]
                .parse::<i64>()
                .map_err(|_| "expected revision must be a nonnegative number".to_string())?;
            if expected_revision < 0 {
                return Err("expected revision must be a nonnegative number".to_string());
            }
            let metadata = fs::symlink_metadata(&args[3])
                .map_err(|_| "check catalog file is unavailable".to_string())?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > 128 * 1024
            {
                return Err("check catalog file is invalid or too large".to_string());
            }
            let mut catalog_bytes = Vec::new();
            fs::File::open(&args[3])
                .map_err(|_| "check catalog file could not be read".to_string())?
                .take(128 * 1024 + 1)
                .read_to_end(&mut catalog_bytes)
                .map_err(|_| "check catalog file could not be read".to_string())?;
            if catalog_bytes.len() > 128 * 1024 {
                return Err("check catalog file is too large".to_string());
            }
            let catalog: Value = serde_json::from_slice(&catalog_bytes)
                .map_err(|_| "check catalog is invalid JSON".to_string())?;
            let req_id = request_id();
            execute_human(
                CommandRequest {
                    request_id: req_id.clone(),
                    command: "project.check_catalog.put".to_string(),
                    command_version: Some(1),
                    arguments: json!({"project_id": args[1], "expected_revision": expected_revision, "catalog": catalog}),
                    idempotency_key: Some(format!("CLI-{req_id}")),
                    context: RequestContext::default(),
                },
                has_json_flag(args),
                "default",
            )
        }
        "check-catalog-get" => {
            if !matches!(args.len(), 2 | 3)
                || args[1].starts_with("--")
                || (args.len() == 3 && args[2] != "--json")
            {
                return Err("usage: relay check-catalog-get <project-id> [--json]".to_string());
            }
            execute_human(
                make_request(
                    "project.check_catalog.get",
                    json!({"project_id": args[1]}),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "check-add-file" => {
            if !matches!(args.len(), 4 | 5)
                || args[1..4].iter().any(|arg| arg.starts_with("--"))
                || (args.len() == 5 && args[4] != "--json")
            {
                return Err("usage: relay check-add-file <project-id> <check-id> <project-relative-path> [--json]".to_string());
            }
            let read = invoke(&make_request(
                "project.check_catalog.get",
                json!({"project_id": args[1]}),
                None,
            ))?;
            let (revision, mut catalog) = if read.ok {
                let record = read.result.as_ref().ok_or_else(|| "check catalog response is incomplete".to_string())?;
                let revision = record["revision"].as_i64().ok_or_else(|| "check catalog revision is invalid".to_string())?;
                let catalog = record.get("catalog").cloned().ok_or_else(|| "check catalog response is incomplete".to_string())?;
                (revision, catalog)
            } else if read.error.as_ref().is_some_and(|error| error.code == "CHECK_CATALOG_MISSING") {
                (0, json!({"format_version": 1, "checks": []}))
            } else {
                if has_json_flag(args) { print_machine(&read); } else { print_human(&read); }
                return Ok(2);
            };
            let checks = catalog.get_mut("checks").and_then(Value::as_array_mut)
                .ok_or_else(|| "check catalog response is invalid".to_string())?;
            if checks.iter().any(|check| check["id"] == args[2]) {
                return Err("check ID already exists in this project".to_string());
            }
            checks.push(json!({
                "id": args[2], "roots": [args[3]], "leaves": [],
                "dependency_mode": "direct",
                "assertion": {"kind": "indexed_file_present", "path": args[3]}
            }));
            let req_id = request_id();
            let saved = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "project.check_catalog.put".to_string(),
                command_version: Some(1),
                arguments: json!({"project_id": args[1], "expected_revision": revision, "catalog": catalog}),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) { print_machine(&saved); } else { print_human(&saved); }
            Ok(if saved.ok { 0 } else { 2 })
        }
        "plan-checks" => {
            if !matches!(args.len(), 3 | 4)
                || args[1..3].iter().any(|arg| arg.starts_with("--"))
                || (args.len() == 4 && args[3] != "--json")
            {
                return Err(
                    "usage: relay plan-checks <project-id> <after-generation> [--json]".to_string(),
                );
            }
            let after_generation = args[2]
                .parse::<i64>()
                .map_err(|_| "after-generation must be a nonnegative number".to_string())?;
            if after_generation < 0 {
                return Err("after-generation must be a nonnegative number".to_string());
            }
            execute_human(
                make_request(
                    "automation.checks.plan",
                    json!({"project_id": args[1], "after_generation": after_generation}),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "run-checks" => {
            if !matches!(args.len(), 4 | 5)
                || args[1..4].iter().any(|arg| arg.starts_with("--"))
                || (args.len() == 5 && args[4] != "--json")
            {
                return Err(
                    "usage: relay run-checks <project-id> <after-generation> <plan-id> [--json]"
                        .to_string(),
                );
            }
            let after_generation = args[2]
                .parse::<i64>()
                .map_err(|_| "after-generation must be a nonnegative number".to_string())?;
            if after_generation < 0 {
                return Err("after-generation must be a nonnegative number".to_string());
            }
            let req_id = request_id();
            execute_human(
                CommandRequest {
                    request_id: req_id.clone(),
                    command: "automation.checks.execute".to_string(),
                    command_version: Some(1),
                    arguments: json!({"project_id": args[1], "after_generation": after_generation, "plan_id": args[3]}),
                    idempotency_key: Some(format!("CLI-{req_id}")),
                    context: RequestContext::default(),
                },
                has_json_flag(args),
                "default",
            )
        }
        "project-archive" | "project-restore" => {
            let project_id = args
                .get(1)
                .filter(|arg| !arg.starts_with("--"))
                .ok_or_else(|| format!("usage: relay {command} <project-id> [--json]"))?;
            if args.iter().skip(2).any(|arg| arg != "--json") {
                return Err(format!("usage: relay {command} <project-id> [--json]"));
            }
            let operation = command.strip_prefix("project-").unwrap();
            let arguments = json!({ "project_id": project_id });
            let req_id = request_id();
            execute_human(
                CommandRequest {
                    request_id: req_id.clone(),
                    command: format!("project.{operation}"),
                    command_version: Some(1),
                    arguments,
                    idempotency_key: Some(format!("CLI-{req_id}")),
                    context: RequestContext::default(),
                },
                has_json_flag(args),
                "default",
            )
        }
        "project-removal-plan" | "project-removal-list" => {
            if !matches!(args.len(), 2 | 3)
                || args[1].starts_with("--")
                || (args.len() == 3 && args[2] != "--json")
            {
                return Err(format!("usage: relay {command} <project-id> [--json]"));
            }
            let operation = if command.ends_with("plan") {
                "plan"
            } else {
                "list"
            };
            let mut request = make_request(
                format!("project.removal.{operation}"),
                json!({"project_id": args[1]}),
                None,
            );
            if operation == "plan" {
                request.idempotency_key = Some(format!("CLI-{}", request.request_id));
            }
            execute_human(request, has_json_flag(args), "default")
        }
        "project-removal-get"
        | "project-removal-approve"
        | "project-removal-reject"
        | "project-remove" => {
            if !matches!(args.len(), 3 | 4)
                || args[1..3].iter().any(|arg| arg.starts_with("--"))
                || (args.len() == 4 && args[3] != "--json")
            {
                return Err(format!(
                    "usage: relay {command} <project-id> <approval-id> [--json]"
                ));
            }
            let (operation, arguments) = match command {
                "project-removal-get" => (
                    "project.removal.get",
                    json!({"project_id": args[1], "approval_id": args[2]}),
                ),
                "project-removal-approve" => (
                    "project.removal.decide",
                    json!({"project_id": args[1], "approval_id": args[2], "decision": "approve"}),
                ),
                "project-removal-reject" => (
                    "project.removal.decide",
                    json!({"project_id": args[1], "approval_id": args[2], "decision": "reject"}),
                ),
                _ => (
                    "project.remove",
                    json!({"project_id": args[1], "approval_id": args[2]}),
                ),
            };
            let mut request = make_request(operation, arguments, None);
            if operation == "project.remove" {
                request.command_version = Some(2);
            }
            if operation != "project.removal.get" {
                request.idempotency_key = Some(format!("CLI-{}", request.request_id));
            }
            execute_human(request, has_json_flag(args), "default")
        }
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
            let project_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| "uefn-audit requires a project ID".to_string())?;
            let inspection = invoke(&make_request(
                "uefn.static.inspect",
                json!({ "project_id": project_id }),
                None,
            ))?;
            if !inspection.ok {
                if has_json_flag(args) {
                    print_machine(&inspection);
                } else {
                    print_human(&inspection);
                }
                return Ok(2);
            }
            let payload = inspection
                .result
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
            if has_json_flag(args) {
                print_machine(&recorded);
            } else {
                print_human(&recorded);
            }
            Ok(if recorded.ok { 0 } else { 2 })
        }
        "uefn-describe" => {
            let name = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| {
                    "usage: relay uefn-describe <toolset-name> [port] [--json]".to_string()
                })?;
            let port = match args.get(2) {
                Some(value) if value != "--json" => value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or_else(|| "UEFN MCP port must be between 1 and 65535".to_string())?,
                _ => 8000,
            };
            execute_human(
                make_request(
                    "uefn.mcp.describe_toolset",
                    json!({
                        "toolset_name": name, "port": port
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "uefn-discover" | "uefn-toolsets" => {
            let port = match args.get(1) {
                Some(value) if value != "--json" => value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or_else(|| "UEFN MCP port must be between 1 and 65535".to_string())?,
                _ => 8000,
            };
            execute_human(
                make_request(
                    if command == "uefn-toolsets" {
                        "uefn.mcp.toolsets"
                    } else {
                        "uefn.mcp.discover"
                    },
                    json!({ "port": port }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "verse-analyze" | "verse-record" => {
            if args.len() < 4 || args.len() > 6 {
                return Err("usage: relay <verse-analyze|verse-record> <project-id> <session-id> <capture-file> [assertions.json] [--json]".to_string());
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
            let analysis = invoke(&make_request(
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
            ))?;
            if command == "verse-analyze" || !analysis.ok {
                if has_json_flag(args) {
                    print_machine(&analysis);
                } else {
                    print_human(&analysis);
                }
                return Ok(if analysis.ok { 0 } else { 2 });
            }
            let payload = analysis
                .result
                .ok_or_else(|| "Verse analysis returned no result".to_string())?;
            let req_id = request_id();
            let recorded = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "result.put".to_string(),
                command_version: Some(1),
                arguments: json!({
                    "project_id": args[1],
                    "kind": "IMPORTED_VERSE_CAPTURE_ANALYSIS",
                    "payload": payload
                }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) {
                print_machine(&recorded);
            } else {
                print_human(&recorded);
            }
            Ok(if recorded.ok { 0 } else { 2 })
        }
        "verse-file-analyze" | "verse-file-record" => {
            if args.len() < 4 || args.len() > 6 {
                return Err("usage: relay <verse-file-analyze|verse-file-record> <project-id> <session-id> <project-relative-log-path> [assertions.json] [--json]".to_string());
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
            let analysis = invoke(&make_request(
                "runtime.capture.file.analyze",
                json!({
                    "project_id": args[1], "session_id": args[2],
                    "relative_path": args[3], "assertions": assertions
                }),
                None,
            ))?;
            if command == "verse-file-analyze" || !analysis.ok {
                if has_json_flag(args) {
                    print_machine(&analysis);
                } else {
                    print_human(&analysis);
                }
                return Ok(if analysis.ok { 0 } else { 2 });
            }
            let payload = analysis
                .result
                .ok_or_else(|| "Verse file analysis returned no result".to_string())?;
            let req_id = request_id();
            let recorded = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "result.put".to_string(),
                command_version: Some(1),
                arguments: json!({
                    "project_id": args[1],
                    "kind": "PROJECT_FILE_VERSE_CAPTURE_ANALYSIS",
                    "payload": payload
                }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) {
                print_machine(&recorded);
            } else {
                print_human(&recorded);
            }
            Ok(if recorded.ok { 0 } else { 2 })
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
                    if command == "krita-inspect" {
                        "assets.krita.inspect"
                    } else {
                        "assets.manifest.validate"
                    },
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
        "asset-impact" => {
            let usage = "usage: relay asset-impact <project-id> <manifest-file> <changed-project-relative-path>... [--json]";
            let project_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let manifest_path = args
                .get(2)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let changed_paths: Vec<&str> = args
                .iter()
                .skip(3)
                .filter(|value| value.as_str() != "--json")
                .map(String::as_str)
                .collect();
            if changed_paths.is_empty()
                || changed_paths.len() > 256
                || changed_paths.iter().any(|value| value.starts_with("--"))
            {
                return Err(usage.to_string());
            }
            let manifest_json = fs::read_to_string(manifest_path)
                .map_err(|_| "asset manifest could not be read".to_string())?;
            if manifest_json.len() > 32768 {
                return Err("asset manifest exceeds the local command size limit".to_string());
            }
            execute_human(
                make_request(
                    "assets.impact.analyze",
                    json!({
                        "project_id": project_id,
                        "manifest_json": manifest_json,
                        "changed_paths": changed_paths
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "blender-mesh-check" | "blender-mesh-record" => {
            let usage = "usage: relay <blender-mesh-check|blender-mesh-record> <project-id> <project-relative.blend> [--json]";
            let project_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let blend_path = args
                .get(2)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            if args.iter().skip(3).any(|value| value != "--json") {
                return Err(usage.to_string());
            }
            let validation = invoke(&make_request(
                "assets.blender.mesh.validate",
                json!({
                    "project_id": project_id, "blend_path": blend_path
                }),
                None,
            ))?;
            if command == "blender-mesh-check" || !validation.ok {
                if has_json_flag(args) {
                    print_machine(&validation);
                } else {
                    print_human(&validation);
                }
                return Ok(if validation.ok { 0 } else { 2 });
            }
            let payload = validation
                .result
                .ok_or_else(|| "Blender validation returned no result".to_string())?;
            let req_id = request_id();
            let recorded = invoke(&CommandRequest {
                request_id: req_id.clone(),
                command: "result.put".to_string(),
                command_version: Some(1),
                arguments: json!({
                    "project_id": project_id,
                    "kind": "BLENDER_MESH_VALIDATION",
                    "payload": payload
                }),
                idempotency_key: Some(format!("CLI-{req_id}")),
                context: RequestContext::default(),
            })?;
            if has_json_flag(args) {
                print_machine(&recorded);
            } else {
                print_human(&recorded);
            }
            Ok(if recorded.ok { 0 } else { 2 })
        }
        "krita-export" => {
            let usage = "usage: relay krita-export <project-id> <project-relative.kra> <new-project-relative.png> [--json]";
            let project_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let source_path = args
                .get(2)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let export_path = args
                .get(3)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            if args.iter().skip(4).any(|value| value != "--json") {
                return Err(usage.to_string());
            }
            let response = invoke(&make_request(
                "assets.krita.export",
                json!({
                    "project_id": project_id, "source_path": source_path,
                    "export_path": export_path
                }),
                Some(format!("CLI-{}", request_id())),
            ))?;
            if has_json_flag(args) {
                print_machine(&response);
            } else {
                print_human(&response);
            }
            Ok(
                if response.ok
                    && response
                        .result
                        .as_ref()
                        .is_some_and(|value| value["status"] == "exported")
                {
                    0
                } else {
                    2
                },
            )
        }
        "krita-reconcile" => {
            let usage = "usage: relay krita-reconcile <project-id> <project-relative.kra> <existing-project-relative.png> [--json]";
            let project_id = args.get(1).filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let source_path = args.get(2).filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let export_path = args.get(3).filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            if args.iter().skip(4).any(|value| value != "--json") {
                return Err(usage.to_string());
            }
            let response = invoke(&make_request(
                "assets.krita.reconcile",
                json!({
                    "project_id": project_id, "source_path": source_path,
                    "export_path": export_path
                }),
                Some(format!("CLI-{}", request_id())),
            ))?;
            if has_json_flag(args) {
                print_machine(&response);
            } else {
                print_human(&response);
            }
            Ok(if response.ok && response.result.as_ref().is_some_and(|value| {
                value["status"] == "candidate" && value["record_state"] == "candidate_stored"
            }) { 0 } else { 2 })
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
        "context-compile" => {
            let usage = "usage: relay context-compile <project-id> <max-bytes> <result-id,...> [--require result-id=/pointer] [--focus term] [--json]";
            let project_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| usage.to_string())?;
            let max_bytes: u64 = args
                .get(2)
                .ok_or_else(|| usage.to_string())?
                .parse()
                .map_err(|_| usage.to_string())?;
            let result_ids: Vec<&str> = args
                .get(3)
                .ok_or_else(|| usage.to_string())?
                .split(',')
                .collect();
            if result_ids.is_empty() || result_ids.iter().any(|id| id.is_empty()) {
                return Err(usage.to_string());
            }
            let mut required = Vec::new();
            let mut focus_terms = Vec::new();
            let mut position = 4;
            while position < args.len() {
                match args[position].as_str() {
                    "--json" => position += 1,
                    "--require" => {
                        let value = args.get(position + 1).ok_or_else(|| usage.to_string())?;
                        let (result_id, pointer) =
                            value.split_once('=').ok_or_else(|| usage.to_string())?;
                        if result_id.is_empty() || pointer.is_empty() {
                            return Err(usage.to_string());
                        }
                        required.push(json!({ "result_id": result_id, "pointer": pointer }));
                        position += 2;
                    }
                    "--focus" => {
                        focus_terms.push(
                            args.get(position + 1)
                                .ok_or_else(|| usage.to_string())?
                                .as_str(),
                        );
                        position += 2;
                    }
                    _ => return Err(usage.to_string()),
                }
            }
            execute_human(
                make_request(
                    "context.compile",
                    json!({
                        "project_id": project_id,
                        "result_ids": result_ids,
                        "max_bytes": max_bytes,
                        "required_pointers": required,
                        "focus_terms": focus_terms
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "task-context" => {
            let usage = "usage: relay task-context <project-id> <diagnose|implement|review|project_admin> <max-bytes> [--result <id>] [--approval <id>] [--require <result-id=/pointer>] [--focus <term>] [--json]";
            if args.len() < 4 || args[1..4].iter().any(|value| value.starts_with("--")) {
                return Err(usage.to_string());
            }
            let max_bytes: u64 = args[3].parse().map_err(|_| usage.to_string())?;
            let mut result_ids = Vec::new();
            let mut approval_ids = Vec::new();
            let mut required = Vec::new();
            let mut focus_terms = Vec::new();
            let mut position = 4;
            while position < args.len() {
                match args[position].as_str() {
                    "--json" => position += 1,
                    "--result" => {
                        result_ids.push(
                            args.get(position + 1)
                                .ok_or_else(|| usage.to_string())?
                                .as_str(),
                        );
                        position += 2;
                    }
                    "--approval" => {
                        approval_ids.push(
                            args.get(position + 1)
                                .ok_or_else(|| usage.to_string())?
                                .as_str(),
                        );
                        position += 2;
                    }
                    "--require" => {
                        let value = args.get(position + 1).ok_or_else(|| usage.to_string())?;
                        let (result_id, pointer) =
                            value.split_once('=').ok_or_else(|| usage.to_string())?;
                        if result_id.is_empty() || pointer.is_empty() {
                            return Err(usage.to_string());
                        }
                        required.push(json!({"result_id": result_id, "pointer": pointer}));
                        position += 2;
                    }
                    "--focus" => {
                        focus_terms.push(
                            args.get(position + 1)
                                .ok_or_else(|| usage.to_string())?
                                .as_str(),
                        );
                        position += 2;
                    }
                    _ => return Err(usage.to_string()),
                }
            }
            execute_human(
                make_request(
                    "context.task.compile",
                    json!({
                        "project_id": args[1], "task_kind": args[2], "max_bytes": max_bytes,
                        "result_ids": result_ids, "approval_ids": approval_ids,
                        "required_pointers": required, "focus_terms": focus_terms
                    }),
                    None,
                ),
                has_json_flag(args),
                "default",
            )
        }
        "job-list" => {
            let project_id = args.get(1).filter(|value| !value.starts_with("--"));
            let arguments = match project_id {
                Some(project_id) => json!({ "project_id": project_id, "limit": 20 }),
                None => json!({ "limit": 20 }),
            };
            execute_human(
                make_request("job.list", arguments, None),
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
