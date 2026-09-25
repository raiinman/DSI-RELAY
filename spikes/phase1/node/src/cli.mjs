#!/usr/bin/env node
import { callHost } from "./client.mjs";
import { RELAY_VERSION, SCHEMA_VERSION } from "./config.mjs";

const args = process.argv.slice(2);
const json = args.includes("--json");
const command = args.find(value => !value.startsWith("-"));
const machineError = (code, message) => ({ schema_version: SCHEMA_VERSION, ok: false, error: { code, message }, producer: { version: RELAY_VERSION } });
const print = value => process.stdout.write(`${typeof value === "string" ? value : JSON.stringify(value)}\n`);

async function main() {
  if (args.includes("--version")) return print(RELAY_VERSION);
  if (command === "status" || command === "doctor") {
    const response = await callHost(`system.${command}`);
    if (json) print(response);
    else if (command === "status") print(`RELAY ${response.result.version} — ${response.result.recovery_state} — PID ${response.result.pid}`);
    else print(response.result.healthy ? `RELAY doctor: OK — ${response.result.summary}` : `RELAY doctor: problem — ${response.result.summary}`);
    return response.ok ? 0 : 3;
  }
  if (command === "exec" && args.includes("--stdin") && json) {
    const raw = await new Promise(resolve => { let data = ""; process.stdin.setEncoding("utf8"); process.stdin.on("data", c => data += c); process.stdin.on("end", () => resolve(data)); });
    let request;
    try { request = JSON.parse(raw); } catch { print(machineError("BAD_REQUEST", "stdin must contain one JSON object.")); return 2; }
    if (!request?.command || typeof request.command !== "string") { print(machineError("BAD_REQUEST", "command is required.")); return 2; }
    const response = await callHost(request.command, request.arguments ?? {});
    print(response);
    return response.ok ? 0 : 3;
  }
  print(json ? machineError("BAD_REQUEST", "Use status, doctor, or exec --stdin --json.") : "Usage: relay status [--json] | relay doctor [--json] | relay exec --stdin --json");
  return 2;
}

main().then(code => { if (Number.isInteger(code)) process.exitCode = code; }).catch(error => {
  print(json ? machineError(error.code ?? "INTERNAL", error.message) : `RELAY error: ${error.message}`);
  process.exitCode = error.code === "HOST_UNAVAILABLE" ? 4 : error.code === "PROTOCOL_INCOMPATIBLE" ? 5 : 70;
});
