import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { performance } from "node:perf_hooks";
import {
  aggregateEvents,
  compressZstd,
  downsampleTelemetry,
  restoreZstd,
  sha256,
  wholeBlobDedupe
} from "../src/evidence.mjs";

const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "..", "..", "..");
const workDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike3-"));
const resultPath = path.join(repoRoot, "spikes", "phase1", "results", "2026-09-24-spike3-evidence-lifecycle-windows.json");

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}
function summarizeMs(values) {
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
function deterministicBytes(size, seed = 0x12345678) {
  const out = Buffer.allocUnsafe(size);
  let x = seed >>> 0;
  for (let i = 0; i < out.length; i++) {
    x ^= x << 13; x ^= x >>> 17; x ^= x << 5;
    out[i] = x & 0xff;
  }
  return out;
}
function timed(fn) {
  const t0 = performance.now();
  const value = fn();
  return { value, ms: performance.now() - t0 };
}
async function fileSize(file) {
  try { return (await fs.stat(file)).size; } catch { return 0; }
}
async function directoryPayloadBytes(dir) {
  let total = 0;
  for (const name of await fs.readdir(dir)) total += (await fs.stat(path.join(dir, name))).size;
  return total;
}
function ffmpegVersion() {
  try {
    return execFileSync("ffmpeg", ["-version"], { encoding: "utf8", windowsHide: true }).split(/\r?\n/)[0];
  } catch {
    return null;
  }
}
function runFfmpeg(args) {
  const t0 = performance.now();
  execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], {
    encoding: "buffer", windowsHide: true, maxBuffer: 32 * 1024 * 1024
  });
  return performance.now() - t0;
}
function decodedPixelHash(file) {
  const raw = execFileSync("ffmpeg", [
    "-hide_banner", "-loglevel", "error", "-i", file,
    "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"
  ], { encoding: "buffer", windowsHide: true, maxBuffer: 16 * 1024 * 1024 });
  return sha256(raw);
}

const events = Array.from({ length: 100000 }, (_, i) => ({
  timestamp: new Date(1780000000000 + i * 10).toISOString(),
  level: i % 97 === 0 ? "error" : i % 11 === 0 ? "warn" : "info",
  code: `EVT-${i % 24}`,
  message: `Synthetic RELAY event family ${i % 24}`
}));
const logBuffer = Buffer.from(events.map(row => JSON.stringify(row)).join("\n") + "\n");

const telemetry = Array.from({ length: 200000 }, (_, i) => ({
  timestamp: i,
  value: i % 9999 === 0 ? 5000 : Math.sin(i / 47) * 120 + Math.cos(i / 251) * 20
}));
const telemetryBuffer = Buffer.from(telemetry.map(row => JSON.stringify(row)).join("\n") + "\n");

const sourceText = Buffer.from(
  Array.from({ length: 120000 }, (_, i) =>
    `module=${i % 64}; entity=ENTITY-${i % 4096}; rule=R-${i % 37}; state=${i % 5}; message=stable-deterministic-project-text\n`
  ).join("")
);
const randomBuffer = deterministicBytes(8 * 1024 * 1024);

const fixtures = [
  { name: "structured_logs", bytes: logBuffer },
  { name: "telemetry_jsonl", bytes: telemetryBuffer },
  { name: "source_like_text", bytes: sourceText },
  { name: "incompressible_binary", bytes: randomBuffer }
];

const compression = {};
for (const fixture of fixtures) {
  compression[fixture.name] = {
    original_bytes: fixture.bytes.length,
    levels: {}
  };
  for (const level of [1, 3, 9, 19]) {
    const compressTimes = [];
    let compressed;
    const compressionRuns = level === 19 ? 1 : 3;
    for (let i = 0; i < compressionRuns; i++) {
      const run = timed(() => compressZstd(fixture.bytes, level));
      compressTimes.push(run.ms);
      compressed = run.value;
    }
    const restoreTimes = [];
    let restored;
    const restoreRuns = level === 19 ? 3 : 5;
    for (let i = 0; i < restoreRuns; i++) {
      const run = timed(() => restoreZstd(compressed));
      restoreTimes.push(run.ms);
      restored = run.value;
    }
    if (!restored.equals(fixture.bytes)) throw new Error(`restore mismatch: ${fixture.name} level ${level}`);
    compression[fixture.name].levels[level] = {
      compressed_bytes: compressed.length,
      reduction_ratio: +(1 - compressed.length / fixture.bytes.length).toFixed(6),
      saved_bytes: fixture.bytes.length - compressed.length,
      compression: summarizeMs(compressTimes),
      decompression: summarizeMs(restoreTimes),
      restore_exact: true
    };
  }
}

const baseBlobs = [];
for (let i = 0; i < 30; i++) {
  const source = fixtures[i % fixtures.length].bytes;
  const start = (i * 131071) % Math.max(1, source.length - 262144);
  const slice = source.subarray(start, Math.min(start + 262144, source.length));
  baseBlobs.push(Buffer.concat([Buffer.from(`fixture-${i}\n`), slice]));
}
const dedupeRecords = [];
for (const project of ["P1", "P2"]) {
  for (let i = 0; i < 90; i++) {
    dedupeRecords.push({
      workspace_id: "W1",
      project_id: project,
      bytes: baseBlobs[(i + (project === "P2" ? 7 : 0)) % (project === "P1" ? 20 : 30)]
    });
  }
}
for (let i = 0; i < 60; i++) {
  dedupeRecords.push({ workspace_id: "W2", project_id: "P3", bytes: baseBlobs[i % 18] });
}
const projectDedupe = wholeBlobDedupe(dedupeRecords, "project");
const workspaceDedupe = wholeBlobDedupe(dedupeRecords, "workspace");
function dedupeReport(value) {
  const compressedBytes = value.unique.reduce((sum, item) => sum + compressZstd(item.bytes, 3).length, 0);
  return {
    references: dedupeRecords.length,
    original_bytes: value.original_bytes,
    unique_objects: value.unique.length,
    unique_raw_bytes: value.unique_bytes,
    raw_saved_bytes: value.saved_bytes,
    raw_reduction_ratio: +value.reduction_ratio.toFixed(6),
    unique_zstd_level3_bytes: compressedBytes,
    combined_dedupe_plus_compression_reduction_ratio: +(1 - compressedBytes / value.original_bytes).toFixed(6)
  };
}

const aggregationRun = timed(() => aggregateEvents(events));
const aggregatedBuffer = Buffer.from(JSON.stringify(aggregationRun.value));
const aggregationReport = {
  original_event_count: events.length,
  aggregate_rows: aggregationRun.value.length,
  original_jsonl_bytes: logBuffer.length,
  aggregate_json_bytes: aggregatedBuffer.length,
  reduction_ratio: +(1 - aggregatedBuffer.length / logBuffer.length).toFixed(6),
  elapsed_ms: +aggregationRun.ms.toFixed(3),
  count_preserved: aggregationRun.value.reduce((sum, row) => sum + row.count, 0) === events.length
};

const downsampleRun = timed(() => downsampleTelemetry(telemetry, 100));
const downsampleBuffer = Buffer.from(JSON.stringify(downsampleRun.value));
let originalMin = Infinity;
let originalMax = -Infinity;
for (const sample of telemetry) {
  originalMin = Math.min(originalMin, sample.value);
  originalMax = Math.max(originalMax, sample.value);
}
const downsampleReport = {
  original_sample_count: telemetry.length,
  buckets: downsampleRun.value.length,
  bucket_size: 100,
  original_jsonl_bytes: telemetryBuffer.length,
  downsample_json_bytes: downsampleBuffer.length,
  reduction_ratio: +(1 - downsampleBuffer.length / telemetryBuffer.length).toFixed(6),
  elapsed_ms: +downsampleRun.ms.toFixed(3),
  count_preserved: downsampleRun.value.reduce((sum, row) => sum + row.count, 0) === telemetry.length,
  global_min_preserved: Math.min(...downsampleRun.value.map(x => x.min)) === originalMin,
  global_max_preserved: Math.max(...downsampleRun.value.map(x => x.max)) === originalMax
};

const layoutRecords = dedupeRecords.filter(x => x.project_id === "P2");
const layoutUnique = wholeBlobDedupe(layoutRecords, "project").unique;
const layoutItems = layoutUnique.map(item => ({
  hash: item.hash,
  original: item.bytes,
  compressed: compressZstd(item.bytes, 3)
}));

const blobDbPath = path.join(workDir, "blob-layout.sqlite3");
const blobDb = new DatabaseSync(blobDbPath);
blobDb.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE evidence(hash TEXT PRIMARY KEY, original_bytes INTEGER, data BLOB NOT NULL);");
const blobInsert = blobDb.prepare("INSERT INTO evidence(hash, original_bytes, data) VALUES (?, ?, ?)");
const blobWriteTimes = [];
for (const item of layoutItems) {
  const run = timed(() => blobInsert.run(item.hash, item.original.length, item.compressed));
  blobWriteTimes.push(run.ms);
}
const blobRead = blobDb.prepare("SELECT data FROM evidence WHERE hash = ?");
const blobReadTimes = [];
for (let i = 0; i < 300; i++) {
  const item = layoutItems[(i * 17) % layoutItems.length];
  const run = timed(() => blobRead.get(item.hash));
  if (!Buffer.from(run.value.data).equals(item.compressed)) throw new Error("SQLite BLOB read mismatch");
  blobReadTimes.push(run.ms);
}
blobDb.exec("PRAGMA wal_checkpoint(TRUNCATE)");
blobDb.close();
const blobDbBytes = await fileSize(blobDbPath);

const fileRoot = path.join(workDir, "file-layout");
const fileBlobDir = path.join(fileRoot, "blobs");
await fs.mkdir(fileBlobDir, { recursive: true });
const fileMetaPath = path.join(fileRoot, "metadata.sqlite3");
const fileDb = new DatabaseSync(fileMetaPath);
fileDb.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE evidence(hash TEXT PRIMARY KEY, original_bytes INTEGER, compressed_bytes INTEGER, relative_path TEXT NOT NULL);");
const fileInsert = fileDb.prepare("INSERT INTO evidence(hash, original_bytes, compressed_bytes, relative_path) VALUES (?, ?, ?, ?)");
const fileLookup = fileDb.prepare("SELECT relative_path FROM evidence WHERE hash = ?");
const fileWriteTimes = [];
for (const item of layoutItems) {
  const relative = path.join("blobs", `${item.hash}.zst`);
  const run = performance.now();
  await fs.writeFile(path.join(fileRoot, relative), item.compressed);
  fileInsert.run(item.hash, item.original.length, item.compressed.length, relative);
  fileWriteTimes.push(performance.now() - run);
}
const fileReadTimes = [];
for (let i = 0; i < 300; i++) {
  const item = layoutItems[(i * 17) % layoutItems.length];
  const t0 = performance.now();
  const row = fileLookup.get(item.hash);
  const bytes = await fs.readFile(path.join(fileRoot, row.relative_path));
  fileReadTimes.push(performance.now() - t0);
  if (!bytes.equals(item.compressed)) throw new Error("file blob read mismatch");
}
fileDb.exec("PRAGMA wal_checkpoint(TRUNCATE)");
fileDb.close();
const fileMetaBytes = await fileSize(fileMetaPath);
const fileBlobBytes = await directoryPayloadBytes(fileBlobDir);

const vacuumPath = path.join(workDir, "blob-layout-vacuum.sqlite3");
const vacuumDb = new DatabaseSync(blobDbPath);
const vacuumStart = performance.now();
vacuumDb.exec(`VACUUM INTO '${vacuumPath.replaceAll("'", "''")}'`);
const vacuumMs = performance.now() - vacuumStart;
vacuumDb.close();
const vacuumBytes = await fileSize(vacuumPath);

let level1Total = 0;
let level9Total = 0;
let maxReplacementBytes = 0;
const recompressStart = performance.now();
for (const item of layoutItems) {
  const level1 = compressZstd(item.original, 1);
  level1Total += level1.length;
  const restored = restoreZstd(level1);
  const level9 = compressZstd(restored, 9);
  if (!restoreZstd(level9).equals(item.original)) throw new Error("idle recompression restore mismatch");
  level9Total += level9.length;
  maxReplacementBytes = Math.max(maxReplacementBytes, level9.length);
}
const recompressMs = performance.now() - recompressStart;

const imageDir = path.join(workDir, "images");
await fs.mkdir(imageDir, { recursive: true });
const imageReport = { ffmpeg: ffmpegVersion(), formats: {} };
if (imageReport.ffmpeg) {
  const png = path.join(imageDir, "evidence.png");
  runFfmpeg(["-f", "lavfi", "-i", "testsrc=size=1920x1080:rate=1", "-frames:v", "1", "-pix_fmt", "rgb24", png]);
  const sourceHash = decodedPixelHash(png);
  imageReport.formats.png = { bytes: await fileSize(png), exact_pixel_match: true, role: "lossless_evidence" };

  const attempts = [
    { name: "webp_lossless", file: "evidence-lossless.webp", args: ["-i", png, "-frames:v", "1", "-c:v", "libwebp", "-lossless", "1"], role: "lossless_evidence" },
    { name: "jpeg_q2", file: "reference-q2.jpg", args: ["-i", png, "-frames:v", "1", "-q:v", "2"], role: "lossy_reference" },
    { name: "avif_crf18", file: "reference-crf18.avif", args: ["-i", png, "-frames:v", "1", "-c:v", "libaom-av1", "-crf", "18", "-cpu-used", "6", "-still-picture", "1"], role: "lossy_reference" }
  ];
  for (const candidate of attempts) {
    const output = path.join(imageDir, candidate.file);
    try {
      const encodeMs = runFfmpeg([...candidate.args, output]);
      const decodedHash = decodedPixelHash(output);
      imageReport.formats[candidate.name] = {
        bytes: await fileSize(output),
        encode_ms: +encodeMs.toFixed(3),
        exact_pixel_match: decodedHash === sourceHash,
        role: candidate.role
      };
    } catch (error) {
      imageReport.formats[candidate.name] = { available: false, role: candidate.role, error: String(error.message).slice(0, 200) };
    }
  }
}

const layoutReport = {
  corpus_objects: layoutItems.length,
  original_raw_bytes: layoutItems.reduce((sum, x) => sum + x.original.length, 0),
  compressed_level3_bytes: layoutItems.reduce((sum, x) => sum + x.compressed.length, 0),
  sqlite_blob: {
    database_bytes_after_checkpoint: blobDbBytes,
    write_latency: summarizeMs(blobWriteTimes),
    read_latency: summarizeMs(blobReadTimes),
    vacuum_copy_bytes: vacuumBytes,
    vacuum_ms: +vacuumMs.toFixed(3),
    observed_db_plus_vacuum_headroom_bytes: blobDbBytes + vacuumBytes
  },
  metadata_plus_files: {
    metadata_db_bytes_after_checkpoint: fileMetaBytes,
    blob_payload_bytes: fileBlobBytes,
    total_logical_bytes: fileMetaBytes + fileBlobBytes,
    write_latency: summarizeMs(fileWriteTimes),
    read_latency: summarizeMs(fileReadTimes),
    per_blob_atomic_recompression_extra_headroom_bytes: maxReplacementBytes
  }
};

const result = {
  benchmark: "phase1-spike3-evidence-storage-lifecycle-windows",
  recorded_at: new Date().toISOString(),
  decision_informed: "How RELAY should reduce evidence growth before choosing a v0.1 evidence-storage layout and default compression policy.",
  hypothesis: "Exact hash dedupe plus fast Zstd and metadata-plus-file blobs will provide strong savings with simpler bounded maintenance than storing heavyweight evidence directly as SQLite BLOBs; aggregation/downsampling should be policy-tiered, not universal.",
  runtime: {
    os: { platform: process.platform, release: os.release(), arch: os.arch() },
    node: process.version,
    sqlite: process.versions.sqlite,
    zstd: process.versions.zstd,
    ffmpeg: imageReport.ffmpeg,
    cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
    logical_cpu_count: os.cpus().length,
    total_memory_bytes: os.totalmem()
  },
  fixture_bytes: Object.fromEntries(fixtures.map(x => [x.name, x.bytes.length])),
  compression,
  dedupe: {
    project_scope: dedupeReport(projectDedupe),
    workspace_scope: dedupeReport(workspaceDedupe)
  },
  log_aggregation: aggregationReport,
  telemetry_downsampling: downsampleReport,
  storage_layout: layoutReport,
  idle_recompression: {
    objects: layoutItems.length,
    level1_total_bytes: level1Total,
    level9_total_bytes: level9Total,
    additional_savings_bytes: level1Total - level9Total,
    additional_savings_ratio_vs_level1: +(1 - level9Total / level1Total).toFixed(6),
    elapsed_ms: +recompressMs.toFixed(3),
    restore_exact: true
  },
  image_storage: imageReport,
  pipeline_recommendation_subject_to_review: [
    "classify and retain provenance before compression",
    "SHA-256 whole-blob hash before write",
    "default dedupe boundary: project; workspace dedupe only when policy explicitly allows cross-project sharing",
    "fast/default Zstd for hot/warm exact evidence",
    "SQLite for metadata and compact results; file/blob storage for heavyweight evidence if layout benchmark remains favorable",
    "aggregate repetitive logs and downsample telemetry only under declared retention classes that preserve required counts/extrema",
    "recompress colder unpinned evidence during idle windows only when measured savings justify CPU/I/O",
    "lossy image formats are reference derivatives, never substitutes for pinned exact evidence"
  ],
  limitations: [
    "Synthetic fixtures are designed to expose compression extremes; real UEFN evidence distributions still need field traces.",
    "Filesystem results report logical payload bytes, not NTFS allocation-unit overhead or antivirus/indexer effects.",
    "No concurrent UEFN/Fortnite foreground workload was active, so compression interference is not yet approved.",
    "Whole-blob dedupe is tested; content-defined chunking remains intentionally deferred until measured evidence shows a need.",
    "Telemetry downsampling and log aggregation are intentionally lossy/structural transformations and require retention-class policy before production use.",
    "AVIF/JPEG are evaluated only as reference derivatives; exact evidentiary fidelity is represented by decoded-pixel equality for lossless formats."
  ]
};

await fs.mkdir(path.dirname(resultPath), { recursive: true });
await fs.writeFile(resultPath, JSON.stringify(result, null, 2) + "\n", "utf8");
console.log(JSON.stringify(result, null, 2));
await fs.rm(workDir, { recursive: true, force: true });
