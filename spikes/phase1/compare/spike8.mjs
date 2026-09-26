import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { execFileSync, spawn } from "node:child_process";
import { performance } from "node:perf_hooks";
import { callHostWithState } from "../node/src/client.mjs";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const nodeRoot = path.join(phase1, "node");
const rustRoot = path.join(phase1, "rust");
const nodeHost = path.join(nodeRoot, "src", "daemon.mjs");
const rustExe = path.join(rustRoot, "target", "release", "relay-rust-challenger.exe");
const resultPath = path.join(
  phase1,
  "results",
  "2026-09-24-spike8-rust-storage-parity-windows.json"
);
const spike7Path = path.join(
  phase1,
  "results",
  "2026-09-24-spike7-runtime-ipc-challenger-windows.json"
);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const liveChildren = new Set();

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(
    sorted.length - 1,
    Math.ceil((p / 100) * sorted.length) - 1
  )];
}

function summarize(values) {
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

async function fileSize(file) {
  try {
    return (await fs.stat(file)).size;
  } catch {
    return 0;
  }
}

function processMetrics(pid) {
  const command =
    "$p=Get-Process -Id " + pid + " -ErrorAction Stop; " +
    "[pscustomobject]@{" +
    "rss_bytes=[int64]$p.WorkingSet64;" +
    "private_bytes=[int64]$p.PrivateMemorySize64;" +
    "cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;" +
    "handle_count=$p.HandleCount;thread_count=$p.Threads.Count" +
    "} | ConvertTo-Json -Compress";
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

async function waitState(statePath, expectedPid, timeoutMs = 5000) {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (state.pid === expectedPid) {
        return { state, elapsed_ms: performance.now() - started };
      }
    } catch {}
    await sleep(5);
  }
  throw new Error("state did not become current for PID " + expectedPid);
}

function runtimeDef(kind, stateDir, instance) {
  const env = {
    ...process.env,
    RELAY_STATE_DIR: stateDir,
    RELAY_INSTANCE: instance
  };

  if (kind === "node") {
    return {
      kind,
      env,
      stateDir,
      statePath: path.join(stateDir, "host.json"),
      command: process.execPath,
      args: [nodeHost],
      cwd: nodeRoot
    };
  }

  return {
    kind,
    env,
    stateDir,
    statePath: path.join(stateDir, "host-rust.json"),
    command: rustExe,
    args: ["host"],
    cwd: rustRoot
  };
}

async function startRuntime(def) {
  await fs.mkdir(def.stateDir, { recursive: true });
  const child = spawn(def.command, def.args, {
    cwd: def.cwd,
    env: def.env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  liveChildren.add(child);
  child.once("close", () => liveChildren.delete(child));

  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", chunk => stdout += chunk);
  child.stderr.on("data", chunk => stderr += chunk);

  const ready = await waitState(def.statePath, child.pid);
  return {
    def,
    child,
    state: ready.state,
    startup_ms: ready.elapsed_ms,
    stdout: () => stdout,
    stderr: () => stderr
  };
}

async function stopClean(runtime) {
  if (runtime.child.exitCode !== null) return;
  const closing = new Promise(resolve => runtime.child.once("close", resolve));
  const response = await callHostWithState(runtime.state, "system.shutdown");
  if (!response.ok) throw new Error(runtime.def.kind + " clean shutdown failed");
  if (runtime.child.exitCode === null) await closing;
}

async function killHard(runtime) {
  if (runtime.child.exitCode !== null) return;
  const closing = new Promise(resolve => runtime.child.once("close", resolve));
  runtime.child.kill("SIGKILL");
  if (runtime.child.exitCode === null) await closing;
}


const payloadBase = {
  summary: "Synthetic durable result fixture",
  exact_ids: Array.from(
    { length: 16 },
    (_, index) => `ENTITY-${String(index).padStart(4, "0")}`
  ),
  repeated_findings: Array.from({ length: 24 }, (_, index) => ({
    rule: `R-${index % 6}`,
    severity: index % 3,
    message: "deterministic fixture finding"
  })),
  note: "x".repeat(2048)
};

async function runStorageWorkload(kind, tempRoot) {
  const stateDir = path.join(tempRoot, kind + "-workload");
  const def = runtimeDef(kind, stateDir, "spike8-" + kind + "-workload");
  let host = await startRuntime(def);
  const initialStartupMs = host.startup_ms;

  const project = await callHostWithState(host.state, "project.register", {
    id: "PRJ-storage-benchmark",
    name: "Storage Benchmark Fixture",
    root_uri: "file:///C:/relay-fixture"
  });
  if (!project.ok) throw new Error(kind + " project registration failed");


  const writeTimes = [];
  const resultIds = [];
  for (let index = 0; index < 300; index++) {
    const started = performance.now();
    const response = await callHostWithState(host.state, "result.put", {
      project_id: "PRJ-storage-benchmark",
      kind: "BENCH",
      payload: { ...payloadBase, sequence: index }
    });
    if (!response.ok) throw new Error(kind + " result write failed");
    writeTimes.push(performance.now() - started);
    resultIds.push(response.result.id);
  }

  const readTimes = [];
  for (let index = 0; index < 600; index++) {
    const id = resultIds[(index * 37) % resultIds.length];
    const started = performance.now();
    const response = await callHostWithState(
      host.state,
      "result.get",
      { result_id: id }
    );
    if (!response.ok || response.result.id !== id) {
      throw new Error(kind + " result read mismatch");
    }
    readTimes.push(performance.now() - started);
  }


  const checkpointTimes = [];
  let jobId = null;
  for (let index = 0; index < 100; index++) {
    const started = performance.now();
    const args = {
      project_id: "PRJ-storage-benchmark",
      command: "benchmark.pipeline",
      state: index === 99 ? "CHECKPOINTED" : "RUNNING",
      checkpoint: {
        completed_stage: index,
        exact_cursor: `cursor-${index}`
      },
      result_id: resultIds[index % resultIds.length]
    };
    if (jobId) args.id = jobId;
    const response = await callHostWithState(
      host.state,
      "job.checkpoint",
      args
    );
    if (!response.ok) throw new Error(kind + " checkpoint failed");
    checkpointTimes.push(performance.now() - started);
    jobId = response.result.id;
  }

  const integrityTimes = [];
  let lastIntegrity = null;
  for (let index = 0; index < 20; index++) {
    const started = performance.now();
    lastIntegrity = await callHostWithState(
      host.state,
      "storage.integrity"
    );
    if (!lastIntegrity.ok || !lastIntegrity.result.ok) {
      throw new Error(kind + " integrity failed");
    }
    integrityTimes.push(performance.now() - started);
  }


  const dbPath = path.join(stateDir, "relay.sqlite3");
  const filesBeforeKill = {
    db_bytes: await fileSize(dbPath),
    wal_bytes: await fileSize(dbPath + "-wal"),
    shm_bytes: await fileSize(dbPath + "-shm")
  };
  const metricsAfterWork = processMetrics(host.child.pid);
  const persistedId = resultIds[173];

  await killHard(host);
  const restartStarted = performance.now();
  host = await startRuntime(def);
  const restartStartupMs = performance.now() - restartStarted;

  const persisted = await callHostWithState(
    host.state,
    "result.get",
    { result_id: persistedId }
  );
  const checkpoint = await callHostWithState(
    host.state,
    "job.get",
    { job_id: jobId }
  );
  const projects = await callHostWithState(host.state, "project.list");
  const postRestartIntegrity = await callHostWithState(
    host.state,
    "storage.integrity"
  );
  const metricsAfterRestart = processMetrics(host.child.pid);
  const idleAfterRestart = await idleSample(host.child.pid);

  await stopClean(host);
  const filesAfterCleanClose = {
    db_bytes: await fileSize(dbPath),
    wal_bytes: await fileSize(dbPath + "-wal"),
    shm_bytes: await fileSize(dbPath + "-shm")
  };

  return {
    startup_initial_ms: +initialStartupMs.toFixed(3),
    restart_startup_ms: +restartStartupMs.toFixed(3),
    sqlite_version:
      kind === "node"
        ? process.versions.sqlite
        : (lastIntegrity.result.sqlite_version ?? null),
    schema_version: lastIntegrity.result.schema_version,
    result_put_round_trip: summarize(writeTimes),
    result_get_round_trip: summarize(readTimes),
    checkpoint_round_trip: summarize(checkpointTimes),
    quick_check_round_trip: summarize(integrityTimes),
    files_before_hard_kill: filesBeforeKill,
    files_after_clean_close: filesAfterCleanClose,
    host_after_work: metricsAfterWork,
    host_after_restart: metricsAfterRestart,
    idle_after_restart: idleAfterRestart,
    pass_fail: {
      project_registry: project.result.id === "PRJ-storage-benchmark",
      durable_result_after_hard_kill:
        persisted.ok === true &&
        persisted.result.id === persistedId &&
        persisted.result.payload.sequence === 173,
      durable_checkpoint_after_hard_kill:
        checkpoint.ok === true &&
        checkpoint.result.id === jobId &&
        checkpoint.result.checkpoint.completed_stage === 99,
      project_list_after_hard_kill:
        projects.ok === true &&
        projects.result.projects.length === 1,
      integrity_after_hard_kill:
        postRestartIntegrity.ok === true &&
        postRestartIntegrity.result.ok === true,
      schema_version:
        postRestartIntegrity.result.schema_version === 1
    }
  };
}


async function runCorruptionTest(kind, tempRoot) {
  const stateDir = path.join(tempRoot, kind + "-corrupt");
  await fs.mkdir(stateDir, { recursive: true });
  const dbPath = path.join(stateDir, "relay.sqlite3");
  const original = Buffer.from("this is not sqlite", "utf8");
  await fs.writeFile(dbPath, original);

  const def = runtimeDef(kind, stateDir, "spike8-" + kind + "-corrupt");
  const host = await startRuntime(def);
  const status = await callHostWithState(host.state, "system.status");
  const doctor = await callHostWithState(host.state, "system.doctor");
  const write = await callHostWithState(host.state, "result.put", {
    kind: "TEST",
    payload: { should_not: "write" }
  });
  const afterBytes = await fs.readFile(dbPath);
  await stopClean(host);

  return {
    recovery_state: host.state.recovery_state,
    status_recovery_state: status.result?.recovery_state ?? null,
    doctor_healthy: doctor.result?.healthy ?? null,
    write_ok: write.ok,
    write_error_code: write.error?.code ?? null,
    original_bytes_preserved: afterBytes.equals(original)
  };
}


async function runFutureSchemaTest(kind, tempRoot) {
  const stateDir = path.join(tempRoot, kind + "-future");
  await fs.mkdir(stateDir, { recursive: true });
  const dbPath = path.join(stateDir, "relay.sqlite3");
  const db = new DatabaseSync(dbPath);
  db.exec(
    "CREATE TABLE schema_migrations (" +
    "version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);" +
    "INSERT INTO schema_migrations(version, applied_at) VALUES (999, 'future');"
  );
  db.close();

  const def = runtimeDef(kind, stateDir, "spike8-" + kind + "-future");
  const host = await startRuntime(def);
  const status = await callHostWithState(host.state, "system.status");
  const write = await callHostWithState(host.state, "project.register", {
    id: "PRJ-should-not-write",
    name: "Should Not Write",
    root_uri: "file:///blocked"
  });
  await stopClean(host);

  const verify = new DatabaseSync(dbPath, { readOnly: true });
  const row = verify
    .prepare("SELECT MAX(version) AS version FROM schema_migrations")
    .get();
  verify.close();

  return {
    recovery_state: host.state.recovery_state,
    status_recovery_state: status.result?.recovery_state ?? null,
    write_ok: write.ok,
    write_error_code: write.error?.code ?? null,
    schema_version_preserved: Number(row.version)
  };
}


async function writeInteropFixture(kind, stateDir, label) {
  const def = runtimeDef(kind, stateDir, "spike8-" + label + "-writer");
  const host = await startRuntime(def);
  const projectId = "PRJ-cross-runtime";
  const project = await callHostWithState(host.state, "project.register", {
    id: projectId,
    name: "Cross Runtime Fixture",
    root_uri: "file:///relay-cross-runtime"
  });
  if (!project.ok) throw new Error(kind + " interop project write failed");

  const result = await callHostWithState(host.state, "result.put", {
    project_id: projectId,
    kind: "INTEROP",
    payload: {
      writer: kind,
      exact_number: 73,
      nested: { preserved: true }
    }
  });
  if (!result.ok) throw new Error(kind + " interop result write failed");

  const job = await callHostWithState(host.state, "job.checkpoint", {
    project_id: projectId,
    command: "interop.fixture",
    state: "CHECKPOINTED",
    checkpoint: { writer: kind, cursor: 17 },
    result_id: result.result.id
  });
  if (!job.ok) throw new Error(kind + " interop checkpoint failed");


  const integrity = await callHostWithState(
    host.state,
    "storage.integrity"
  );
  await stopClean(host);

  return {
    project_id: projectId,
    result_id: result.result.id,
    result_sha256: result.result.payload_sha256,
    result_producer_version: result.result.producer_version,
    job_id: job.result.id,
    sqlite_version:
      kind === "node"
        ? process.versions.sqlite
        : (integrity.result.sqlite_version ?? null)
  };
}

async function readInteropFixture(kind, stateDir, label, fixture) {
  const def = runtimeDef(kind, stateDir, "spike8-" + label + "-reader");
  const host = await startRuntime(def);
  const result = await callHostWithState(host.state, "result.get", {
    result_id: fixture.result_id
  });
  const job = await callHostWithState(host.state, "job.get", {
    job_id: fixture.job_id
  });
  const projects = await callHostWithState(host.state, "project.list");
  const integrity = await callHostWithState(
    host.state,
    "storage.integrity"
  );
  await stopClean(host);

  return {
    result_ok: result.ok,
    result_payload_writer: result.result?.payload?.writer ?? null,
    result_exact_number: result.result?.payload?.exact_number ?? null,
    result_sha256_preserved:
      result.result?.payload_sha256 === fixture.result_sha256,

    producer_version_preserved:
      result.result?.producer_version === fixture.result_producer_version,
    job_ok: job.ok,
    checkpoint_writer: job.result?.checkpoint?.writer ?? null,
    checkpoint_cursor: job.result?.checkpoint?.cursor ?? null,
    project_count: projects.result?.projects?.length ?? null,
    project_id_preserved:
      projects.result?.projects?.[0]?.id === fixture.project_id,
    integrity_ok: integrity.result?.ok === true,
    schema_version: integrity.result?.schema_version ?? null,
    reader_sqlite_version:
      kind === "node"
        ? process.versions.sqlite
        : (integrity.result?.sqlite_version ?? null)
  };
}

async function crossRuntimeCompatibility(tempRoot) {
  const nodeToRustDir = path.join(tempRoot, "interop-node-to-rust");
  await fs.mkdir(nodeToRustDir, { recursive: true });
  const nodeFixture = await writeInteropFixture(
    "node",
    nodeToRustDir,
    "node-to-rust"
  );
  const rustReadsNode = await readInteropFixture(
    "rust",
    nodeToRustDir,
    "node-to-rust",
    nodeFixture
  );

  const rustToNodeDir = path.join(tempRoot, "interop-rust-to-node");
  await fs.mkdir(rustToNodeDir, { recursive: true });
  const rustFixture = await writeInteropFixture(
    "rust",
    rustToNodeDir,
    "rust-to-node"
  );
  const nodeReadsRust = await readInteropFixture(
    "node",
    rustToNodeDir,
    "rust-to-node",
    rustFixture
  );


  return {
    node_writes_rust_reads: {
      writer: nodeFixture,
      reader: rustReadsNode,
      pass:
        rustReadsNode.result_ok &&
        rustReadsNode.result_payload_writer === "node" &&
        rustReadsNode.result_exact_number === 73 &&
        rustReadsNode.result_sha256_preserved &&
        rustReadsNode.producer_version_preserved &&
        rustReadsNode.job_ok &&
        rustReadsNode.checkpoint_writer === "node" &&
        rustReadsNode.checkpoint_cursor === 17 &&
        rustReadsNode.project_id_preserved &&
        rustReadsNode.integrity_ok &&
        rustReadsNode.schema_version === 1
    },
    rust_writes_node_reads: {
      writer: rustFixture,
      reader: nodeReadsRust,
      pass:
        nodeReadsRust.result_ok &&
        nodeReadsRust.result_payload_writer === "rust" &&
        nodeReadsRust.result_exact_number === 73 &&
        nodeReadsRust.result_sha256_preserved &&
        nodeReadsRust.producer_version_preserved &&
        nodeReadsRust.job_ok &&
        nodeReadsRust.checkpoint_writer === "rust" &&
        nodeReadsRust.checkpoint_cursor === 17 &&
        nodeReadsRust.project_id_preserved &&
        nodeReadsRust.integrity_ok &&
        nodeReadsRust.schema_version === 1
    }
  };
}


function measuredExec(command, args, options = {}) {
  const started = performance.now();
  execFileSync(command, args, {
    windowsHide: true,
    stdio: "ignore",
    ...options
  });
  return performance.now() - started;
}

async function rustBuildEvidence() {
  const spike7 = JSON.parse(await fs.readFile(spike7Path, "utf8"));
  const baseline = spike7.build_and_package.rust;

  const cleanMs = measuredExec(
    "cargo.exe",
    ["clean", "--release"],
    { cwd: rustRoot }
  );
  const releaseBuildMs = measuredExec(
    "cargo.exe",
    ["build", "--release"],
    { cwd: rustRoot }
  );
  const incrementalBuildMs = measuredExec(
    "cargo.exe",
    ["build", "--release"],
    { cwd: rustRoot }
  );

  const metadata = JSON.parse(execFileSync(
    "cargo.exe",
    ["metadata", "--format-version", "1"],
    { cwd: rustRoot, encoding: "utf8", windowsHide: true }
  ));


  const rootId = metadata.resolve.root;
  const rootPackage = metadata.packages.find(pkg => pkg.id === rootId);
  const direct = rootPackage.dependencies
    .filter(dep => dep.kind === null)
    .map(dep => dep.name)
    .sort();

  const licenseNames = [
    "rusqlite",
    "libsqlite3-sys",
    "sha2"
  ];
  const licenses = Object.fromEntries(
    licenseNames.map(name => {
      const pkg = metadata.packages.find(candidate => candidate.name === name);
      return [
        name,
        pkg ? {
          version: pkg.version,
          license: pkg.license ?? null,
          source: pkg.source ?? null
        } : null
      ];
    })
  );

  const releaseBytes = (await fs.stat(rustExe)).size;
  return {
    spike7_baseline: {
      direct_dependency_count: baseline.direct_dependency_count,
      resolved_package_count_including_root:
        baseline.resolved_package_count_including_root,
      release_binary_bytes: baseline.release_binary_bytes,
      clean_release_build_ms: baseline.clean_release_build_ms
    },
    spike8_storage_build: {
      clean_command_ms: +cleanMs.toFixed(3),
      clean_release_build_ms: +releaseBuildMs.toFixed(3),
      incremental_release_build_ms: +incrementalBuildMs.toFixed(3),
      direct_dependencies: direct,
      direct_dependency_count: direct.length,
      resolved_package_count_including_root: metadata.resolve.nodes.length,
      release_binary_bytes: releaseBytes,
      selected_dependency_licenses: licenses
    }
  };
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
    rss_delta_bytes: after.rss_bytes - before.rss_bytes,
    handle_count: after.handle_count,
    thread_count: after.thread_count
  };
}


const tempRoot = await fs.mkdtemp(
  path.join(os.tmpdir(), "relay-spike8-")
);

try {
  process.stderr.write("[spike8] build/package evidence\n");
  const build = await rustBuildEvidence();

  process.stderr.write("[spike8] Node storage workload\n");
  const nodeWorkload = await runStorageWorkload("node", tempRoot);

  process.stderr.write("[spike8] Rust storage workload\n");
  const rustWorkload = await runStorageWorkload("rust", tempRoot);

  process.stderr.write("[spike8] damaged-store behavior\n");
  const corruption = {
    node: await runCorruptionTest("node", tempRoot),
    rust: await runCorruptionTest("rust", tempRoot)
  };

  process.stderr.write("[spike8] future-schema behavior\n");
  const futureSchema = {
    node: await runFutureSchemaTest("node", tempRoot),
    rust: await runFutureSchemaTest("rust", tempRoot)
  };

  process.stderr.write("[spike8] cross-runtime database compatibility\n");
  const interoperability = await crossRuntimeCompatibility(tempRoot);


  const workload = {
    projects: 1,
    result_writes: 300,
    result_reads: 600,
    checkpoint_updates: 100,
    integrity_checks: 20,
    approximate_payload_json_bytes:
      Buffer.byteLength(JSON.stringify({ ...payloadBase, sequence: 0 }))
  };

  const comparisons = {
    post_storage_idle_rss_reduction_rust_vs_node:
      +(1 - rustWorkload.idle_after_restart.rss_bytes /
        nodeWorkload.idle_after_restart.rss_bytes).toFixed(6),
    result_put_p50_ratio_rust_to_node:
      +(rustWorkload.result_put_round_trip.p50_ms /
        nodeWorkload.result_put_round_trip.p50_ms).toFixed(6),
    result_get_p50_ratio_rust_to_node:
      +(rustWorkload.result_get_round_trip.p50_ms /
        nodeWorkload.result_get_round_trip.p50_ms).toFixed(6),
    checkpoint_p50_ratio_rust_to_node:
      +(rustWorkload.checkpoint_round_trip.p50_ms /
        nodeWorkload.checkpoint_round_trip.p50_ms).toFixed(6),
    quick_check_p50_ratio_rust_to_node:
      +(rustWorkload.quick_check_round_trip.p50_ms /
        nodeWorkload.quick_check_round_trip.p50_ms).toFixed(6),
    rust_release_binary_growth_ratio_from_spike7:
      +(build.spike8_storage_build.release_binary_bytes /
        build.spike7_baseline.release_binary_bytes - 1).toFixed(6),
    rust_resolved_package_growth:
      build.spike8_storage_build.resolved_package_count_including_root -
      build.spike7_baseline.resolved_package_count_including_root
  };


  const result = {
    benchmark: "phase1-spike8-rust-storage-parity-windows",
    recorded_at: new Date().toISOString(),
    decision_informed:
      "Whether Rust preserves the proven SQLite operational-state contracts and enough of its Spike 7 footprint advantage to remain the preferred runtime candidate.",
    hypothesis:
      "Bundled synchronous SQLite can reproduce Node's schema-1 durability/recovery contract in Rust while keeping materially lower resident memory, at the cost of a larger binary/dependency/build surface.",
    runtime: {
      os: {
        platform: process.platform,
        release: os.release(),
        arch: os.arch()
      },
      node: process.version,
      node_sqlite: nodeWorkload.sqlite_version,
      rustc: execFileSync(
        "rustc.exe",
        ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim(),
      cargo: execFileSync(
        "cargo.exe",
        ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim(),
      rust_bundled_sqlite: rustWorkload.sqlite_version,
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem()
    },
    storage_contract: {
      engine: "SQLite",
      journal_mode: "WAL",
      synchronous: "FULL",
      foreign_keys: true,
      busy_timeout_ms: 3000,
      schema_version: 1,
      payload_model:
        "compact JSON result rows; heavyweight evidence intentionally excluded"
    },

    build_and_dependency_economics: build,
    workload,
    measurements: {
      node: nodeWorkload,
      rust: rustWorkload
    },
    failure_recovery: {
      corrupted_store: corruption,
      future_schema: futureSchema
    },
    database_interoperability: interoperability,
    comparisons,
    pass_fail: {
      node_workload_contracts:
        Object.values(nodeWorkload.pass_fail).every(Boolean),
      rust_workload_contracts:
        Object.values(rustWorkload.pass_fail).every(Boolean),
      node_corrupt_store_degrades_and_blocks_writes:
        corruption.node.recovery_state === "Degraded" &&
        corruption.node.status_recovery_state === "Degraded" &&
        corruption.node.doctor_healthy === false &&
        corruption.node.write_ok === false &&
        corruption.node.write_error_code === "STORAGE_UNAVAILABLE" &&
        corruption.node.original_bytes_preserved,
      rust_corrupt_store_degrades_and_blocks_writes:
        corruption.rust.recovery_state === "Degraded" &&
        corruption.rust.status_recovery_state === "Degraded" &&
        corruption.rust.doctor_healthy === false &&
        corruption.rust.write_ok === false &&
        corruption.rust.write_error_code === "STORAGE_UNAVAILABLE" &&
        corruption.rust.original_bytes_preserved,

      node_future_schema_degrades_without_downgrade:
        futureSchema.node.recovery_state === "Degraded" &&
        futureSchema.node.status_recovery_state === "Degraded" &&
        futureSchema.node.write_ok === false &&
        futureSchema.node.write_error_code === "STORAGE_UNAVAILABLE" &&
        futureSchema.node.schema_version_preserved === 999,
      rust_future_schema_degrades_without_downgrade:
        futureSchema.rust.recovery_state === "Degraded" &&
        futureSchema.rust.status_recovery_state === "Degraded" &&
        futureSchema.rust.write_ok === false &&
        futureSchema.rust.write_error_code === "STORAGE_UNAVAILABLE" &&
        futureSchema.rust.schema_version_preserved === 999,
      node_database_readable_by_rust:
        interoperability.node_writes_rust_reads.pass,
      rust_database_readable_by_node:
        interoperability.rust_writes_node_reads.pass,
      rust_storage_memory_advantage_survives:
        rustWorkload.idle_after_restart.rss_bytes <
        nodeWorkload.idle_after_restart.rss_bytes * 0.5,
      rust_storage_idle_cpu_near_zero:
        rustWorkload.idle_after_restart.cpu_ms <= 20,
      both_schema_version_one:
        nodeWorkload.schema_version === 1 &&
        rustWorkload.schema_version === 1
    },

    limitations: [
      "Single high-end Windows workstation and local NVMe-class storage only; synced/network-backed project state is not evaluated.",
      "The workload is single-host/single-writer. Concurrent readers/writers, long-reader WAL checkpoint behavior, VACUUM, and maintenance interruption remain open.",
      "Disk-full injection, backup/restore, and interrupted future migrations are not covered by this narrow schema-1 parity spike.",
      "SQLite holds operational metadata and compact result JSON only; heavyweight evidence remains outside the database by the Spike 3 decision.",
      "Rust uses rusqlite with default features disabled and bundled SQLite enabled. This increases the release/build dependency surface and statically compiles SQLite C code into the candidate.",
      "Selected dependency license metadata is captured from Cargo metadata, but final installer notice generation and release-license auditing remain packaging gates.",
      "Cross-runtime compatibility proves schema-1 database exchange in both directions; it does not approve arbitrary mixed-version rolling upgrades."
    ]
  };

  const passValues = Object.values(result.pass_fail);
  if (!passValues.every(Boolean)) {
    throw new Error(
      "Spike 8 gate failed: " +
      Object.entries(result.pass_fail)
        .filter(([, value]) => !value)
        .map(([key]) => key)
        .join(", ")
    );
  }

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(
    resultPath,
    JSON.stringify(result, null, 2) + "\n",
    "utf8"
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  for (const child of [...liveChildren]) {
    try {
      child.kill("SIGKILL");
    } catch {}
  }
  await sleep(100);
  await fs.rm(tempRoot, { recursive: true, force: true });
}
