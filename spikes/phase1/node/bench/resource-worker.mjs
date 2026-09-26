import crypto from "node:crypto";
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { zstdCompressSync, constants } from "node:zlib";

if (!isMainThread) {
  const pattern = Buffer.from("RELAY-resource-coexistence-fixture\n");
  const buffer = Buffer.alloc(workerData.bufferBytes);
  for (let offset = 0; offset < buffer.length; offset += pattern.length) {
    pattern.copy(buffer, offset, 0, Math.min(pattern.length, buffer.length - offset));
  }

  const deadline = Date.now() + workerData.durationMs;
  let cycles = 0;
  let hashedBytes = 0;
  let compressedInputBytes = 0;
  let compressedOutputBytes = 0;
  while (Date.now() < deadline) {
    crypto.createHash("sha256").update(buffer).digest();
    hashedBytes += buffer.length;
    if ((cycles & 3) === 0) {
      const compressed = zstdCompressSync(buffer, {
        params: { [constants.ZSTD_c_compressionLevel]: 1 }
      });
      compressedInputBytes += buffer.length;
      compressedOutputBytes += compressed.length;
    }
    cycles++;
  }
  parentPort.postMessage({ cycles, hashedBytes, compressedInputBytes, compressedOutputBytes });
} else {
  const args = process.argv.slice(2);
  const value = (name, fallback) => {
    const index = args.indexOf(name);
    return index >= 0 ? Number(args[index + 1]) : fallback;
  };
  const durationMs = value("--duration-ms", 4000);
  const concurrency = value("--concurrency", Math.max(1, Math.min(12, (await import("node:os")).cpus().length - 2)));
  const bufferBytes = value("--buffer-bytes", 2 * 1024 * 1024);

  process.stdin.setEncoding("utf8");
  let started = false;
  process.stdin.on("data", async chunk => {
    if (started || !chunk.includes("GO")) return;
    started = true;
    const startedAt = performance.now();
    const workers = Array.from({ length: concurrency }, () => new Promise((resolve, reject) => {
      const worker = new Worker(new URL(import.meta.url), {
        workerData: { durationMs, bufferBytes }
      });
      worker.once("message", resolve);
      worker.once("error", reject);
    }));
    const results = await Promise.all(workers);
    const elapsedMs = performance.now() - startedAt;
    const totals = results.reduce((sum, item) => ({
      cycles: sum.cycles + item.cycles,
      hashedBytes: sum.hashedBytes + item.hashedBytes,
      compressedInputBytes: sum.compressedInputBytes + item.compressedInputBytes,
      compressedOutputBytes: sum.compressedOutputBytes + item.compressedOutputBytes
    }), { cycles: 0, hashedBytes: 0, compressedInputBytes: 0, compressedOutputBytes: 0 });

    process.stdout.write(JSON.stringify({
      duration_requested_ms: durationMs,
      elapsed_ms: +elapsedMs.toFixed(3),
      concurrency,
      buffer_bytes: bufferBytes,
      ...totals,
      hashed_mib_per_second: +(totals.hashedBytes / 1048576 / (elapsedMs / 1000)).toFixed(3)
    }) + "\n");
    process.exit(0);
  });
}
