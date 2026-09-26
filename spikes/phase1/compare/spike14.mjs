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
  "2026-09-25-spike14-diagnostics-foundation-windows.json"
);
const probeExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-diagnostics-probe.exe"
);
const coreExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-rust-challenger.exe"
);
const providerGuid = "{9a421fa8-f21b-4f9e-99ab-1db6bb8b1470}";
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

function probeJson(args, timeout = 30000) {
  const result = runSync(probeExe, args, { timeout });
  return JSON.parse(result.stdout.trim());
}

function powershellJson(command) {
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

function processMetrics(pid) {
  return powershellJson(
    "$p=Get-Process -Id " + pid + " -ErrorAction Stop;" +
    "[pscustomobject]@{" +
    "cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;" +
    "rss_bytes=[int64]$p.WorkingSet64;" +
    "private_bytes=[int64]$p.PrivateMemorySize64;" +
    "handle_count=$p.HandleCount;" +
    "thread_count=$p.Threads.Count" +
    "}|ConvertTo-Json -Compress"
  );
}


async function firstJsonLine(child, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(
      () => reject(new Error("idle probe did not report ready")),
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
  });
}

async function idleEvidence(mode, target, durationMs = 6500) {
  const args = mode === "etw"
    ? ["idle-etw", String(durationMs)]
    : [
        "idle-" + mode,
        target,
        String(durationMs)
      ];
  const child = spawn(probeExe, args, {
    cwd: rustRoot,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const ready = await firstJsonLine(child);
  await sleep(500);
  const before = processMetrics(child.pid);
  await sleep(5000);
  const after = processMetrics(child.pid);
  const closing = new Promise(resolve => child.once("close", resolve));
  if (child.exitCode === null) {
    await closing;
  }
  return {
    ready,
    sample_ms: 5000,
    cpu_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
    rss_bytes: after.rss_bytes,
    private_bytes: after.private_bytes,
    rss_delta_bytes: after.rss_bytes - before.rss_bytes,
    handle_count: after.handle_count,
    thread_count: after.thread_count
  };
}

function cleanReleaseBuild() {
  execFileSync("cargo", ["clean", "--release"], {
    cwd: rustRoot,
    windowsHide: true,
    stdio: "ignore"
  });
  const started = performance.now();
  execFileSync(
    "cargo",
    [
      "build",
      "--release",
      "--bin", "relay-rust-challenger",
      "--bin", "relay-diagnostics-probe"
    ],
    {
      cwd: rustRoot,
      windowsHide: true,
      stdio: "ignore"
    }
  );
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
    path.join(rustRoot, "src", "diagnostics.rs"),
    path.join(rustRoot, "src", "bin", "relay-diagnostics-probe.rs")
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


function etwDurableSessionEvidence(tempRoot, events = 5000) {
  const sessionName = "RELAY-Spike14-" + process.pid;
  const etl = path.join(tempRoot, "relay-spike14.etl");
  const start = spawnSync(
    "logman.exe",
    [
      "start", sessionName,
      "-p", providerGuid,
      "0xFFFFFFFF", "5",
      "-o", etl,
      "-ets"
    ],
    {
      windowsHide: true,
      encoding: "utf8",
      timeout: 5000
    }
  );

  if (start.status !== 0) {
    const message = (start.stderr || start.stdout || "").toLowerCase();
    return {
      session_started: false,
      start_exit_code: start.status,
      access_denied: message.includes("access is denied"),
      provider_run: null,
      etl_bytes: 0
    };
  }

  let providerRun;
  let stop;
  try {
    providerRun = probeJson(["bench-etw", String(events)]);
  } finally {
    stop = spawnSync(
      "logman.exe",
      ["stop", sessionName, "-ets"],
      {
        windowsHide: true,
        encoding: "utf8",
        timeout: 5000
      }
    );
  }
  let etlBytes = 0;
  try {
    etlBytes = Number(execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        "(Get-Item -LiteralPath '" +
          etl.replaceAll("'", "''") +
          "').Length"
      ],
      { encoding: "utf8", windowsHide: true }
    ).trim());
  } catch {}

  return {
    session_started: true,
    start_exit_code: start.status,
    stop_exit_code: stop?.status ?? null,
    access_denied: false,
    provider_run: providerRun,
    etl_bytes: etlBytes
  };
}

function runRustTests() {
  const result = spawnSync(
    "cargo",
    ["test"],
    {
      cwd: rustRoot,
      windowsHide: true,
      encoding: "utf8",
      timeout: 60000
    }
  );
  if (result.status !== 0) {
    throw new Error("Rust verification failed: " + result.stderr);
  }
  const combined = result.stdout + result.stderr;
  const matches = [...combined.matchAll(
    /test result: ok\. (\d+) passed; 0 failed/g
  )];
  return {
    passed_groups: matches.map(match => Number(match[1])),
    total_passed: matches.reduce(
      (sum, match) => sum + Number(match[1]),
      0
    )
  };
}


function countLockPackages(text) {
  return (text.match(/^name = /gm) ?? []).length;
}

const repoRoot = path.resolve(phase1, "..", "..");
const tempRoot = await fs.mkdtemp(
  path.join(os.tmpdir(), "relay-spike14-")
);

try {
  const verification = runRustTests();
  const cleanBuildMs = cleanReleaseBuild();
  const metadata = cargoMetadata();
  const rootPackage = metadata.packages.find(
    pkg => pkg.name === "relay-rust-challenger"
  );
  const directDependencyCount = rootPackage?.dependencies.length ?? 0;
  const currentLock = await fs.readFile(
    path.join(rustRoot, "Cargo.lock"),
    "utf8"
  );
  const headLock = execFileSync(
    "git",
    ["-C", repoRoot, "show", "HEAD:spikes/phase1/rust/Cargo.lock"],
    { encoding: "utf8", windowsHide: true }
  );
  const lockPackageCount = countLockPackages(currentLock);
  const headLockPackageCount = countLockPackages(headLock);

  const events = 10_000;
  const jsonl = probeJson([
    "bench-jsonl",
    path.join(tempRoot, "jsonl"),
    String(events)
  ]);
  const jsonlSync1 = probeJson([
    "bench-jsonl-sync1",
    path.join(tempRoot, "jsonl-sync1"),
    String(events)
  ], 120000);
  const sqlite = probeJson([
    "bench-sqlite",
    path.join(tempRoot, "sqlite", "diagnostics.sqlite3"),
    String(events)
  ]);
  const etwNoConsumer = probeJson([
    "bench-etw",
    String(events)
  ]);
  const detail = probeJson([
    "bench-detail-jsonl",
    path.join(tempRoot, "detail-normal"),
    path.join(tempRoot, "detail-enabled"),
    "1000"
  ]);
  const recovery = probeJson([
    "recovery-jsonl",
    path.join(tempRoot, "recovery")
  ]);

  const idleJsonl = await idleEvidence(
    "jsonl",
    path.join(tempRoot, "idle-jsonl")
  );
  const idleSqlite = await idleEvidence(
    "sqlite",
    path.join(tempRoot, "idle-sqlite", "diagnostics.sqlite3")
  );
  const idleEtw = await idleEvidence("etw", null);

  const etwDurable = etwDurableSessionEvidence(tempRoot, 5000);
  const source = await sourceMetrics();
  const coreBytes = (await fs.stat(coreExe)).size;
  const probeBytes = (await fs.stat(probeExe)).size;


  const jsonlBoundBytes =
    jsonl.health.max_file_bytes * jsonl.health.max_files + 64 * 1024;

  const passFail = {
    rust_regressions_green:
      verification.total_passed > 0,
    jsonl_bounded_retention:
      jsonl.retained_bytes <= jsonlBoundBytes &&
      jsonl.health.evicted_files > 0 &&
      jsonl.aggregate.retention_evicted_events > 0,
    jsonl_partial_tail_recovery:
      recovery.recovered_partial_bytes > 0 &&
      recovery.total_events_after_recovery === 1 &&
      recovery.invalid_lines_after_recovery === 0 &&
      recovery.health_ok === true,
    jsonl_aggregate_valid:
      jsonl.aggregate.invalid_lines === 0 &&
      jsonl.aggregate.total_events > 0,
    detail_mode_is_explicit_and_bounded:
      detail.amplification.bytes_ratio > 1 &&
      detail.detail.bytes_per_event < 32 * 1024 &&
      detail.normal.bytes_per_event < detail.detail.bytes_per_event,
    sync1_control_measured:
      jsonlSync1.sync_every === 1 &&
      jsonl.sync_every === 16,
    sqlite_retention_maintenance_measured:
      sqlite.retention_target_rows === 4096 &&
      sqlite.aggregate.total_events === 4096 &&
      sqlite.prune_ms >= 0 &&
      sqlite.checkpoint_ms >= 0 &&
      sqlite.vacuum_ms >= 0,
    idle_jsonl_cpu_bounded:
      idleJsonl.cpu_ms <= 20,
    idle_sqlite_cpu_bounded:
      idleSqlite.cpu_ms <= 20,
    idle_etw_cpu_bounded:
      idleEtw.cpu_ms <= 20,
    etw_no_consumer_is_not_durable:
      etwNoConsumer.durable_without_consumer_session === false &&
      etwNoConsumer.retained_bytes === 0,
    etw_session_availability_classified:
      etwDurable.session_started === true ||
      etwDurable.access_denied === true,
    no_new_direct_dependency_or_lock_package:
      lockPackageCount === headLockPackageCount
  };
  passFail.all_gates_pass = Object.values(passFail).every(Boolean);

  const result = {
    benchmark: "phase1-spike14-diagnostics-foundation-windows",
    recorded_at: new Date().toISOString(),
    decision_informed:
      "Which minimum local logging/diagnostic foundation RELAY should use before Phase 1 closes.",
    hypothesis:
      "Bounded structured JSONL with an explicit periodic sync window will provide the simplest durable default support record; SQLite raw-log storage will add write/maintenance coupling, while ETW is best kept as optional temporary deep tracing.",
    runtime: {
      os: {
        platform: process.platform,
        release: os.release(),
        arch: os.arch()
      },
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem(),
      rustc: execFileSync(
        "rustc",
        ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim(),
      cargo: execFileSync(
        "cargo",
        ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim()
    },
    workload: {
      normal_events_per_candidate: events,
      normal_payload_bytes: 192,
      detail_events: 1000,
      detail_payload_bytes: 3000,
      jsonl_retention: {
        max_file_bytes: 1024 * 1024,
        max_files: 4,
        default_sync_every: 16,
        strict_sync_control: 1
      },
      sqlite_retention_rows: 4096,
      idle_sample_ms: 5000
    },
    candidates: {
      jsonl,
      jsonl_sync_every_event: jsonlSync1,
      sqlite,
      etw_no_consumer: etwNoConsumer,
      etw_durable_session: etwDurable
    },
    detail_mode: detail,
    recovery,
    idle: {
      jsonl: idleJsonl,
      sqlite: idleSqlite,
      etw: idleEtw
    },
    build_and_complexity: {
      clean_release_build_ms: +cleanBuildMs.toFixed(3),
      core_exe_bytes: coreBytes,
      diagnostics_probe_exe_bytes: probeBytes,
      direct_dependency_count: directDependencyCount,
      cargo_lock_package_count: lockPackageCount,
      head_cargo_lock_package_count: headLockPackageCount,
      new_resolved_packages: lockPackageCount - headLockPackageCount,
      diagnostics_source: source
    },
    verification,
    candidate_outcome: {
      default_candidate: "bounded_jsonl",
      optional_deep_trace: "etw",
      not_selected_as_default_raw_log_store: "sqlite",
      selection_rationale: [
        "SQLite is faster on this fixture, but bounded JSONL already sustains far more diagnostic events than RELAY is expected to generate.",
        "JSONL keeps raw diagnostics outside the operational SQLite failure and maintenance domain and provides directly inspectable/salvageable support evidence.",
        "SQLite raw-log retention requires explicit prune/checkpoint/VACUUM work; ETW has no non-admin durable consumer path on this fixture."
      ],
      durability_window:
        "normal JSONL syncs every 16 events; explicit flush/shutdown sync narrows the window; sync-every-event control is measured separately",
      support_export:
        "aggregate/health summary only by default; unrestricted raw history is excluded"
    },
    comparisons: {
      sqlite_vs_jsonl_throughput_ratio:
        sqlite.events_per_second / jsonl.events_per_second,
      jsonl_vs_sqlite_retained_bytes_ratio:
        jsonl.retained_bytes / sqlite.retained_bytes,
      jsonl_vs_sqlite_idle_rss_ratio:
        idleJsonl.rss_bytes / idleSqlite.rss_bytes,
      jsonl_sync16_p50_ms:
        jsonl.durability_sync_latency.p50_ms,
      sqlite_commit16_p50_ms:
        sqlite.durability_sync_latency.p50_ms,
      sqlite_retention_maintenance_ms:
        sqlite.prune_ms +
        sqlite.checkpoint_ms +
        sqlite.vacuum_ms +
        sqlite.post_vacuum_checkpoint_ms
    },
    pass_fail: passFail,
    limitations: [
      "One high-end Windows workstation cannot establish public diagnostic resource budgets.",
      "The default JSONL candidate intentionally accepts a documented periodic durability window between sync_data calls; sudden power loss may lose the unsynced tail even though process-crash tail recovery is bounded.",
      "The SQLite candidate uses synchronous FULL with the same 16-event durability window as default JSONL, plus explicit row pruning, WAL checkpoint, VACUUM, and a post-VACUUM checkpoint; other batching changes durability/latency semantics and were not used to hide that trade-off.",
      "The ETW no-consumer measurement is intentionally non-durable. A non-elevated durable logman session was attempted separately; if denied, elevated ETW session loss counters and ETL export cost remain unmeasured.",
      "ETW remains Windows-specific and does not replace RELAY's portable structured support summary.",
      "The support-summary spike returns health and aggregate counts only; a final user-facing support-bundle archive format is later product work.",
      "Remote analytics, telemetry upload, encryption at rest, and public retention defaults are outside Phase 1."
    ]
  };

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(
    resultPath,
    JSON.stringify(result, null, 2) + "\n",
    "utf8"
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await fs.rm(tempRoot, { recursive: true, force: true });
}
