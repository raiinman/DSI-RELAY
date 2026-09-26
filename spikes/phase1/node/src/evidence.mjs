import crypto from "node:crypto";
import { constants, zstdCompressSync, zstdDecompressSync } from "node:zlib";

export const sha256 = bytes => crypto.createHash("sha256").update(bytes).digest("hex");

export function compressZstd(bytes, level = 3) {
  return zstdCompressSync(bytes, {
    params: { [constants.ZSTD_c_compressionLevel]: level }
  });
}

export function restoreZstd(bytes) {
  return zstdDecompressSync(bytes);
}

export function wholeBlobDedupe(records, scope = "project") {
  if (!["project", "workspace"].includes(scope)) throw new Error("scope must be project or workspace");
  const seen = new Map();
  let originalBytes = 0;
  for (const record of records) {
    const bytes = Buffer.isBuffer(record.bytes) ? record.bytes : Buffer.from(record.bytes);
    originalBytes += bytes.length;
    const hash = sha256(bytes);
    const owner = scope === "project" ? record.project_id : record.workspace_id;
    const key = `${owner}:${hash}`;
    if (!seen.has(key)) seen.set(key, { key, hash, bytes });
  }
  const uniqueBytes = [...seen.values()].reduce((sum, item) => sum + item.bytes.length, 0);
  return {
    scope,
    original_bytes: originalBytes,
    unique_bytes: uniqueBytes,
    saved_bytes: originalBytes - uniqueBytes,
    reduction_ratio: originalBytes ? 1 - uniqueBytes / originalBytes : 0,
    unique: [...seen.values()]
  };
}

export function aggregateEvents(events) {
  const groups = new Map();
  for (const event of events) {
    const key = JSON.stringify([event.level, event.code, event.message]);
    let group = groups.get(key);
    if (!group) {
      group = {
        level: event.level,
        code: event.code,
        message: event.message,
        count: 0,
        first_timestamp: event.timestamp,
        last_timestamp: event.timestamp
      };
      groups.set(key, group);
    }
    group.count += 1;
    if (event.timestamp < group.first_timestamp) group.first_timestamp = event.timestamp;
    if (event.timestamp > group.last_timestamp) group.last_timestamp = event.timestamp;
  }
  return [...groups.values()].sort((a, b) => String(a.code).localeCompare(String(b.code)));
}

export function downsampleTelemetry(samples, bucketSize = 100) {
  if (!Number.isInteger(bucketSize) || bucketSize < 1) throw new Error("bucketSize must be a positive integer");
  const buckets = [];
  for (let i = 0; i < samples.length; i += bucketSize) {
    const slice = samples.slice(i, i + bucketSize);
    let min = Infinity, max = -Infinity, sum = 0;
    for (const sample of slice) {
      const value = Number(sample.value);
      min = Math.min(min, value);
      max = Math.max(max, value);
      sum += value;
    }
    buckets.push({
      start_timestamp: slice[0].timestamp,
      end_timestamp: slice.at(-1).timestamp,
      count: slice.length,
      min,
      max,
      mean: sum / slice.length,
      first: Number(slice[0].value),
      last: Number(slice.at(-1).value)
    });
  }
  return buckets;
}
