import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { performance } from "node:perf_hooks";
import {
  checkUsnContinuity,
  diffSnapshots,
  parsePaths,
  probeUsnRead,
  queryUsnJournal,
  scanTree,
  watchTree
} from "../src/indexing.mjs";

const rootDir = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(rootDir, "..", "..", "..");
const fixture = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike4-index-"));
const resultPath = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike4-windows-indexing.json");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function percentile(values, p) {
  if (!values.length) return null;
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

function ioSnapshot(pid = process.pid) {
  const script = `$p=Get-CimInstance Win32_PerfRawData_PerfProc_Process | Where-Object {$_.IDProcess -eq ${pid}} | Select-Object -First 1 IDProcess,IOReadBytesPersec,IOWriteBytesPersec,IODataBytesPersec,WorkingSetPrivate | ConvertTo-Json -Compress`;
  try {
    const raw = execFileSync("powershell.exe", ["-NoProfile", "-Command", script], {
      encoding: "utf8",
      windowsHide: true
    }).trim();
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

async function measure(fn) {
  const ioBefore = ioSnapshot();
  const cpuBefore = process.cpuUsage();
  const rssBefore = process.memoryUsage().rss;
  const t0 = performance.now();
  const value = await fn();
  const elapsed = performance.now() - t0;
  const cpu = process.cpuUsage(cpuBefore);
  const rssAfter = process.memoryUsage().rss;
  const resource = process.resourceUsage();
  const ioAfter = ioSnapshot();
  const ioDelta = ioBefore && ioAfter ? {
    read_bytes: Math.max(0, Number(ioAfter.IOReadBytesPersec) - Number(ioBefore.IOReadBytesPersec)),
    write_bytes: Math.max(0, Number(ioAfter.IOWriteBytesPersec) - Number(ioBefore.IOWriteBytesPersec)),
    data_bytes: Math.max(0, Number(ioAfter.IODataBytesPersec) - Number(ioBefore.IODataBytesPersec))
  } : null;
  return {
    value,
    metrics: {
      elapsed_ms: +elapsed.toFixed(3),
      cpu_ms: +((cpu.user + cpu.system) / 1000).toFixed(3),
      rss_before_bytes: rssBefore,
      rss_after_bytes: rssAfter,
      rss_delta_bytes: rssAfter - rssBefore,
      max_rss_kib_reported: resource.maxRSS,
      windows_process_io_delta: ioDelta
    }
  };
}

async function createFixture() {
  const t0 = performance.now();
  let bytes = 0;
  const directories = 150;
  const filesPerDirectory = 100;
  for (let d = 0; d < directories; d++) {
    const dir = path.join(fixture, `d${String(d).padStart(3, "0")}`);
    await fs.mkdir(dir, { recursive: true });
    const writes = [];
    for (let f = 0; f < filesPerDirectory; f++) {
      const relative = `d${String(d).padStart(3, "0")}/f${String(f).padStart(3, "0")}.txt`;
      const header = `fixture=${d}:${f};entity=ENTITY-${(d * filesPerDirectory + f) % 2048};\n`;
      const content = header + "stable-project-content\n".repeat(45);
      bytes += Buffer.byteLength(content);
      writes.push(fs.writeFile(path.join(fixture, ...relative.split("/")), content));
    }
    await Promise.all(writes);
  }
  return {
    directories,
    files: directories * filesPerDirectory,
    logical_bytes_written: bytes,
    elapsed_ms: +(performance.now() - t0).toFixed(3)
  };
}

function mutationScript() {
  return String.raw`
const fs = require("fs");
const path = require("path");
const root = process.argv[1];
const plan = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
for (const op of plan) {
  const at = Date.now();
  const paths = op.op === "rename" ? [op.from, op.to] : [op.path];
  process.stdout.write(JSON.stringify({paths, at_ms: at}) + "\n");
  if (op.op === "modify") fs.appendFileSync(path.join(root, ...op.path.split("/")), op.append);
  else if (op.op === "add") fs.writeFileSync(path.join(root, ...op.path.split("/")), op.data);
  else if (op.op === "delete") fs.unlinkSync(path.join(root, ...op.path.split("/")));
  else if (op.op === "rename") fs.renameSync(path.join(root, ...op.from.split("/")), path.join(root, ...op.to.split("/")));
  if (op.delay_ms) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, op.delay_ms);
}
`;
}

async function runMutationPlan(plan, label) {
  const planPath = path.join(fixture, `.${label}-plan.json`);
  await fs.writeFile(planPath, JSON.stringify(plan));
  const mutationTimes = new Map();
  const child = spawn(process.execPath, ["-e", mutationScript(), fixture, planPath], {
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", chunk => {
    stdout += chunk;
    while (true) {
      const index = stdout.indexOf("\n");
      if (index < 0) break;
      const line = stdout.slice(0, index).trim();
      stdout = stdout.slice(index + 1);
      if (!line) continue;
      const event = JSON.parse(line);
      for (const relative of event.paths) mutationTimes.set(relative, event.at_ms);
    }
  });
  child.stderr.on("data", chunk => stderr += chunk);
  const code = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", resolve);
  });
  await fs.rm(planPath, { force: true });
  if (code !== 0) throw new Error(`mutation child failed: ${stderr}`);
  return mutationTimes;
}

function makeLivePlan(paths) {
  return [
    ...paths.slice(0, 120).map((file, i) => ({ op: "modify", path: file, append: `live-change-${i}\n` })),
    ...Array.from({ length: 20 }, (_, i) => ({
      op: "add",
      path: `d${String((i * 7) % 150).padStart(3, "0")}/added-live-${String(i).padStart(3, "0")}.txt`,
      data: `added live ${i}\n` + "x".repeat(700)
    })),
    ...paths.slice(120, 130).map(file => ({ op: "delete", path: file })),
    ...paths.slice(130, 140).map((file, i) => ({
      op: "rename",
      from: file,
      to: `${path.posix.dirname(file)}/renamed-live-${String(i).padStart(3, "0")}.txt`
    }))
  ];
}

function makeOfflinePlan(paths) {
  return [
    ...paths.slice(1000, 1050).map((file, i) => ({ op: "modify", path: file, append: `offline-change-${i}\n` })),
    ...Array.from({ length: 10 }, (_, i) => ({
      op: "add",
      path: `d${String((i * 11 + 3) % 150).padStart(3, "0")}/added-offline-${String(i).padStart(3, "0")}.txt`,
      data: `added offline ${i}\n` + "y".repeat(700)
    })),
    ...paths.slice(1050, 1060).map(file => ({ op: "delete", path: file }))
  ];
}

function makePacedPlan(paths) {
  return paths.slice(500, 540).map((file, i) => ({
    op: "modify",
    path: file,
    append: `paced-change-${i}\n`,
    delay_ms: 10
  }));
}

function changedPathSet(diff) {
  return new Set([
    ...diff.added,
    ...diff.removed,
    ...diff.modified,
    ...diff.renamed.flatMap(item => [item.from, item.to])
  ]);
}

async function settleWatcher(lastEventRef, maxMs = 2000, quietMs = 200) {
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    if (Date.now() - lastEventRef.value >= quietMs) return;
    await sleep(25);
  }
}

function fileIdCorrelation(samplePath, scan) {
  const absolute = path.join(fixture, ...samplePath.split("/"));
  const record = scan.files.get(samplePath);
  const output = execFileSync("fsutil.exe", ["usn", "readdata", absolute], {
    encoding: "utf8",
    windowsHide: true
  });
  const match = output.match(/FileRef#\s*:\s*(0x[0-9a-fA-F]+)/);
  if (!match) return { matched: false, node_inode_hex: record?.inode ?? null, usn_file_ref: null };
  const usnLow = BigInt(match[1]).toString(16);
  return {
    matched: usnLow === BigInt("0x" + record.inode).toString(16),
    node_inode_hex: record.inode,
    usn_file_ref: match[1]
  };
}

try {
  const fixtureInfo = await createFixture();

  const initialScan = await measure(() => scanTree(fixture));
  const allInitialPaths = [...initialScan.value.files.keys()].sort();
  const fullInitialParse = await measure(() => parsePaths(fixture, allInitialPaths));

  const watcherEvents = [];
  const firstSeenEpoch = new Map();
  const lastEventRef = { value: Date.now() };
  let watcherError = null;
  const watcher = watchTree(
    fixture,
    event => {
      watcherEvents.push(event);
      lastEventRef.value = Date.now();
      if (!firstSeenEpoch.has(event.path)) firstSeenEpoch.set(event.path, event.observed_at_epoch_ms);
    },
    error => { watcherError = String(error?.message ?? error); }
  );
  await sleep(150);

  const watcherIdleCpuBefore = process.cpuUsage();
  const watcherIdleRssBefore = process.memoryUsage().rss;
  await sleep(3000);
  const watcherIdleCpu = process.cpuUsage(watcherIdleCpuBefore);
  const watcherIdle = {
    sample_ms: 3000,
    cpu_ms: +((watcherIdleCpu.user + watcherIdleCpu.system) / 1000).toFixed(3),
    rss_before_bytes: watcherIdleRssBefore,
    rss_after_bytes: process.memoryUsage().rss
  };

  const livePlan = makeLivePlan(allInitialPaths);
  const watcherCpuBefore = process.cpuUsage();
  const mutationTimes = await runMutationPlan(livePlan, "live");
  await settleWatcher(lastEventRef);
  const watcherCpu = process.cpuUsage(watcherCpuBefore);
  const afterLiveScan = await measure(() => scanTree(fixture));
  const liveDiff = diffSnapshots(initialScan.value, afterLiveScan.value);
  const liveChangedPaths = changedPathSet(liveDiff);
  const watcherPaths = new Set(watcherEvents.map(event => event.path));
  const detectedChanged = [...liveChangedPaths].filter(relative => watcherPaths.has(relative));
  const latencies = [];
  for (const relative of detectedChanged) {
    const mutationAt = mutationTimes.get(relative);
    const observedAt = firstSeenEpoch.get(relative);
    if (mutationAt !== undefined && observedAt !== undefined) latencies.push(Math.max(0, observedAt - mutationAt));
  }

  const changedOnlyParse = await measure(() => parsePaths(fixture, liveDiff.candidatePaths));
  const fullAfterLiveParse = await measure(() => parsePaths(fixture, afterLiveScan.value.files.keys()));

  watcher.close();
  await sleep(100);

  const pacedEvents = [];
  const pacedFirstSeen = new Map();
  const pacedLastEventRef = { value: Date.now() };
  let pacedWatcherError = null;
  const pacedWatcher = watchTree(
    fixture,
    event => {
      pacedEvents.push(event);
      pacedLastEventRef.value = Date.now();
      if (!pacedFirstSeen.has(event.path)) pacedFirstSeen.set(event.path, event.observed_at_epoch_ms);
    },
    error => { pacedWatcherError = String(error?.message ?? error); }
  );
  await sleep(100);
  const pacedPlan = makePacedPlan([...afterLiveScan.value.files.keys()].sort());
  const pacedMutationTimes = await runMutationPlan(pacedPlan, "paced");
  await settleWatcher(pacedLastEventRef);
  pacedWatcher.close();
  await sleep(100);

  const afterPacedScan = await measure(() => scanTree(fixture));
  const pacedDiff = diffSnapshots(afterLiveScan.value, afterPacedScan.value);
  const pacedChangedPaths = changedPathSet(pacedDiff);
  const pacedWatcherPaths = new Set(pacedEvents.map(event => event.path));
  const pacedDetected = [...pacedChangedPaths].filter(relative => pacedWatcherPaths.has(relative));
  const pacedLatencies = [];
  for (const relative of pacedDetected) {
    const mutationAt = pacedMutationTimes.get(relative);
    const observedAt = pacedFirstSeen.get(relative);
    if (mutationAt !== undefined && observedAt !== undefined) pacedLatencies.push(Math.max(0, observedAt - mutationAt));
  }

  const usnBefore = await queryUsnJournal("C:");
  const beforeOffline = afterPacedScan.value;
  const offlinePaths = [...beforeOffline.files.keys()].sort();
  const offlinePlan = makeOfflinePlan(offlinePaths);
  await runMutationPlan(offlinePlan, "offline");
  await sleep(100);
  const usnAfter = await queryUsnJournal("C:");
  const usnContinuity = checkUsnContinuity(
    { journal_id: usnBefore.journal_id, next_usn: usnBefore.next_usn },
    usnAfter
  );
  const usnRead = await probeUsnRead("C:", usnBefore.next_usn);

  const reconciliation = await measure(() => scanTree(fixture));
  const offlineDiff = diffSnapshots(beforeOffline, reconciliation.value);
  const offlineChangedOnlyParse = await measure(() => parsePaths(fixture, offlineDiff.candidatePaths));
  const offlineChangedPaths = changedPathSet(offlineDiff);

  const samplePath = [...reconciliation.value.files.keys()][0];
  const idCorrelation = fileIdCorrelation(samplePath, reconciliation.value);

  const result = {
    benchmark: "phase1-spike4-windows-indexing",
    recorded_at: new Date().toISOString(),
    decision_informed: "Which Windows indexing/change-detection path should RELAY use without treating notifications or journals as authoritative project state.",
    hypothesis: "A baseline scan plus recursive notifications and changed-only parsing will provide the least-privilege default; USN may accelerate continuity recovery only if it can be isolated behind optional privilege without becoming a core dependency.",
    runtime: {
      os: { platform: process.platform, release: os.release(), arch: os.arch() },
      node: process.version,
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem()
    },
    fixture: fixtureInfo,
    baseline: {
      metadata_scan: { ...initialScan.metrics, logical_content_bytes_read: initialScan.value.metrics.logical_content_bytes_read },
      full_content_parse: { ...fullInitialParse.metrics, logical_content_bytes_read: fullInitialParse.value.logical_bytes_read }
    },
    notifications: {
      api: "node:fs.watch recursive on Windows (ReadDirectoryChangesW-backed platform surface)",
      watcher_error: watcherError,
      idle: watcherIdle,
      raw_event_count: watcherEvents.length,
      unique_event_paths: watcherPaths.size,
      actual_changed_paths: liveChangedPaths.size,
      detected_changed_paths: detectedChanged.length,
      path_coverage_ratio: liveChangedPaths.size ? +(detectedChanged.length / liveChangedPaths.size).toFixed(6) : 1,
      detection_latency: summarize(latencies),
      parent_cpu_ms_during_external_mutation_and_settle: +((watcherCpu.user + watcherCpu.system) / 1000).toFixed(3),
      paced_control: {
        delay_between_mutations_ms: 10,
        watcher_error: pacedWatcherError,
        raw_event_count: pacedEvents.length,
        unique_event_paths: pacedWatcherPaths.size,
        actual_changed_paths: pacedChangedPaths.size,
        detected_changed_paths: pacedDetected.length,
        path_coverage_ratio: pacedChangedPaths.size ? +(pacedDetected.length / pacedChangedPaths.size).toFixed(6) : 1,
        detection_latency: summarize(pacedLatencies),
        reconciliation_scan: afterPacedScan.metrics
      }
    },
    live_update: {
      actual_diff: {
        added: liveDiff.added.length,
        removed: liveDiff.removed.length,
        modified: liveDiff.modified.length,
        renamed: liveDiff.renamed.length
      },
      reconciliation_scan: afterLiveScan.metrics,
      changed_only_parse: { ...changedOnlyParse.metrics, files_parsed: changedOnlyParse.value.parsed.length, logical_content_bytes_read: changedOnlyParse.value.logical_bytes_read },
      full_parse_same_state: { ...fullAfterLiveParse.metrics, files_parsed: fullAfterLiveParse.value.parsed.length, logical_content_bytes_read: fullAfterLiveParse.value.logical_bytes_read },
      parse_byte_reduction_ratio: fullAfterLiveParse.value.logical_bytes_read
        ? +(1 - changedOnlyParse.value.logical_bytes_read / fullAfterLiveParse.value.logical_bytes_read).toFixed(6)
        : 0
    },
    continuity_loss: {
      watcher_was_intentionally_stopped: true,
      offline_changed_paths_recovered_by_reconciliation: offlineChangedPaths.size,
      reconciliation_scan: reconciliation.metrics,
      changed_only_parse_after_reconciliation: {
        ...offlineChangedOnlyParse.metrics,
        files_parsed: offlineChangedOnlyParse.value.parsed.length,
        logical_content_bytes_read: offlineChangedOnlyParse.value.logical_bytes_read
      }
    },
    usn: {
      query_before: usnBefore,
      query_after: usnAfter,
      journal_advanced: BigInt(usnAfter.next_usn) > BigInt(usnBefore.next_usn),
      continuity_metadata: usnContinuity,
      file_id_to_node_inode_correlation: idCorrelation,
      read_journal_as_signed_in_user: {
        available: usnRead.available,
        elapsed_ms: +usnRead.elapsed_ms.toFixed(3),
        code: usnRead.code ?? null,
        access_denied: !usnRead.available && (usnRead.stdout + usnRead.stderr).toLowerCase().includes("access is denied")
      },
      production_implication: usnRead.available
        ? "USN journal read is available in the signed-in user context on this fixture; benchmark direct journal delta processing before promotion."
        : "Journal metadata is queryable, but reading records is denied in the signed-in user context. USN cannot be a normal-host dependency; only an optional narrowly privileged helper remains a candidate."
    },
    pass_fail: {
      full_scan_complete: initialScan.value.metrics.files === fixtureInfo.files,
      recursive_notifications_operational: watcherError === null && detectedChanged.length > 0,
      burst_notifications_complete: detectedChanged.length === liveChangedPaths.size,
      paced_notifications_complete: pacedDetected.length === pacedChangedPaths.size,
      notifications_authoritative: false,
      reconciliation_recovers_offline_changes: offlineChangedPaths.size > 0,
      changed_only_parse_reduces_bytes: changedOnlyParse.value.logical_bytes_read < fullAfterLiveParse.value.logical_bytes_read,
      file_id_mapping_proven: idCorrelation.matched,
      usn_default_least_privilege_viable: usnRead.available
    },
    limitations: [
      "Synthetic 15,000-file fixture on one high-end Windows workstation; hardware-tier and real UEFN fixtures remain required.",
      "Raw Windows per-process I/O counter snapshots were unavailable in this run; logical content bytes, file counts, stat calls, elapsed time, CPU, and RSS are reported as the reproducible I/O/resource comparison.",
      "This benchmark does not force an actual ReadDirectoryChangesW buffer overflow; watcher downtime is used to prove the required reconciliation path.",
      "USN journal record reading could not be benchmarked because the signed-in non-elevated process received Access Denied. No elevation bypass was attempted.",
      "NTFS USN is volume-local and does not cover non-NTFS/network project roots; reconciliation remains mandatory even if an optional privileged USN helper is later adopted."
    ]
  };

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(resultPath, JSON.stringify(result, null, 2) + "\n", "utf8");
  console.log(JSON.stringify(result, null, 2));
} finally {
  await fs.rm(fixture, { recursive: true, force: true });
}
