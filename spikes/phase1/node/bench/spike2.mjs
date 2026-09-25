import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { performance } from "node:perf_hooks";

const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "..", "..", "..");
const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike2-bench-"));
process.env.RELAY_STATE_DIR = stateDir;
process.env.RELAY_INSTANCE = `spike2-bench-${process.pid}`;
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
function processMetrics(pid) {
  const script = `$p=Get-Process -Id ${pid}; [pscustomobject]@{rss_bytes=[int64]$p.WorkingSet64; private_bytes=[int64]$p.PrivateMemorySize64; cpu_ms=$p.TotalProcessorTime.TotalMilliseconds} | ConvertTo-Json -Compress`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-Command", script], { encoding: "utf8", windowsHide: true }));
}
async function fileSize(file) {
  try { return (await fs.stat(file)).size; } catch { return 0; }
}

let host = await startHost();
const project = (await callHost("project.register", {
  id: "PRJ-storage-benchmark",
  name: "Storage Benchmark Fixture",
  root_uri: "file:///C:/relay-fixture"
})).result;

const payloadBase = {
  summary: "Synthetic durable result fixture",
  exact_ids: Array.from({ length: 16 }, (_, i) => `ENTITY-${String(i).padStart(4, "0")}`),
  repeated_findings: Array.from({ length: 24 }, (_, i) => ({ rule: `R-${i % 6}`, severity: i % 3, message: "deterministic fixture finding" })),
  note: "x".repeat(2048)
};

const writeTimes = [];
const resultIds = [];
for (let i = 0; i < 300; i++) {
  const t0 = performance.now();
  const response = await callHost("result.put", {
    project_id: project.id,
    kind: "BENCH",
    payload: { ...payloadBase, sequence: i }
  });
  writeTimes.push(performance.now() - t0);
  resultIds.push(response.result.id);
}

const readTimes = [];
for (let i = 0; i < 600; i++) {
  const id = resultIds[(i * 37) % resultIds.length];
  const t0 = performance.now();
  const response = await callHost("result.get", { result_id: id });
  if (!response.ok || response.result.id !== id) throw new Error("result read mismatch");
  readTimes.push(performance.now() - t0);
}

const checkpointTimes = [];
let jobId;
for (let i = 0; i < 100; i++) {
  const t0 = performance.now();
  const response = await callHost("job.checkpoint", {
    id: jobId,
    project_id: project.id,
    command: "benchmark.pipeline",
    state: i === 99 ? "CHECKPOINTED" : "RUNNING",
    checkpoint: { completed_stage: i, exact_cursor: `cursor-${i}` },
    result_id: resultIds[i % resultIds.length]
  });
  checkpointTimes.push(performance.now() - t0);
  jobId = response.result.id;
}

const integrityTimes = [];
for (let i = 0; i < 20; i++) {
  const t0 = performance.now();
  const check = await callHost("storage.integrity");
  if (!check.result.ok) throw new Error("integrity check failed");
  integrityTimes.push(performance.now() - t0);
}

const dbPath = path.join(stateDir, "relay.sqlite3");
const sizesBeforeKill = {
  db_bytes: await fileSize(dbPath),
  wal_bytes: await fileSize(dbPath + "-wal"),
  shm_bytes: await fileSize(dbPath + "-shm")
};
const metricsAfterWork = processMetrics(host.child.pid);
const persistedId = resultIds[173];

host.child.kill("SIGKILL");
await new Promise(resolve => host.child.once("close", resolve));
const restart = await startHost();
host = restart;
const persisted = await callHost("result.get", { result_id: persistedId });
const checkpoint = await callHost("job.get", { job_id: jobId });
const postRestartIntegrity = await callHost("storage.integrity");
const metricsAfterRestart = processMetrics(host.child.pid);
await stopHost(host);

const sizesAfterCleanClose = {
  db_bytes: await fileSize(dbPath),
  wal_bytes: await fileSize(dbPath + "-wal"),
  shm_bytes: await fileSize(dbPath + "-shm")
};

const result = {
  benchmark: "phase1-spike2-node-sqlite-windows",
  recorded_at: new Date().toISOString(),
  decision_informed: "Whether Node's built-in SQLite is viable for RELAY durable operational metadata/results/checkpoints during Phase 1.",
  hypothesis: "SQLite WAL with synchronous=FULL can provide simple crash-recoverable local durability with acceptable latency for RELAY metadata and compact results.",
  runtime: {
    os: { platform: process.platform, release: os.release(), arch: os.arch() },
    node: process.version,
    sqlite: process.versions.sqlite,
    cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
    logical_cpu_count: os.cpus().length,
    total_memory_bytes: os.totalmem()
  },
  storage: {
    engine: "SQLite",
    journal_mode: "WAL",
    synchronous: "FULL",
    schema_version: 1,
    payload_model: "compact JSON result rows; heavyweight evidence intentionally excluded"
  },
  workload: {
    projects: 1,
    result_writes: writeTimes.length,
    result_reads: readTimes.length,
    checkpoint_updates: checkpointTimes.length,
    integrity_checks: integrityTimes.length,
    approximate_payload_json_bytes: Buffer.byteLength(JSON.stringify({ ...payloadBase, sequence: 0 }))
  },
  measurements: {
    startup_with_existing_store_ms: +restart.startupMs.toFixed(3),
    result_put_round_trip: summarize(writeTimes),
    result_get_round_trip: summarize(readTimes),
    checkpoint_round_trip: summarize(checkpointTimes),
    quick_check_round_trip: summarize(integrityTimes),
    files_before_hard_kill: sizesBeforeKill,
    files_after_clean_close: sizesAfterCleanClose,
    host_after_work: metricsAfterWork,
    host_after_restart: metricsAfterRestart
  },
  pass_fail: {
    project_registry: project.id === "PRJ-storage-benchmark",
    durable_result_after_hard_kill: persisted.ok === true && persisted.result.id === persistedId,
    durable_checkpoint_after_hard_kill: checkpoint.ok === true && checkpoint.result.id === jobId,
    integrity_after_hard_kill: postRestartIntegrity.result.ok === true,
    schema_version: postRestartIntegrity.result.schema_version === 1
  },
  limitations: [
    "Single workstation and local NVMe-class storage only; synced/network-backed paths are not evaluated.",
    "This spike stores compact normalized result JSON in SQLite. It does not approve multi-megabyte evidence BLOBs in the database.",
    "The workload is single-host/single-writer; concurrent reader/writer and long-reader checkpoint behavior remain for storage-maintenance testing.",
    "VACUUM/checkpoint starvation, disk-full injection, backup/restore, and migration interruption are not yet covered by this narrow result-round-trip spike."
  ]
};
const output = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike2-node-sqlite-windows.json");
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.writeFile(output, JSON.stringify(result, null, 2) + "\n", "utf8");
console.log(JSON.stringify(result, null, 2));
await fs.rm(stateDir, { recursive: true, force: true });
