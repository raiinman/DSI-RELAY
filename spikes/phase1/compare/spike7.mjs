import crypto from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const repoRoot = path.resolve(phase1, "..", "..");
const nodeRoot = path.join(phase1, "node");
const rustRoot = path.join(phase1, "rust");
const resultPath = path.join(phase1, "results", "2026-09-24-spike7-runtime-ipc-challenger-windows.json");
const rustExe = path.join(rustRoot, "target", "release", "relay-rust-challenger.exe");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let currentStage = "initializing";
const stage = value => {
  currentStage = value;
  process.stderr.write(`[spike7] ${value}\n`);
};
process.on("uncaughtException", error => {
  process.stderr.write(`[spike7] uncaught stage=${currentStage} code=${error.code ?? ""} message=${error.message}\n`);
  process.exit(1);
});
process.on("unhandledRejection", error => {
  process.stderr.write(`[spike7] unhandled stage=${currentStage} message=${error?.message ?? error}\n`);
  process.exit(1);
});

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}
function summarize(values, suffix = "ms") {
  if (!values.length) return { n: 0 };
  const key = name => suffix ? `${name}_${suffix}` : name;
  return {
    n: values.length,
    [key("min")]: +Math.min(...values).toFixed(3),
    [key("p50")]: +percentile(values, 50).toFixed(3),
    [key("p95")]: +percentile(values, 95).toFixed(3),
    [key("p99")]: +percentile(values, 99).toFixed(3),
    [key("max")]: +Math.max(...values).toFixed(3),
    [key("mean")]: +(values.reduce((a, b) => a + b, 0) / values.length).toFixed(3)
  };
}

function runSync(exe, args, options = {}) {
  const result = spawnSync(exe, args, {
    encoding: "utf8",
    windowsHide: true,
    ...options
  });
  if (result.status !== 0) {
    throw new Error(`${exe} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result;
}

function measureSync(exe, args, options = {}) {
  const started = performance.now();
  const result = runSync(exe, args, options);
  return { elapsed_ms: performance.now() - started, result };
}

function rustBuildEvidence() {
  const clean = measureSync("cargo.exe", ["clean"], { cwd: rustRoot });
  const release = measureSync("cargo.exe", ["build", "--release"], { cwd: rustRoot });
  const incremental = measureSync("cargo.exe", ["build", "--release"], { cwd: rustRoot });
  const metadata = JSON.parse(
    runSync("cargo.exe", ["metadata", "--format-version", "1"], { cwd: rustRoot }).stdout
  );
  const rootPackage = metadata.packages.find(pkg => pkg.name === "relay-rust-challenger");
  return {
    clean_command_elapsed_ms: +clean.elapsed_ms.toFixed(3),
    clean_release_build_ms: +release.elapsed_ms.toFixed(3),
    incremental_release_build_ms: +incremental.elapsed_ms.toFixed(3),
    direct_dependency_count: rootPackage?.dependencies?.length ?? null,
    resolved_package_count_including_root: metadata.packages.length,
    release_binary_bytes: null
  };
}

async function waitForState(statePath, expectedPid, timeoutMs = 5000) {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (state.pid === expectedPid) return state;
    } catch {}
    await sleep(5);
  }
  throw new Error(`state did not become current: ${statePath}`);
}

function capture(child) {
  let stdout = "";
  let stderr = "";
  child.stdout?.setEncoding("utf8");
  child.stderr?.setEncoding("utf8");
  child.stdout?.on("data", chunk => stdout += chunk);
  child.stderr?.on("data", chunk => stderr += chunk);
  return { stdout: () => stdout, stderr: () => stderr };
}

async function startCandidate(kind, { dashboard = false, stateDir = null, instance = null } = {}) {
  const dir = stateDir ?? await fs.mkdtemp(path.join(os.tmpdir(), `relay-spike7-${kind}-`));
  const instanceName = instance ?? `spike7-${kind}-${process.pid}-${crypto.randomUUID().slice(0, 8)}`;
  const env = {
    ...process.env,
    RELAY_STATE_DIR: dir,
    RELAY_INSTANCE: instanceName
  };

  const started = performance.now();
  let child;
  let statePath;
  if (kind === "node") {
    if (dashboard) env.RELAY_DASHBOARD_MODE = "embedded";
    child = spawn(process.execPath, [path.join(nodeRoot, "src", "daemon.mjs")], {
      cwd: nodeRoot,
      env,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    });
    statePath = path.join(dir, "host.json");
  } else {
    const args = ["host"];
    if (dashboard) args.push("--dashboard");
    child = spawn(rustExe, args, {
      cwd: rustRoot,
      env,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    });
    statePath = path.join(dir, "host-rust.json");
  }
  const output = capture(child);
  const state = await waitForState(statePath, child.pid);
  return {
    kind,
    child,
    state,
    stateDir: dir,
    statePath,
    instance: instanceName,
    env,
    startup_to_state_ms: performance.now() - started,
    output
  };
}

async function waitExit(child, timeoutMs = 3000) {
  if (child.exitCode !== null) return child.exitCode;
  return Promise.race([
    new Promise(resolve => child.once("close", resolve)),
    sleep(timeoutMs).then(() => {
      if (child.exitCode === null) child.kill();
      return "timeout-killed";
    })
  ]);
}

async function stopCandidate(host) {
  if (host.child.exitCode !== null) return { clean: true, state_removed: !(await exists(host.statePath)) };
  try {
    const response = await callPipe(host.state, "system.shutdown", {});
    if (!response.ok) throw new Error(response.error?.code ?? "shutdown failed");
  } catch {
    host.child.kill();
  }
  await waitExit(host.child);
  await sleep(25);
  return {
    clean: host.child.exitCode === 0 || host.child.exitCode === null,
    state_removed: !(await exists(host.statePath))
  };
}

async function exists(file) {
  try { await fs.access(file); return true; } catch { return false; }
}

function attachLines(socket, onLine, onError) {
  let buffer = "";
  socket.setEncoding("utf8");
  socket.on("data", chunk => {
    buffer += chunk;
    while (true) {
      const index = buffer.indexOf("\n");
      if (index < 0) break;
      const line = buffer.slice(0, index).trim();
      buffer = buffer.slice(index + 1);
      if (!line) continue;
      try { onLine(JSON.parse(line)); } catch (error) { onError(error); }
    }
  });
}

function callPipe(state, command, args = {}, options = {}) {
  const timeoutMs = options.timeoutMs ?? 3000;
  const authToken = options.authToken ?? state.auth_token;
  const protocolMin = options.protocolMin ?? 1;
  const protocolMax = options.protocolMax ?? 1;
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(state.pipe);
    const requestId = crypto.randomUUID();
    let settled = false;
    const timer = setTimeout(() => fail(new Error("TIMEOUT")), timeoutMs);
    let greeted = false;
    const finish = value => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.end();
      resolve(value);
    };
    const fail = error => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      reject(error);
    };
    socket.once("error", fail);
    attachLines(socket, message => {
      if (!greeted) {
        if (message.type === "hello_error") return finish(message);
        if (message.type !== "hello_ok") return fail(new Error("BAD_HANDSHAKE"));
        greeted = true;
        socket.write(JSON.stringify({
          type: "command",
          request_id: requestId,
          schema_version: 1,
          command,
          arguments: args
        }) + "\n");
        return;
      }
      if (message.type === "command_result" && message.request_id === requestId) finish(message);
    }, fail);
    socket.once("connect", () => {
      socket.write(JSON.stringify({
        type: "hello",
        auth_token: authToken,
        protocol_min: protocolMin,
        protocol_max: protocolMax,
        client: { name: "spike7-neutral-harness", version: "1" }
      }) + "\n");
    });
  });
}

async function measureDirect(state, count = 500) {
  for (let i = 0; i < 20; i++) {
    const warm = await callPipe(state, "system.status", {});
    if (!warm.ok) throw new Error("status warmup failed");
  }
  const values = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    const response = await callPipe(state, "system.status", {});
    if (!response.ok) throw new Error("status failed");
    values.push(performance.now() - started);
  }
  return summarize(values);
}

function measureCli(kind, stateDir, count = 30) {
  const values = [];
  const env = { ...process.env, RELAY_STATE_DIR: stateDir };
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    const result = kind === "node"
      ? spawnSync(process.execPath, [path.join(nodeRoot, "src", "cli.mjs"), "status", "--json"], {
          cwd: nodeRoot, env, encoding: "utf8", windowsHide: true
        })
      : spawnSync(rustExe, ["status"], {
          cwd: rustRoot, env, encoding: "utf8", windowsHide: true
        });
    if (result.status !== 0) throw new Error(`${kind} CLI failed: ${result.stderr}`);
    const parsed = JSON.parse(result.stdout.trim());
    if (!parsed.ok) throw new Error(`${kind} CLI returned not ok`);
    values.push(performance.now() - started);
  }
  return summarize(values);
}

function processMetrics(pid) {
  const script = `$p=Get-Process -Id ${pid} -ErrorAction Stop; [pscustomobject]@{cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;rss_bytes=[int64]$p.WorkingSet64;private_bytes=[int64]$p.PrivateMemorySize64} | ConvertTo-Json -Compress`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true
  }));
}

async function idleSample(pid, sampleMs = 5000) {
  await sleep(500);
  const before = processMetrics(pid);
  await sleep(sampleMs);
  const after = processMetrics(pid);
  return {
    sample_ms: sampleMs,
    cpu_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
    rss_bytes: after.rss_bytes,
    private_bytes: after.private_bytes,
    rss_delta_bytes: after.rss_bytes - before.rss_bytes
  };
}

function aclProbe(pipeName) {
  const escaped = pipeName.replaceAll("'", "''");
  const script = `
$ErrorActionPreference='Stop'
try {
  $acl=Get-Acl -LiteralPath '${escaped}'
  $current=[System.Security.Principal.WindowsIdentity]::GetCurrent().Name
  $entries=@($acl.Access)
  $mine=@($entries | Where-Object { $_.IdentityReference.Value -eq $current })
  $extra=@($entries | Where-Object { $_.IdentityReference.Value -ne $current })
  [pscustomobject]@{
    query_ok=$true
    owner_is_current_user=($acl.Owner -eq $current)
    access_entry_count=$entries.Count
    current_user_entry_count=$mine.Count
    extra_principal_count=$extra.Count
    current_user_full_control=(@($mine | Where-Object { $_.FileSystemRights.ToString() -match 'FullControl' -and $_.AccessControlType.ToString() -eq 'Allow' }).Count -gt 0)
    protected_dacl=($acl.Sddl -match 'D:P')
  } | ConvertTo-Json -Compress
} catch {
  [pscustomobject]@{query_ok=$false;error_hresult=$_.Exception.HResult} | ConvertTo-Json -Compress
}`;
  const raw = execFileSync("powershell.exe", ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true
  }).trim();
  return JSON.parse(raw);
}

function rawHttpRequest(url, { method = "GET", headers = {}, body = null, agent }) {
  return new Promise((resolve, reject) => {
    const target = new URL(url);
    const request = http.request({
      hostname: target.hostname,
      port: target.port,
      path: target.pathname + target.search,
      method,
      headers,
      agent
    }, response => {
      const chunks = [];
      response.socket?.on("error", () => {});
      response.on("error", reject);
      response.on("data", chunk => chunks.push(chunk));
      response.on("end", () => resolve({
        status: response.statusCode,
        headers: response.headers,
        body: Buffer.concat(chunks)
      }));
    });
    request.once("error", reject);
    if (body !== null) request.write(body);
    request.end();
  });
}

async function dashboardSession(url) {
  const response = await rawHttpRequest(url, {
    agent: false,
    headers: { "connection": "close" }
  });
  if (response.status !== 200) throw new Error(`dashboard root failed: ${response.status}`);
  const html = response.body.toString("utf8");
  const token = html.match(/name="relay-dashboard-token" content="([0-9a-f]+)"/)?.[1];
  if (!token) throw new Error("dashboard token missing");
  return { token };
}

async function measureDashboard(url, token, count = 200) {
  const body = JSON.stringify({ command: "system.status", arguments: {} });
  const invoke = async () => {
    const response = await rawHttpRequest(url + "/api/execute", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "content-length": Buffer.byteLength(body),
        "x-relay-dashboard-token": token,
        "connection": "close"
      },
      body,
      agent: false
    });
    if (response.status !== 200) throw new Error(`dashboard HTTP ${response.status}`);
    const payload = JSON.parse(response.body.toString("utf8"));
    if (!payload.ok) throw new Error(payload.error?.code ?? "dashboard status not ok");
  };
  for (let i = 0; i < 10; i++) await invoke();
  const values = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    await invoke();
    values.push(performance.now() - started);
  }
  return summarize(values);
}

async function startupSeries(kind, count = 12) {
  const values = [];
  let first = null;
  let allClean = true;
  for (let i = 0; i < count; i++) {
    const host = await startCandidate(kind);
    if (i === 0) first = host.startup_to_state_ms;
    values.push(host.startup_to_state_ms);
    const response = await callPipe(host.state, "system.status", {});
    if (!response.ok) throw new Error(`${kind} startup status failed`);
    const stopped = await stopCandidate(host);
    allClean &&= stopped.state_removed;
    await fs.rm(host.stateDir, { recursive: true, force: true });
  }
  return {
    first_run_ms: +first.toFixed(3),
    repeated: summarize(values),
    graceful_state_cleanup_all_runs: allClean
  };
}

async function hardKillRestart(kind) {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), `relay-spike7-restart-${kind}-`));
  const instance = `spike7-restart-${kind}-${process.pid}`;
  let original;
  let replacement;
  try {
    original = await startCandidate(kind, { stateDir, instance });
    const originalPid = original.child.pid;
    original.child.kill();
    await waitExit(original.child);
    const stalePresent = await exists(original.statePath);

    const started = performance.now();
    replacement = await startCandidate(kind, { stateDir, instance });
    const replacementWindow = performance.now() - started;
    const response = await callPipe(replacement.state, "system.status", {});
    return {
      original_pid_replaced: replacement.child.pid !== originalPid,
      stale_state_present_after_hard_kill: stalePresent,
      replacement_state_ready_ms: +replacementWindow.toFixed(3),
      post_restart_status_ok: response.ok === true
    };
  } finally {
    if (replacement) await stopCandidate(replacement).catch(() => {});
    if (original?.child.exitCode === null) original.child.kill();
    await fs.rm(stateDir, { recursive: true, force: true });
  }
}

async function actualCliCompatibility(nodeHost, rustHost) {
  const rustAsNodeDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike7-node-cli-to-rust-"));
  const nodeAsRustDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike7-rust-cli-to-node-"));
  try {
    await fs.writeFile(path.join(rustAsNodeDir, "host.json"), JSON.stringify({
      ...rustHost.state,
      started_at: new Date().toISOString()
    }, null, 2));

    await fs.writeFile(path.join(nodeAsRustDir, "host-rust.json"), JSON.stringify({
      pid: nodeHost.state.pid,
      pipe: nodeHost.state.pipe,
      auth_token: nodeHost.state.auth_token,
      version: nodeHost.state.version,
      protocol: nodeHost.state.protocol,
      capabilities: nodeHost.state.capabilities,
      recovery_state: nodeHost.state.recovery_state,
      ipc_security: { explicit_dacl: false, scope: "unverified", auth_token: true },
      dashboard: null,
      started_at_unix_ms: Date.now()
    }, null, 2));

    const nodeCli = spawnSync(process.execPath, [
      path.join(nodeRoot, "src", "cli.mjs"), "status", "--json"
    ], {
      cwd: nodeRoot,
      env: { ...process.env, RELAY_STATE_DIR: rustAsNodeDir },
      encoding: "utf8",
      windowsHide: true
    });
    const rustCli = spawnSync(rustExe, ["status"], {
      cwd: rustRoot,
      env: { ...process.env, RELAY_STATE_DIR: nodeAsRustDir },
      encoding: "utf8",
      windowsHide: true
    });

    return {
      node_cli_to_rust_host: {
        exit_code: nodeCli.status,
        ok: nodeCli.status === 0 && JSON.parse(nodeCli.stdout.trim()).ok === true
      },
      rust_cli_to_node_host: {
        exit_code: rustCli.status,
        ok: rustCli.status === 0 && JSON.parse(rustCli.stdout.trim()).ok === true
      }
    };
  } finally {
    await fs.rm(rustAsNodeDir, { recursive: true, force: true });
    await fs.rm(nodeAsRustDir, { recursive: true, force: true });
  }
}

async function sourceMetrics() {
  const rustFiles = ["lib.rs", "main.rs", "protocol.rs", "pipe.rs", "security.rs", "state.rs", "dashboard.rs"];
  const nodeFiles = [
    "src/host.mjs", "src/client.mjs", "src/wire.mjs", "src/config.mjs",
    "src/commands.mjs", "src/state.mjs", "src/dashboard-http.mjs", "src/dispatch.mjs"
  ];
  const count = async (root, files) => {
    let lines = 0;
    let bytes = 0;
    let unsafeMentions = 0;
    for (const file of files) {
      const text = await fs.readFile(path.join(root, file), "utf8");
      lines += text.split(/\r?\n/).length;
      bytes += Buffer.byteLength(text);
      unsafeMentions += (text.match(/\bunsafe\b/g) ?? []).length;
    }
    return { files: files.length, lines, bytes, unsafe_mentions: unsafeMentions };
  };
  return {
    node_selected_host_path: await count(nodeRoot, nodeFiles),
    rust_challenger_src: await count(path.join(rustRoot, "src"), rustFiles)
  };
}

async function runCandidateSteady(kind) {
  const host = await startCandidate(kind);
  try {
    const direct = await measureDirect(host.state);
    const status = await callPipe(host.state, "system.status", {});
    const doctor = await callPipe(host.state, "system.doctor", {});
    const cli = measureCli(kind, host.stateDir);
    await sleep(500);
    const idle = await idleSample(host.child.pid);
    const acl = aclProbe(host.state.pipe);
    const wrongAuth = await callPipe(host.state, "system.status", {}, {
      authToken: "0".repeat(64)
    });
    const badProtocol = await callPipe(host.state, "system.status", {}, {
      protocolMin: 99,
      protocolMax: 99
    });
    return {
      host,
      evidence: {
        startup_to_state_ms: +host.startup_to_state_ms.toFixed(3),
        direct_status: direct,
        cli_process_status: cli,
        idle,
        kernel_ipc_security: status.result?.ipc_security ?? null,
        doctor_explicit_dacl_check: doctor.result?.checks?.find(check => check.id === "ipc.explicit_dacl") ?? null,
        external_acl_provider_probe: acl,
        wrong_auth_rejected: wrongAuth.type === "hello_error" && wrongAuth.code === "UNAUTHORIZED",
        incompatible_protocol_rejected:
          badProtocol.type === "hello_error" && badProtocol.code === "PROTOCOL_INCOMPATIBLE"
      }
    };
  } catch (error) {
    await stopCandidate(host).catch(() => {});
    await fs.rm(host.stateDir, { recursive: true, force: true });
    throw error;
  }
}

async function dashboardEvidence(kind) {
  const host = await startCandidate(kind, { dashboard: true });
  try {
    const url = host.state.dashboard?.url;
    if (!url) throw new Error(`${kind} dashboard missing from state`);
    const session = await dashboardSession(url);
    const httpStatus = await measureDashboard(url, session.token);
    await sleep(500);
    const idle = await idleSample(host.child.pid);
    return {
      startup_to_state_ms: +host.startup_to_state_ms.toFixed(3),
      http_status: httpStatus,
      idle,
      same_process_as_host: true
    };
  } finally {
    await stopCandidate(host).catch(() => {});
    await fs.rm(host.stateDir, { recursive: true, force: true });
  }
}

const cleanup = [];
try {
  stage("rust clean/release build");
  const rustBuild = rustBuildEvidence();
  rustBuild.release_binary_bytes = (await fs.stat(rustExe)).size;

  stage("package metadata");
  const nodeExecutableBytes = (await fs.stat(process.execPath)).size;
  const nodePackage = JSON.parse(await fs.readFile(path.join(nodeRoot, "package.json"), "utf8"));

  stage("startup series node");
  const nodeStartup = await startupSeries("node");
  stage("startup series rust");
  const rustStartup = await startupSeries("rust");
  const startups = { node: nodeStartup, rust: rustStartup };

  stage("steady node");
  const nodeSteady = await runCandidateSteady("node");
  cleanup.push(async () => {
    await stopCandidate(nodeSteady.host).catch(() => {});
    await fs.rm(nodeSteady.host.stateDir, { recursive: true, force: true });
  });

  stage("steady rust");
  const rustSteady = await runCandidateSteady("rust");
  cleanup.push(async () => {
    await stopCandidate(rustSteady.host).catch(() => {});
    await fs.rm(rustSteady.host.stateDir, { recursive: true, force: true });
  });

  stage("actual CLI cross-runtime compatibility");
  const compatibility = await actualCliCompatibility(nodeSteady.host, rustSteady.host);

  stage("hard-kill restart node");
  const nodeRestart = await hardKillRestart("node");
  stage("hard-kill restart rust");
  const rustRestart = await hardKillRestart("rust");
  const restart = { node: nodeRestart, rust: rustRestart };

  stage("embedded dashboard node");
  const nodeDashboard = await dashboardEvidence("node");
  stage("embedded dashboard rust");
  const rustDashboard = await dashboardEvidence("rust");
  const dashboards = { node: nodeDashboard, rust: rustDashboard };

  stage("source metrics");
  const sources = await sourceMetrics();

  const result = {
    benchmark: "phase1-spike7-runtime-ipc-challenger-windows",
    recorded_at: new Date().toISOString(),
    decision_informed: "Whether the provisional Node per-user host should remain the runtime/IPC direction or whether a narrow Rust challenger materially improves footprint and explicit Windows IPC security enough to justify deeper porting.",
    hypothesis: "A synchronous Rust host using an explicit current-user named-pipe DACL can materially reduce idle memory/startup and close the IPC ACL gap without requiring an async runtime, while preserving protocol interoperability and embedded-dashboard feasibility.",
    runtime: {
      os: { platform: process.platform, release: os.release(), arch: os.arch() },
      node: process.version,
      rustc: runSync("rustc.exe", ["--version"]).stdout.trim(),
      cargo: runSync("cargo.exe", ["--version"]).stdout.trim(),
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem()
    },
    build_and_package: {
      node: {
        interpreted_source_no_compile_step: true,
        declared_npm_runtime_dependencies: Object.keys(nodePackage.dependencies ?? {}).length,
        node_executable_bytes: nodeExecutableBytes
      },
      rust: rustBuild,
      source_metrics: sources
    },
    startup: startups,
    steady_state: {
      node: nodeSteady.evidence,
      rust: rustSteady.evidence
    },
    dashboard_embedded: dashboards,
    hard_kill_restart: restart,
    protocol_interop: compatibility,
    comparisons: {
      idle_rss_reduction_rust_vs_node: +(1 - rustSteady.evidence.idle.rss_bytes / nodeSteady.evidence.idle.rss_bytes).toFixed(6),
      embedded_dashboard_idle_rss_reduction_rust_vs_node: +(1 - dashboards.rust.idle.rss_bytes / dashboards.node.idle.rss_bytes).toFixed(6),
      direct_status_p50_ratio_rust_to_node: +(rustSteady.evidence.direct_status.p50_ms / nodeSteady.evidence.direct_status.p50_ms).toFixed(4),
      cli_status_p50_ratio_rust_to_node: +(rustSteady.evidence.cli_process_status.p50_ms / nodeSteady.evidence.cli_process_status.p50_ms).toFixed(4)
    },
    pass_fail: {
      rust_kernel_current_user_acl_verified:
        rustSteady.evidence.kernel_ipc_security?.kernel_acl_verified === true &&
        rustSteady.evidence.kernel_ipc_security?.owner_current_user === true &&
        rustSteady.evidence.kernel_ipc_security?.acl_ace_count === 1 &&
        rustSteady.evidence.kernel_ipc_security?.scope === "current_user" &&
        rustSteady.evidence.doctor_explicit_dacl_check?.status === "pass",
      rust_wrong_token_fails_closed: rustSteady.evidence.wrong_auth_rejected,
      rust_incompatible_protocol_fails_closed: rustSteady.evidence.incompatible_protocol_rejected,
      node_wrong_token_fails_closed: nodeSteady.evidence.wrong_auth_rejected,
      node_incompatible_protocol_fails_closed: nodeSteady.evidence.incompatible_protocol_rejected,
      node_cli_can_use_rust_host: compatibility.node_cli_to_rust_host.ok,
      rust_cli_can_use_node_host: compatibility.rust_cli_to_node_host.ok,
      rust_hard_kill_restart_recovers: restart.rust.post_restart_status_ok,
      node_hard_kill_restart_recovers: restart.node.post_restart_status_ok,
      rust_idle_rss_lower_than_node: rustSteady.evidence.idle.rss_bytes < nodeSteady.evidence.idle.rss_bytes,
      rust_embedded_dashboard_rss_lower_than_node: dashboards.rust.idle.rss_bytes < dashboards.node.idle.rss_bytes
    },
    limitations: [
      "The Rust challenger intentionally does not port SQLite, evidence storage, indexing, background jobs, or adapter workers. Its memory advantage is a host/runtime result, not a complete-product memory forecast.",
      "The current Node candidate is an integrated Phase 1 prototype with storage and more commands loaded; startup and RSS comparisons therefore include functionality Rust has not yet earned permission to port.",
      "The Rust challenger uses synchronous blocking named-pipe I/O and a small thread-per-dashboard-connection server. Concurrency/scalability beyond the narrow local control-plane workload is not yet approved.",
      "The Rust host now calls Windows GetSecurityInfo on the created named-pipe handle and fails startup unless the kernel descriptor is protected, owned by the current user, and contains exactly one full-control ACE for that user. PowerShell Get-Acl proved unreliable for idle named-pipe paths and is retained only as a best-effort external observation. Node/libuv ACL state remains unverified rather than being classified as permissive.",
      "Build/package evidence does not yet include signed installer size, update mechanism, crash-symbol handling, or third-party license bundling.",
      "Only one high-end Windows workstation was benchmarked."
    ]
  };

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(resultPath, JSON.stringify(result, null, 2) + "\n", "utf8");
  console.log(JSON.stringify(result, null, 2));
} finally {
  for (const fn of cleanup.reverse()) await fn().catch(() => {});
}
