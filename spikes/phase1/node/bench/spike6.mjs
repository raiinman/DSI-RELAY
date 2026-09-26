import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import http from "node:http";
import { performance } from "node:perf_hooks";
import { callHostWithState } from "../src/client.mjs";

const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "..", "..", "..");
const resultPath = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike6-dashboard-shell-windows.json");
const node = process.execPath;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}
function summarize(values) {
  if (!values.length) return { n: 0 };
  return {
    n: values.length,
    min_ms: +Math.min(...values).toFixed(3),
    p50_ms: +percentile(values, 50).toFixed(3),
    p95_ms: +percentile(values, 95).toFixed(3),
    p99_ms: +percentile(values, 99).toFixed(3),
    max_ms: +Math.max(...values).toFixed(3),
    mean_ms: +(values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3)
  };
}

async function waitForState(statePath, expectedPid, timeoutMs = 5000) {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (state.pid === expectedPid) return state;
    } catch {}
    await sleep(10);
  }
  throw new Error("host state did not become current");
}

async function startHost({ stateDir, instance, embedded = false }) {
  const env = {
    ...process.env,
    RELAY_STATE_DIR: stateDir,
    RELAY_INSTANCE: instance,
    ...(embedded ? { RELAY_DASHBOARD_MODE: "embedded" } : {})
  };
  const started = performance.now();
  const child = spawn(node, [path.join(root, "src", "daemon.mjs")], {
    cwd: root,
    env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  let stderr = "";
  child.stderr.on("data", chunk => stderr += chunk);
  const state = await waitForState(path.join(stateDir, "host.json"), child.pid);
  return {
    child,
    state,
    env,
    startup_to_state_ms: performance.now() - started,
    stderr: () => stderr
  };
}

async function stopHost(host) {
  if (host.child.exitCode !== null) return;
  const closing = new Promise(resolve => host.child.once("close", resolve));
  await callHostWithState(host.state, "system.shutdown");
  if (host.child.exitCode === null) await closing;
}

function firstJsonLine(child, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(() => reject(new Error("dashboard did not report ready")), timeoutMs);
    child.stdout.setEncoding("utf8");
    const onData = chunk => {
      buffer += chunk;
      const index = buffer.indexOf("\n");
      if (index < 0) return;
      clearTimeout(timer);
      child.stdout.off("data", onData);
      resolve(JSON.parse(buffer.slice(0, index)));
    };
    child.stdout.on("data", onData);
    child.once("error", error => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function startStandaloneDashboard(env) {
  const started = performance.now();
  const child = spawn(node, [path.join(root, "src", "dashboard.mjs")], {
    cwd: root,
    env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const ready = await firstJsonLine(child);
  return {
    child,
    ...ready,
    startup_to_ready_ms: performance.now() - started
  };
}

async function stopProcess(child) {
  if (child.exitCode !== null) return;
  const closing = new Promise(resolve => child.once("close", resolve));
  child.kill("SIGTERM");
  if (child.exitCode === null) await closing;
}

async function dashboardSession(url) {
  const response = await fetch(url);
  const html = await response.text();
  if (!response.ok) throw new Error(`dashboard root failed: ${response.status}`);
  const token = html.match(/name="relay-dashboard-token" content="([0-9a-f]+)"/)?.[1];
  if (!token) throw new Error("dashboard token missing");
  return {
    token,
    csp: response.headers.get("content-security-policy"),
    html_bytes: Buffer.byteLength(html)
  };
}

async function dashboardCommand(url, token, command = "system.status", args = {}) {
  const response = await fetch(url + "/api/execute", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-relay-dashboard-token": token
    },
    body: JSON.stringify({ command, arguments: args })
  });
  const body = await response.json();
  if (!response.ok || body.ok !== true) {
    throw new Error(`dashboard command failed: http=${response.status} code=${body?.error?.code}`);
  }
  return body;
}

async function measureDirect(state, count = 300) {
  for (let i = 0; i < 10; i++) await callHostWithState(state, "system.status");
  const times = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    const response = await callHostWithState(state, "system.status");
    if (!response.ok) throw new Error("direct status failed");
    times.push(performance.now() - started);
  }
  return summarize(times);
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

async function measureDashboard(url, token, count = 300) {
  const agent = new http.Agent({ keepAlive: true, maxSockets: 1 });
  const body = JSON.stringify({ command: "system.status", arguments: {} });
  const invoke = async () => {
    const response = await rawHttpRequest(url + "/api/execute", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "content-length": Buffer.byteLength(body),
        "x-relay-dashboard-token": token
      },
      body,
      agent
    });
    const payload = JSON.parse(response.body.toString("utf8"));
    if (response.status !== 200 || payload.ok !== true) throw new Error("dashboard HTTP status failed");
  };
  for (let i = 0; i < 10; i++) await invoke();
  const times = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    await invoke();
    times.push(performance.now() - started);
  }
  agent.destroy();
  return summarize(times);
}

async function measureRoot(url, count = 50) {
  const agent = new http.Agent({ keepAlive: true, maxSockets: 1 });
  const times = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    const response = await rawHttpRequest(url, { agent });
    if (response.status !== 200) throw new Error("dashboard root failed");
    times.push(performance.now() - started);
  }
  agent.destroy();
  return summarize(times);
}

function processMetrics(pid) {
  const command = `$p=Get-Process -Id ${pid} -ErrorAction Stop; [pscustomobject]@{pid=$p.Id;cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;rss_bytes=[int64]$p.WorkingSet64;private_bytes=[int64]$p.PrivateMemorySize64} | ConvertTo-Json -Compress`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-Command", command], {
    encoding: "utf8",
    windowsHide: true
  }));
}

async function idleSample(pids, sampleMs = 5000) {
  const before = Object.fromEntries(pids.map(pid => [pid, processMetrics(pid)]));
  await sleep(sampleMs);
  const after = Object.fromEntries(pids.map(pid => [pid, processMetrics(pid)]));
  return {
    sample_ms: sampleMs,
    processes: pids.map(pid => ({
      pid,
      cpu_ms: +(after[pid].cpu_ms - before[pid].cpu_ms).toFixed(3),
      rss_bytes: after[pid].rss_bytes,
      private_bytes: after[pid].private_bytes,
      rss_delta_bytes: after[pid].rss_bytes - before[pid].rss_bytes
    })),
    total_rss_bytes: pids.reduce((sum, pid) => sum + after[pid].rss_bytes, 0),
    total_private_bytes: pids.reduce((sum, pid) => sum + after[pid].private_bytes, 0),
    total_cpu_ms: +pids.reduce((sum, pid) => sum + Math.max(0, after[pid].cpu_ms - before[pid].cpu_ms), 0).toFixed(3)
  };
}

async function createStateDir(label) {
  return fs.mkdtemp(path.join(os.tmpdir(), `relay-spike6-${label}-`));
}

async function assetSizes() {
  const names = ["index.html", "app.js", "styles.css"];
  const entries = {};
  let total = 0;
  for (const name of names) {
    const bytes = (await fs.stat(path.join(root, "dashboard", name))).size;
    entries[name] = bytes;
    total += bytes;
  }
  return { files: entries, total_bytes: total };
}

const cleanup = [];
try {
  const baselineDir = await createStateDir("baseline");
  cleanup.push(() => fs.rm(baselineDir, { recursive: true, force: true }));
  const baseline = await startHost({
    stateDir: baselineDir,
    instance: `dash-bench-baseline-${process.pid}`
  });
  const baselineDirect = await measureDirect(baseline.state);
  await sleep(750);
  const baselineIdle = await idleSample([baseline.child.pid]);
  await stopHost(baseline);

  const standaloneDir = await createStateDir("standalone");
  cleanup.push(() => fs.rm(standaloneDir, { recursive: true, force: true }));
  const standaloneHost = await startHost({
    stateDir: standaloneDir,
    instance: `dash-bench-standalone-${process.pid}`
  });
  const standalone = await startStandaloneDashboard(standaloneHost.env);
  const standaloneSession = await dashboardSession(standalone.url);
  const standaloneDirect = await measureDirect(standaloneHost.state);
  const standaloneHttp = await measureDashboard(standalone.url, standaloneSession.token);
  const standaloneRoot = await measureRoot(standalone.url);
  await sleep(750);
  const standaloneIdle = await idleSample([standaloneHost.child.pid, standalone.child.pid]);
  await stopProcess(standalone.child);
  await stopHost(standaloneHost);

  const embeddedDir = await createStateDir("embedded");
  cleanup.push(() => fs.rm(embeddedDir, { recursive: true, force: true }));
  const embedded = await startHost({
    stateDir: embeddedDir,
    instance: `dash-bench-embedded-${process.pid}`,
    embedded: true
  });
  const embeddedSession = await dashboardSession(embedded.state.dashboard.url);
  const embeddedDirect = await measureDirect(embedded.state);
  const embeddedHttp = await measureDashboard(embedded.state.dashboard.url, embeddedSession.token);
  const embeddedRoot = await measureRoot(embedded.state.dashboard.url);
  await sleep(750);
  const embeddedIdle = await idleSample([embedded.child.pid]);
  await stopHost(embedded);

  const assets = await assetSizes();
  const packageJson = JSON.parse(await fs.readFile(path.join(root, "package.json"), "utf8"));
  const standaloneExtraRss = standaloneIdle.total_rss_bytes - baselineIdle.total_rss_bytes;
  const embeddedExtraRss = embeddedIdle.total_rss_bytes - baselineIdle.total_rss_bytes;

  const result = {
    benchmark: "phase1-spike6-dashboard-shell-windows",
    recorded_at: new Date().toISOString(),
    decision_informed: "Whether RELAY's Phase 1 dashboard should run as a separate local process or as a thin HTTP/static client surface hosted by relayd, while preserving one structured command system.",
    hypothesis: "A dependency-free dashboard shell hosted inside relayd will preserve command-contract parity with materially less idle memory than a standalone dashboard proxy process.",
    runtime: {
      os: { platform: process.platform, release: os.release(), arch: os.arch() },
      node: process.version,
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem()
    },
    shell: {
      asset_sizes: assets,
      declared_runtime_dependencies: Object.keys(packageJson.dependencies ?? {}).length,
      exposed_commands: embedded.state.dashboard.commands,
      authentication: "per-start random dashboard token injected only into same-origin HTML; API requires X-Relay-Dashboard-Token",
      security_headers: {
        csp_present: Boolean(embeddedSession.csp),
        frame_ancestors_none: embeddedSession.csp?.includes("frame-ancestors 'none'") ?? false
      }
    },
    baseline_host_only: {
      startup_to_state_ms: +baseline.startup_to_state_ms.toFixed(3),
      direct_named_pipe_status: baselineDirect,
      idle: baselineIdle
    },
    standalone_proxy: {
      host_startup_to_state_ms: +standaloneHost.startup_to_state_ms.toFixed(3),
      dashboard_startup_to_ready_ms: +standalone.startup_to_ready_ms.toFixed(3),
      direct_named_pipe_status: standaloneDirect,
      dashboard_http_status: standaloneHttp,
      root_asset_request: standaloneRoot,
      idle: standaloneIdle,
      extra_rss_vs_host_only_bytes: standaloneExtraRss,
      process_count: 2
    },
    embedded_host: {
      startup_to_state_ms: +embedded.startup_to_state_ms.toFixed(3),
      direct_named_pipe_status: embeddedDirect,
      dashboard_http_status: embeddedHttp,
      root_asset_request: embeddedRoot,
      idle: embeddedIdle,
      extra_rss_vs_host_only_bytes: embeddedExtraRss,
      process_count: 1
    },
    pass_fail: {
      zero_runtime_dependencies: Object.keys(packageJson.dependencies ?? {}).length === 0,
      strict_csp_present: Boolean(embeddedSession.csp) && embeddedSession.csp.includes("frame-ancestors 'none'"),
      embedded_dashboard_exposes_only_read_commands: embedded.state.dashboard.commands.every(command =>
        ["system.status", "system.doctor", "project.list", "result.get"].includes(command)
      ),
      embedded_adds_less_rss_than_standalone: embeddedExtraRss < standaloneExtraRss,
      embedded_idle_cpu_under_20ms_over_5s: embeddedIdle.total_cpu_ms <= 20,
      standalone_idle_cpu_under_20ms_over_5s: standaloneIdle.total_cpu_ms <= 20
    },
    limitations: [
      "This spike benchmarks HTTP/API/static-shell overhead, not full browser renderer memory; Chrome/Edge process cost belongs to the user's existing browser environment.",
      "The dashboard is intentionally read-only and minimal; approvals, configuration writes, live streaming/WebSocket updates, and accessibility/usability studies remain later work.",
      "The embedded HTTP adapter currently uses an in-process shared dispatcher while standalone mode proxies over named-pipe IPC. Both were parity-tested against the same command semantics, but final authorization/client-identity plumbing is still open.",
      "Loopback binding plus same-origin token/CSP is a Phase 1 local-shell boundary, not a completed public threat model. Explicit browser/client identity and installer/update hardening remain open.",
      "One high-end Windows workstation cannot establish public dashboard resource budgets."
    ]
  };

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(resultPath, JSON.stringify(result, null, 2) + "\n", "utf8");
  console.log(JSON.stringify(result, null, 2));
} finally {
  for (const remove of cleanup.reverse()) await remove().catch(() => {});
}
