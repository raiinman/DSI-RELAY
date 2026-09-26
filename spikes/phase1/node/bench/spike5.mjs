import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFile, execFileSync, spawn } from "node:child_process";
import { promisify } from "node:util";
import { performance } from "node:perf_hooks";
import { RelayStorage } from "../src/storage.mjs";
import { chooseResourceMode, schedulingDecision } from "../src/resource-scheduler.mjs";

const execFileAsync = promisify(execFile);
const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "..", "..", "..");
const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike5-"));
const resultPath = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike5-resource-coexistence-windows.json");
const node = process.execPath;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function psQuote(value) {
  return String(value).replaceAll("'", "''");
}

function compileHelper(source, output) {
  const command = `Add-Type -Path '${psQuote(source)}' -OutputAssembly '${psQuote(output)}' -OutputType ConsoleApplication`;
  execFileSync("powershell.exe", ["-NoProfile", "-Command", command], {
    windowsHide: true,
    encoding: "utf8"
  });
}

function parseJsonMaybeArray(raw) {
  if (!raw?.trim()) return [];
  const value = JSON.parse(raw.trim());
  return Array.isArray(value) ? value : [value];
}

function creatorSnapshot() {
  const command = [
    "$names=@('UnrealEditorFortnite-Win64-Shipping','FortniteClient-Win64-Shipping','UnrealEditor','blender','krita');",
    "Get-Process -ErrorAction SilentlyContinue |",
    "Where-Object { $names -contains $_.ProcessName } |",
    "Select-Object Id,ProcessName,MainWindowHandle,MainWindowTitle,WorkingSet64,CPU,PriorityClass |",
    "ConvertTo-Json -Compress"
  ].join(" ");
  const raw = execFileSync("powershell.exe", ["-NoProfile", "-Command", command], {
    encoding: "utf8",
    windowsHide: true
  });
  return parseJsonMaybeArray(raw);
}

function processMetrics(pid) {
  const command = `$p=Get-Process -Id ${pid} -ErrorAction Stop; [pscustomobject]@{pid=$p.Id;cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;rss_bytes=[int64]$p.WorkingSet64;private_bytes=[int64]$p.PrivateMemorySize64;priority=[string]$p.PriorityClass} | ConvertTo-Json -Compress`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-Command", command], {
    encoding: "utf8",
    windowsHide: true
  }));
}

function gpuSnapshot() {
  try {
    const raw = execFileSync("nvidia-smi.exe", [
      "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
      "--format=csv,noheader,nounits"
    ], { encoding: "utf8", windowsHide: true }).trim().split(/\r?\n/)[0];
    const [name, utilization, memoryUsed, memoryTotal, temperature, powerDraw] = raw.split(",").map(value => value.trim());
    return {
      name,
      utilization_percent: Number(utilization),
      memory_used_mib: Number(memoryUsed),
      memory_total_mib: Number(memoryTotal),
      temperature_c: Number(temperature),
      power_draw_w: Number(powerDraw)
    };
  } catch (error) {
    return { available: false, error: String(error.message).slice(0, 200) };
  }
}

function collectChild(child) {
  return new Promise((resolve, reject) => {
    let stdout = "";
    let stderr = "";
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", chunk => stdout += chunk);
    child.stderr?.on("data", chunk => stderr += chunk);
    child.once("error", reject);
    child.once("close", code => resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() }));
  });
}

function firstLine(child, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(() => reject(new Error("policy helper did not report ready")), timeoutMs);
    child.stdout.setEncoding("utf8");
    const onData = chunk => {
      buffer += chunk;
      const index = buffer.indexOf("\n");
      if (index < 0) return;
      clearTimeout(timer);
      child.stdout.off("data", onData);
      resolve(buffer.slice(0, index).trim());
    };
    child.stdout.on("data", onData);
    child.once("error", error => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

function summarize(values) {
  if (!values.length) return { n: 0 };
  const sorted = [...values].sort((a, b) => a - b);
  const percentile = p => sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
  return {
    n: values.length,
    min: +Math.min(...values).toFixed(3),
    p50: +percentile(50).toFixed(3),
    p95: +percentile(95).toFixed(3),
    max: +Math.max(...values).toFixed(3),
    mean: +(values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3)
  };
}

async function runWindowProbe(exe, pid, durationMs) {
  const { stdout, stderr } = await execFileAsync(exe, [
    "--pid", String(pid),
    "--duration-ms", String(durationMs),
    "--interval-ms", "20",
    "--timeout-ms", "500"
  ], { windowsHide: true, encoding: "utf8" });
  if (stderr?.trim()) throw new Error(stderr.trim());
  return JSON.parse(stdout.trim());
}

async function runWorkerScenario({ name, policy, uefnPid, resourceExe, windowExe, durationMs, concurrency }) {
  const before = processMetrics(uefnPid);
  const gpuBefore = gpuSnapshot();

  const worker = spawn(node, [
    path.join(root, "bench", "resource-worker.mjs"),
    "--duration-ms", String(durationMs),
    "--concurrency", String(concurrency),
    "--buffer-bytes", String(2 * 1024 * 1024)
  ], {
    cwd: root,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"]
  });
  const workerDone = collectChild(worker);

  const policyArgs = [
    "--pid", String(worker.pid),
    "--priority", policy.priority,
    "--ecoqos", String(policy.ecoqos),
    "--memory", String(policy.memory ?? 0),
    "--job", policy.job ?? "none",
    "--job-value", String(policy.jobValue ?? 0)
  ];
  if ((policy.job ?? "none") !== "none") policyArgs.push("--hold-job");

  const helper = spawn(resourceExe, policyArgs, {
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const policyLine = await firstLine(helper);
  const appliedPolicy = JSON.parse(policyLine);
  const helperDone = collectChild(helper);

  const probePromise = runWindowProbe(windowExe, uefnPid, durationMs + 250);
  await sleep(100);
  worker.stdin.write("GO\n");
  worker.stdin.end();

  const workerResultRaw = await workerDone;
  if (workerResultRaw.code !== 0) throw new Error(`${name} worker failed: ${workerResultRaw.stderr}`);
  const workerResult = JSON.parse(workerResultRaw.stdout);

  if ((policy.job ?? "none") !== "none") {
    const helperResult = await helperDone;
    if (helperResult.code !== 0) throw new Error(`${name} policy helper failed: ${helperResult.stderr}`);
  }

  const probe = await probePromise;
  const after = processMetrics(uefnPid);
  const gpuAfter = gpuSnapshot();

  return {
    name,
    policy: appliedPolicy,
    worker: workerResult,
    uefn_probe: probe,
    uefn_process: {
      cpu_delta_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
      rss_before_bytes: before.rss_bytes,
      rss_after_bytes: after.rss_bytes,
      rss_delta_bytes: after.rss_bytes - before.rss_bytes,
      priority_before: before.priority,
      priority_after: after.priority
    },
    gpu: { before: gpuBefore, after: gpuAfter }
  };
}

async function runNoWorkerScenario({ name, uefnPid, windowExe, durationMs, decision = null }) {
  const before = processMetrics(uefnPid);
  const gpuBefore = gpuSnapshot();
  const probe = await runWindowProbe(windowExe, uefnPid, durationMs);
  const after = processMetrics(uefnPid);
  return {
    name,
    scheduling_decision: decision,
    worker: null,
    uefn_probe: probe,
    uefn_process: {
      cpu_delta_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
      rss_before_bytes: before.rss_bytes,
      rss_after_bytes: after.rss_bytes,
      rss_delta_bytes: after.rss_bytes - before.rss_bytes,
      priority_before: before.priority,
      priority_after: after.priority
    },
    gpu: { before: gpuBefore, after: gpuSnapshot() }
  };
}

async function waitForState(statePath, expectedPid, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (state.pid === expectedPid) return state;
    } catch {}
    await sleep(20);
  }
  throw new Error("host state did not become current");
}

async function inactiveProjectIdle(count) {
  const dir = await fs.mkdtemp(path.join(tempRoot, `inactive-${count}-`));
  const dbPath = path.join(dir, "relay.sqlite3");
  const storage = new RelayStorage(dbPath);
  for (let i = 0; i < count; i++) {
    storage.registerProject({
      id: `PRJ-idle-${String(i).padStart(4, "0")}`,
      name: `Inactive Fixture ${i}`,
      root_uri: `file:///relay-fixture/${i}`
    });
  }
  storage.close();

  const env = {
    ...process.env,
    RELAY_STATE_DIR: dir,
    RELAY_INSTANCE: `idle-${count}-${Date.now()}`
  };
  const child = spawn(node, [path.join(root, "src", "daemon.mjs")], {
    cwd: root,
    env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const startupStart = performance.now();
  await waitForState(path.join(dir, "host.json"), child.pid);
  const startupMs = performance.now() - startupStart;
  await sleep(300);
  const before = processMetrics(child.pid);
  await sleep(3000);
  const after = processMetrics(child.pid);
  child.kill("SIGKILL");
  if (child.exitCode === null) await new Promise(resolve => child.once("close", resolve));

  return {
    registered_projects: count,
    startup_to_state_ms: +startupMs.toFixed(3),
    idle_sample_ms: 3000,
    idle_cpu_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
    rss_bytes: after.rss_bytes,
    private_bytes: after.private_bytes,
    rss_delta_during_idle_bytes: after.rss_bytes - before.rss_bytes
  };
}

try {
  const resourceExe = path.join(tempRoot, "resource-policy.exe");
  const windowExe = path.join(tempRoot, "window-probe.exe");
  compileHelper(path.join(root, "windows", "resource-policy.cs"), resourceExe);
  compileHelper(path.join(root, "windows", "window-probe.cs"), windowExe);

  const creators = creatorSnapshot();
  const uefn = creators.find(item => item.ProcessName === "UnrealEditorFortnite-Win64-Shipping") ??
    creators.find(item => String(item.ProcessName).includes("UnrealEditor"));
  if (!uefn?.Id) throw new Error("UEFN is not running; Spike 5 real-editor coexistence benchmark requires an active UEFN process.");

  const resourceState = chooseResourceMode({ processNames: creators.map(item => item.ProcessName) });
  const localAiDecision = schedulingDecision({ kind: "local_ai", interruptible: true }, resourceState);
  const deepIndexDecision = schedulingDecision({ kind: "deep_index", interruptible: true }, resourceState);
  const durationMs = 3000;
  const concurrency = Math.max(2, Math.min(12, os.cpus().length - 2));

  const scenarios = [
    { name: "normal", policy: { priority: "normal", ecoqos: false, memory: 5, job: "none", jobValue: 0 } },
    { name: "below_normal", policy: { priority: "belownormal", ecoqos: false, memory: 4, job: "none", jobValue: 0 } },
    { name: "ecoqos_soft", policy: { priority: "belownormal", ecoqos: true, memory: 2, job: "none", jobValue: 0 } },
    { name: "job_weight_1", policy: { priority: "belownormal", ecoqos: true, memory: 2, job: "weight", jobValue: 1 } },
    { name: "job_hardcap_25", policy: { priority: "belownormal", ecoqos: true, memory: 2, job: "hardcap", jobValue: 2500 } }
  ];

  const runs = {};
  runs.baseline = [];
  for (let i = 0; i < 3; i++) {
    const run = await runNoWorkerScenario({ name: "baseline", uefnPid: uefn.Id, windowExe, durationMs: durationMs + 250 });
    runs.baseline.push(run);
    process.stderr.write(`baseline ${i + 1}/3 p95=${run.uefn_probe.p95_ms}ms\n`);
    await sleep(300);
  }

  for (const scenario of scenarios) {
    runs[scenario.name] = [];
    for (let i = 0; i < 3; i++) {
      const run = await runWorkerScenario({
        ...scenario,
        uefnPid: uefn.Id,
        resourceExe,
        windowExe,
        durationMs,
        concurrency
      });
      runs[scenario.name].push(run);
      process.stderr.write(
        `${scenario.name} ${i + 1}/3 p95=${run.uefn_probe.p95_ms}ms throughput=${run.worker.hashed_mib_per_second}MiB/s errors=${run.policy.errors || "none"}\n`
      );
      await sleep(400);
    }
  }

  runs.foreground_safe_deferred = [
    await runNoWorkerScenario({
      name: "foreground_safe_deferred",
      uefnPid: uefn.Id,
      windowExe,
      durationMs: durationMs + 250,
      decision: localAiDecision
    })
  ];

  const saturationDurationMs = 2500;
  const saturationConcurrency = os.cpus().length;
  const saturationRuns = { baseline: [] };
  for (let i = 0; i < 2; i++) {
    saturationRuns.baseline.push(
      await runNoWorkerScenario({
        name: "saturation_baseline",
        uefnPid: uefn.Id,
        windowExe,
        durationMs: saturationDurationMs + 250
      })
    );
  }
  const saturationScenarios = [
    { name: "normal", policy: { priority: "normal", ecoqos: false, memory: 5, job: "none", jobValue: 0 } },
    { name: "ecoqos_soft", policy: { priority: "belownormal", ecoqos: true, memory: 2, job: "none", jobValue: 0 } },
    { name: "job_hardcap_25", policy: { priority: "belownormal", ecoqos: true, memory: 2, job: "hardcap", jobValue: 2500 } }
  ];
  for (const scenario of saturationScenarios) {
    saturationRuns[scenario.name] = [];
    for (let i = 0; i < 2; i++) {
      const run = await runWorkerScenario({
        ...scenario,
        uefnPid: uefn.Id,
        resourceExe,
        windowExe,
        durationMs: saturationDurationMs,
        concurrency: saturationConcurrency
      });
      saturationRuns[scenario.name].push(run);
      process.stderr.write(
        `saturation ${scenario.name} ${i + 1}/2 p95=${run.uefn_probe.p95_ms}ms throughput=${run.worker.hashed_mib_per_second}MiB/s\n`
      );
      await sleep(400);
    }
  }

  const scenarioSummary = {};
  const baselineP95Median = summarize(runs.baseline.map(run => run.uefn_probe.p95_ms)).p50;
  for (const [name, scenarioRuns] of Object.entries(runs)) {
    const probeP50 = scenarioRuns.map(run => run.uefn_probe.p50_ms);
    const probeP95 = scenarioRuns.map(run => run.uefn_probe.p95_ms);
    const probeP99 = scenarioRuns.map(run => run.uefn_probe.p99_ms);
    const timeouts = scenarioRuns.map(run => run.uefn_probe.timeouts);
    const throughput = scenarioRuns.filter(run => run.worker).map(run => run.worker.hashed_mib_per_second);
    scenarioSummary[name] = {
      uefn_message_p50_ms: summarize(probeP50),
      uefn_message_p95_ms: summarize(probeP95),
      uefn_message_p99_ms: summarize(probeP99),
      uefn_timeouts: { total: timeouts.reduce((a, b) => a + b, 0), per_run: timeouts },
      worker_hashed_mib_per_second: summarize(throughput),
      median_p95_ratio_vs_baseline: baselineP95Median
        ? +(summarize(probeP95).p50 / baselineP95Median).toFixed(4)
        : null
    };
  }

  const saturationSummary = {};
  const saturationBaselineP95Median = summarize(
    saturationRuns.baseline.map(run => run.uefn_probe.p95_ms)
  ).p50;
  for (const [name, scenarioRuns] of Object.entries(saturationRuns)) {
    const probeP95 = scenarioRuns.map(run => run.uefn_probe.p95_ms);
    const probeP99 = scenarioRuns.map(run => run.uefn_probe.p99_ms);
    const throughput = scenarioRuns.filter(run => run.worker).map(run => run.worker.hashed_mib_per_second);
    const timeouts = scenarioRuns.map(run => run.uefn_probe.timeouts);
    saturationSummary[name] = {
      uefn_message_p95_ms: summarize(probeP95),
      uefn_message_p99_ms: summarize(probeP99),
      uefn_timeouts: { total: timeouts.reduce((a, b) => a + b, 0), per_run: timeouts },
      worker_hashed_mib_per_second: summarize(throughput),
      median_p95_ratio_vs_saturation_baseline: saturationBaselineP95Median
        ? +(summarize(probeP95).p50 / saturationBaselineP95Median).toFixed(4)
        : null
    };
  }

  const inactiveProjects = {
    one_project: await inactiveProjectIdle(1),
    one_hundred_projects: await inactiveProjectIdle(100)
  };

  const result = {
    benchmark: "phase1-spike5-resource-coexistence-windows",
    recorded_at: new Date().toISOString(),
    decision_informed: "Which Windows-native scheduling controls and RELAY backoff policy best protect creator responsiveness while preserving useful background throughput.",
    hypothesis: "Foreground-safe deferral plus soft OS scheduling hints should protect UEFN responsiveness with less pathological background latency than hard CPU caps; inactive projects should add negligible active compute.",
    runtime: {
      os: { platform: process.platform, release: os.release(), arch: os.arch() },
      node: process.version,
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem(),
      creators_detected: creators,
      uefn_target: {
        pid: uefn.Id,
        process_name: uefn.ProcessName,
        window_title: uefn.MainWindowTitle,
        working_set_bytes_at_start: uefn.WorkingSet64
      },
      gpu_at_start: gpuSnapshot()
    },
    policy: {
      detected_mode: resourceState,
      local_ai_decision: localAiDecision,
      deep_index_decision: deepIndexDecision,
      worker_concurrency: concurrency,
      worker_duration_ms: durationMs,
      saturation_worker_concurrency: saturationConcurrency,
      saturation_worker_duration_ms: saturationDurationMs,
      probe: "SendMessageTimeout(WM_NULL) against UEFN main window; no editor clicks or project mutations"
    },
    scenarios: scenarioSummary,
    saturation: saturationSummary,
    raw_runs: runs,
    raw_saturation_runs: saturationRuns,
    inactive_project_idle: inactiveProjects,
    pass_fail: {
      real_uefn_process_measured: Boolean(uefn.Id),
      no_uefn_probe_timeouts: Object.values(runs).flat().every(run => run.uefn_probe.timeouts === 0),
      no_saturation_probe_timeouts: Object.values(saturationRuns).flat().every(run => run.uefn_probe.timeouts === 0),
      below_normal_applied: runs.below_normal.every(run => run.policy.priority_value === 0x4000),
      ecoqos_applied: runs.ecoqos_soft.every(run => run.policy.ecoqos_state_mask === 1),
      job_weight_applied: runs.job_weight_1.every(run => run.policy.job_flags === 3 && run.policy.job_value === 1),
      hardcap_applied: runs.job_hardcap_25.every(run => run.policy.job_flags === 5 && run.policy.job_value === 2500),
      local_ai_deferred_with_uefn_active: localAiDecision.action === "defer",
      deep_index_deferred_with_uefn_active: deepIndexDecision.action === "defer",
      inactive_project_idle_cpu_near_zero: inactiveProjects.one_hundred_projects.idle_cpu_ms <= 20
    },
    limitations: [
      "UEFN was open and its real main-window message pump was measured, but no Fortnite play session was active; this is not a frame-time benchmark.",
      "WM_NULL round-trip latency measures editor message-pump responsiveness, not mouse/input-to-render latency.",
      "The synthetic RELAY worker stresses CPU hashing/compression and does not represent a GPU local-LLM workload.",
      "Memory priority was verified as applied but this workstation was not placed under artificial memory pressure, so trimming behavior is not performance-approved.",
      "Power/thermal readings are limited to available NVIDIA GPU telemetry; CPU package energy was not directly measured.",
      "One high-end workstation cannot establish public resource budgets; minimum/recommended hardware fixtures remain required."
    ]
  };

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(resultPath, JSON.stringify(result, null, 2) + "\n", "utf8");
  console.log(JSON.stringify(result, null, 2));
} finally {
  await fs.rm(tempRoot, { recursive: true, force: true });
}
