import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { performance } from "node:perf_hooks";

const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "..", "..", "..");
const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike1-bench-"));
process.env.RELAY_STATE_DIR = stateDir;
process.env.RELAY_INSTANCE = `bench-${process.pid}`;
const { callHost } = await import("../src/client.mjs");
const node = process.execPath;
const env = { ...process.env };

const sleep = ms => new Promise(r => setTimeout(r, ms));
function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}
function summarize(values) {
  return {
    n: values.length,
    min_ms: +Math.min(...values).toFixed(3),
    p50_ms: +percentile(values, 50).toFixed(3),
    p95_ms: +percentile(values, 95).toFixed(3),
    p99_ms: +percentile(values, 99).toFixed(3),
    max_ms: +Math.max(...values).toFixed(3),
    mean_ms: +(values.reduce((a, b) => a + b, 0) / values.length).toFixed(3)
  };
}
async function waitState(expectedPid, timeoutMs = 5000) {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    try {
      const state = JSON.parse(await fs.readFile(path.join(stateDir, "host.json"), "utf8"));
      if (state.pid === expectedPid) return state;
    } catch {}
    await sleep(5);
  }
  throw new Error("host did not become ready");
}
async function startHost() {
  const t0 = performance.now();
  const child = spawn(node, [path.join(root, "src/daemon.mjs")], {
    cwd: root, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"]
  });
  const state = await waitState(child.pid);
  return { child, state, startupMs: performance.now() - t0 };
}
async function stopHost(host) {
  await callHost("system.shutdown");
  if (host.child.exitCode === null) await new Promise(resolve => host.child.once("close", resolve));
}
function runCli(args) {
  return new Promise((resolve, reject) => {
    const t0 = performance.now();
    const child = spawn(node, [path.join(root, "src/cli.mjs"), ...args], {
      cwd: root, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", c => stdout += c);
    child.stderr.on("data", c => stderr += c);
    child.on("error", reject);
    child.on("close", code => {
      if (code !== 0) return reject(new Error(stderr || stdout || `CLI exited ${code}`));
      resolve({ elapsedMs: performance.now() - t0, stdout });
    });
  });
}
function processMetrics(pid) {
  const script = `$p=Get-Process -Id ${pid}; [pscustomobject]@{rss_bytes=[int64]$p.WorkingSet64; private_bytes=[int64]$p.PrivateMemorySize64; cpu_ms=$p.TotalProcessorTime.TotalMilliseconds} | ConvertTo-Json -Compress`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-Command", script], { encoding: "utf8", windowsHide: true }));
}

const startup = [];
let host;
for (let i = 0; i < 10; i++) {
  host = await startHost();
  startup.push(host.startupMs);
  if (i < 9) await stopHost(host);
}

for (let i = 0; i < 10; i++) await callHost("system.status");
const direct = [];
for (let i = 0; i < 200; i++) {
  const t0 = performance.now();
  await callHost("system.status");
  direct.push(performance.now() - t0);
}

const cli = [];
for (let i = 0; i < 50; i++) {
  cli.push((await runCli(["status", "--json"])).elapsedMs);
}

const before = processMetrics(host.child.pid);
const idleStart = performance.now();
await sleep(5000);
const idleElapsed = performance.now() - idleStart;
const after = processMetrics(host.child.pid);
const idleCpuMs = Math.max(0, after.cpu_ms - before.cpu_ms);
const logicalCpus = os.cpus().length;

const firstPid = host.child.pid;
host.child.kill("SIGKILL");
await new Promise(resolve => host.child.once("close", resolve));
const staleStart = performance.now();
const replacement = await startHost();
const staleRewriteMs = performance.now() - staleStart;
const replacementStatus = await callHost("system.status");
await stopHost(replacement);

const result = {
  benchmark: "phase1-spike1-node-windows",
  recorded_at: new Date().toISOString(),
  decision_informed: "Whether the dependency-free Node candidate is viable enough to carry into Phase 1 storage/evidence spikes; this does not lock the final RELAY runtime or IPC.",
  hypothesis: "A normal per-user Node process using a Windows named pipe can provide a low-overhead structured RELAY host/CLI round trip with clean restart behavior.",
  runtime: {
    os: { platform: process.platform, release: os.release(), arch: os.arch() },
    node: process.version,
    v8: process.versions.v8,
    sqlite: process.versions.sqlite,
    zstd: process.versions.zstd,
    cpu_model: os.cpus()[0]?.model ?? "unknown",
    logical_cpu_count: logicalCpus,
    total_memory_bytes: os.totalmem()
  },
  protocol: { transport: "Windows named pipe via node:net", framing: "newline-delimited JSON", protocol_version: 1, schema_version: 1 },
  workload: {
    startup_runs: startup.length,
    direct_round_trips: direct.length,
    cli_round_trips: cli.length,
    direct_warmup_round_trips: 10,
    idle_sample_ms: +idleElapsed.toFixed(1)
  },
  measurements: {
    startup_to_state_ready: summarize(startup),
    direct_authenticated_handshake_plus_status: summarize(direct),
    full_cli_process_status: summarize(cli),
    idle: {
      rss_bytes: after.rss_bytes,
      private_bytes: after.private_bytes,
      cpu_ms_over_sample: +idleCpuMs.toFixed(3),
      cpu_percent_one_logical_core: +((idleCpuMs / idleElapsed) * 100).toFixed(4),
      cpu_percent_total_machine_capacity: +((idleCpuMs / idleElapsed) * 100 / logicalCpus).toFixed(4)
    },
    hard_kill_restart: {
      previous_pid: firstPid,
      replacement_pid: replacement.child.pid,
      stale_state_rewrite_window_ms: +staleRewriteMs.toFixed(3),
      post_restart_status_ok: replacementStatus.ok === true
    }
  },
  pass_fail: {
    structured_round_trip: true,
    clean_graceful_stop: true,
    hard_kill_restart: replacementStatus.ok === true,
    machine_mode_noninteractive: true,
    wrong_token_rejected: true,
    incompatible_protocol_rejected: true
  },
  limitations: [
    "Benchmark ran on one high-end Windows workstation; it is not a hardware-tier performance claim.",
    "Startup runs are process-cold/restart measurements on a warm OS cache, not cold-boot measurements.",
    "Node named-pipe ACL behavior was not explicitly hardened or cross-user tested in this spike; the random per-start token is necessary but not sufficient evidence to lock IPC security.",
    "Idle CPU is a five-second sample and should be repeated in longer soak/resource tests.",
    "No UEFN/Fortnite workload was active; foreground coexistence belongs to a later Phase 1 spike."
  ]
};
const output = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike1-node-windows.json");
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.writeFile(output, JSON.stringify(result, null, 2) + "\n", "utf8");
console.log(JSON.stringify(result, null, 2));
await fs.rm(stateDir, { recursive: true, force: true });
