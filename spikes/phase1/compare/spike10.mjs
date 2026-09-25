import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const rustRoot = path.join(phase1, "rust");
const resultPath = path.join(
  phase1,
  "results",
  "2026-09-24-spike10-adapter-broker-windows.json"
);
const spike9Path = path.join(
  phase1,
  "results",
  "2026-09-24-spike9-command-registry-windows.json"
);
const probeExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-adapter-probe.exe"
);
const workerExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-synthetic-adapter.exe"
);
const coreExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-rust-challenger.exe"
);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function runSync(exe, args, options = {}) {
  const result = spawnSync(exe, args, {
    cwd: rustRoot,
    encoding: "utf8",
    windowsHide: true,
    ...options
  });
  if (result.status !== 0) {
    throw new Error(
      exe + " " + args.join(" ") + " failed: " +
      (result.stderr || result.stdout)
    );
  }
  return result;
}

function processMetrics(pid) {
  const command =
    "$p=Get-Process -Id " + pid + " -ErrorAction Stop; " +
    "[pscustomobject]@{" +
    "cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;" +
    "rss_bytes=[int64]$p.WorkingSet64;" +
    "private_bytes=[int64]$p.PrivateMemorySize64;" +
    "handle_count=$p.HandleCount;" +
    "thread_count=$p.Threads.Count" +
    "} | ConvertTo-Json -Compress";
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

function childProcessSnapshot(pid) {
  const command =
    "$items=Get-CimInstance Win32_Process -Filter \"ParentProcessId = " + pid + "\"; " +
    "[pscustomobject]@{" +
    "count=@($items).Count;" +
    "adapter_worker_count=@($items | Where-Object {$_.Name -eq 'relay-synthetic-adapter.exe'}).Count;" +
    "names=@($items | Select-Object -ExpandProperty Name)" +
    "} | ConvertTo-Json -Compress";
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

async function firstJsonLine(child, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(
      () => reject(new Error("probe did not report ready")),
      timeoutMs
    );
    child.stdout.setEncoding("utf8");
    const onData = chunk => {
      buffer += chunk;
      const index = buffer.indexOf("\n");
      if (index < 0) return;
      clearTimeout(timer);
      child.stdout.off("data", onData);
      try {
        resolve(JSON.parse(buffer.slice(0, index).trim()));
      } catch (error) {
        reject(error);
      }
    };
    child.stdout.on("data", onData);
    child.once("error", error => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("close", code => {
      if (code !== 0) {
        clearTimeout(timer);
        reject(new Error("probe exited before ready: " + code));
      }
    });
  });
}

async function idleEvidence(count) {
  const child = spawn(
    probeExe,
    ["idle", workerExe, String(count), "9000"],
    {
      cwd: rustRoot,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    }
  );
  const ready = await firstJsonLine(child);
  await sleep(500);
  const before = processMetrics(child.pid);
  const childrenAtIdle = childProcessSnapshot(child.pid);
  await sleep(5000);
  const after = processMetrics(child.pid);
  const closing = new Promise(resolve => child.once("close", resolve));
  if (child.exitCode === null) await closing;

  return {
    installed_adapters: ready.installed_adapters,
    install_ms: +ready.install_ms.toFixed(3),
    sample_ms: 5000,
    cpu_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
    rss_bytes: after.rss_bytes,
    private_bytes: after.private_bytes,
    rss_delta_bytes: after.rss_bytes - before.rss_bytes,
    handle_count: after.handle_count,
    thread_count: after.thread_count,
    child_processes_while_idle: childrenAtIdle.count,
    adapter_worker_processes_while_idle: childrenAtIdle.adapter_worker_count,
    child_process_names_while_idle: childrenAtIdle.names
  };
}

function cleanReleaseBuild() {
  execFileSync("cargo", ["clean", "--release"], {
    cwd: rustRoot,
    windowsHide: true,
    stdio: "ignore"
  });
  const started = performance.now();
  execFileSync("cargo", ["build", "--release", "--bins"], {
    cwd: rustRoot,
    windowsHide: true,
    stdio: "ignore"
  });
  return performance.now() - started;
}

function cargoMetadata() {
  return JSON.parse(execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--locked"],
    { cwd: rustRoot, encoding: "utf8", windowsHide: true }
  ));
}

async function sourceMetrics() {
  const files = [
    path.join(rustRoot, "src", "adapter.rs"),
    path.join(rustRoot, "src", "bin", "relay-synthetic-adapter.rs")
  ];
  let lines = 0;
  let bytes = 0;
  let unsafeMentions = 0;
  for (const file of files) {
    const text = await fs.readFile(file, "utf8");
    lines += text.split(/\r?\n/).length;
    bytes += Buffer.byteLength(text);
    unsafeMentions += (text.match(/\bunsafe\b/g) ?? []).length;
  }
  return { files: files.length, lines, bytes, unsafe_mentions: unsafeMentions };
}

function runBrokerBench(iterations = 80) {
  const result = runSync(
    probeExe,
    ["bench", workerExe, String(iterations)],
    { timeout: 30000 }
  );
  return JSON.parse(result.stdout.trim());
}


const spike9 = JSON.parse(await fs.readFile(spike9Path, "utf8"));
const buildMs = cleanReleaseBuild();
const metadata = cargoMetadata();
const rootPackage = metadata.packages.find(
  pkg => pkg.name === "relay-rust-challenger"
);
const directDependencyCount = rootPackage?.dependencies.length ?? null;

const integration = spawnSync(
  "cargo",
  ["test", "--test", "spike10", "--", "--nocapture"],
  {
    cwd: rustRoot,
    windowsHide: true,
    encoding: "utf8",
    timeout: 30000
  }
);
if (integration.status !== 0) {
  throw new Error("Spike 10 integration tests failed: " + integration.stderr);
}

const bench = runBrokerBench(80);
const idle0 = await idleEvidence(0);
const idle100 = await idleEvidence(100);
const source = await sourceMetrics();

const coreBytes = (await fs.stat(coreExe)).size;
const workerBytes = (await fs.stat(workerExe)).size;
const probeBytes = (await fs.stat(probeExe)).size;
const spike9Build = spike9.build_economics.spike9_registry_enabled;

const idleRssDelta100 = idle100.rss_bytes - idle0.rss_bytes;

const result = {
  benchmark: "phase1-spike10-adapter-broker-windows",
  recorded_at: new Date().toISOString(),
  decision_informed:
    "Which synthetic adapter-worker isolation, manifest, broker, lifecycle, and resource-containment foundation should RELAY carry forward before real tool adapters are implemented.",
  hypothesis:
    "A manifest-validated out-of-process worker launched on demand and contained by a Windows Job Object can provide fault/lifecycle/resource isolation with near-zero inactive cost, while keeping command semantics in RELAY's trusted registry and leaving stronger filesystem/network sandboxing as a separate security gate.",

  runtime: {
    os: {
      platform: process.platform,
      release: os.release(),
      arch: os.arch()
    },
    rustc: execFileSync(
      "rustc",
      ["--version"],
      { encoding: "utf8", windowsHide: true }
    ).trim(),
    cargo: execFileSync(
      "cargo",
      ["--version"],
      { encoding: "utf8", windowsHide: true }
    ).trim(),
    cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
    logical_cpu_count: os.cpus().length,
    total_memory_bytes: os.totalmem()
  },

  selected_foundation: {
    adapter_manifest_format: 1,
    adapter_protocol_min: 1,
    adapter_protocol_max: 1,
    launch_model: "on-demand out-of-process worker per invocation",
    command_semantics: "trusted RELAY command registry",
    artifact_integrity: "SHA-256 verified before launch",
    resource_containment: {
      mechanism: "Windows Job Object",
      kill_on_broker_close: true,
      active_process_limit: 1,
      process_memory_limit_bytes: 33554432
    },
    broker_policy_defaults_for_fixture: {
      network: false,
      subprocess: false,
      credentials: [],
      project_read: [],
      project_write: [],
      external_apps: []
    }
  },

  measurements: {
    manifest_validation: bench.manifest_validation,
    normal_invocation: bench.normal_invocation,
    out_of_process: bench.out_of_process,
    job_limits: bench.job_limits,
    stderr_marked_untrusted: bench.stderr_marked_untrusted,
    memory_reservation_succeeded: bench.memory_reservation_succeeded,
    crash: bench.crash,
    invalid_message_error_code: bench.invalid_message_error_code,
    hang: bench.hang,
    inactive_zero_adapters: idle0,
    inactive_one_hundred_adapters: idle100,
    inactive_100_minus_0_rss_bytes: idleRssDelta100
  },

  build_and_complexity: {
    spike9_core_baseline: {
      direct_dependency_count: spike9Build.direct_dependency_count,
      resolved_package_count: spike9Build.resolved_package_count,
      release_binary_bytes: spike9Build.release_binary_bytes
    },
    spike10: {
      clean_release_all_bins_ms: +buildMs.toFixed(3),
      direct_dependency_count: directDependencyCount,
      resolved_package_count: metadata.packages.length,
      core_release_binary_bytes: coreBytes,
      synthetic_worker_binary_bytes: workerBytes,
      benchmark_probe_binary_bytes: probeBytes,
      source_metrics: source
    },
    deltas_vs_spike9_core: {
      direct_dependencies:
        directDependencyCount - spike9Build.direct_dependency_count,
      resolved_packages:
        metadata.packages.length - spike9Build.resolved_package_count,
      core_release_binary_bytes:
        coreBytes - spike9Build.release_binary_bytes
    }
  },

  failure_evidence: {
    integration_tests_exit_code: integration.status,
    integration_test_count: 8,
    crash_error_code: bench.crash.error_code,
    crash_elapsed_ms: +bench.crash.elapsed_ms.toFixed(3),
    invalid_message_error_code: bench.invalid_message_error_code,
    hang_error_code: bench.hang.error_code,
    hang_elapsed_ms: +bench.hang.elapsed_ms.toFixed(3),
    quarantine_backoff_tested: true,
    over_permission_manifest_tested: true,
    incompatible_manifest_tested: true,
    bad_digest_manifest_tested: true,
    bad_result_schema_tested: true,
    undeclared_worker_error_tested: true
  },

  pass_fail: {
    integration_tests_pass: integration.status === 0,
    worker_is_out_of_process: bench.out_of_process === true,
    job_active_process_limit_verified:
      bench.job_limits.active_process_limit === 1,
    job_memory_limit_verified:
      bench.job_limits.process_memory_limit_bytes === 33554432,
    job_kill_on_close_verified:
      bench.job_limits.kill_on_close === true,
    large_memory_reservation_blocked:
      bench.memory_reservation_succeeded === false,
    hostile_stderr_remains_untrusted:
      bench.stderr_marked_untrusted === true,
    crash_is_contained:
      bench.crash.error_code === "ADAPTER_WORKER_EXITED",
    invalid_message_is_contained:
      bench.invalid_message_error_code === "ADAPTER_INVALID_MESSAGE",
    hang_times_out:
      bench.hang.error_code === "ADAPTER_TIMEOUT" &&
      bench.hang.elapsed_ms < 1000,
    zero_inactive_workers_for_zero_installed:
      idle0.adapter_worker_processes_while_idle === 0,
    zero_inactive_workers_for_one_hundred_installed:
      idle100.adapter_worker_processes_while_idle === 0,
    inactive_zero_adapters_cpu_near_zero:
      idle0.cpu_ms <= 20,
    inactive_one_hundred_adapters_cpu_near_zero:
      idle100.cpu_ms <= 20,
    hundred_adapter_rss_delta_bounded:
      idleRssDelta100 <= 4 * 1024 * 1024,
    manifest_validation_p50_under_2ms:
      bench.manifest_validation.p50_ms < 2,
    invocation_p50_under_20ms:
      bench.normal_invocation.p50_ms < 20,
    no_new_direct_runtime_dependency:
      directDependencyCount === spike9Build.direct_dependency_count,
    no_new_resolved_runtime_package:
      metadata.packages.length === spike9Build.resolved_package_count
  },

  limitations: [
    "Windows Job Objects provide process lifecycle/resource containment, not a complete security sandbox. The worker still runs under the signed-in user's token and therefore stronger filesystem, registry, IPC, and network restrictions remain a separate gate.",
    "The fixture policy denies network, credentials, subprocess, project, and external-app permissions at the broker/manifest layer, but this spike does not yet provide OS-enforced network or filesystem denial for a malicious worker.",
    "The active-process Job Object limit is queried back from Windows, but the synthetic fixture does not execute a child-process escape attempt.",
    "The 32 MiB process-memory cap and 400 ms default timeout are synthetic spike values, not final product defaults.",
    "The selected launch model starts a fresh worker per invocation. Persistent worker pooling may be benchmarked later for integrations where startup cost materially affects throughput.",
    "All behavior is synthetic. No UEFN, Fortnite, Blender, Krita, or other real adapter product behavior is implemented or inferred from this result.",
    "Only one high-end Windows workstation was benchmarked."
  ]
};

const failed = Object.entries(result.pass_fail)
  .filter(([, value]) => !value)
  .map(([key]) => key);
if (failed.length) {
  throw new Error("Spike 10 gate failed: " + failed.join(", "));
}

await fs.mkdir(path.dirname(resultPath), { recursive: true });
await fs.writeFile(
  resultPath,
  JSON.stringify(result, null, 2) + "\n",
  "utf8"
);
console.log(JSON.stringify(result, null, 2));
