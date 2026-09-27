import assert from "node:assert/strict";
import test from "node:test";
import {
  aggregateEvents,
  compressZstd,
  downsampleTelemetry,
  restoreZstd,
  wholeBlobDedupe
} from "../src/evidence.mjs";

test("Spike 3 zstd levels restore exact bytes", () => {
  const source = Buffer.from("RELAY evidence fixture\n".repeat(10000));
  for (const level of [1, 3, 9, 19]) {
    const compressed = compressZstd(source, level);
    assert.ok(compressed.length < source.length);
    assert.equal(restoreZstd(compressed).equals(source), true);
  }
});

test("Spike 3 whole-blob dedupe respects project and workspace boundaries", () => {
  const same = Buffer.from("same evidence");
  const unique = Buffer.from("unique evidence");
  const records = [
    { workspace_id: "W1", project_id: "P1", bytes: same },
    { workspace_id: "W1", project_id: "P1", bytes: same },
    { workspace_id: "W1", project_id: "P2", bytes: same },
    { workspace_id: "W1", project_id: "P2", bytes: unique }
  ];
  const project = wholeBlobDedupe(records, "project");
  const workspace = wholeBlobDedupe(records, "workspace");
  assert.equal(project.unique.length, 3);
  assert.equal(workspace.unique.length, 2);
  assert.ok(workspace.saved_bytes > project.saved_bytes);
});

test("Spike 3 log aggregation preserves event counts and boundaries", () => {
  const events = [];
  for (let i = 0; i < 1000; i++) {
    events.push({
      timestamp: `2026-09-24T00:00:${String(i % 60).padStart(2, "0")}Z`,
      level: i % 4 === 0 ? "warn" : "info",
      code: `E${i % 5}`,
      message: `message-${i % 5}`
    });
  }
  const aggregated = aggregateEvents(events);
  assert.equal(aggregated.reduce((sum, row) => sum + row.count, 0), events.length);
  assert.ok(aggregated.length < events.length);
  assert.ok(aggregated.every(row => row.first_timestamp <= row.last_timestamp));
});

test("Spike 3 telemetry downsampling keeps count and extrema", () => {
  const samples = Array.from({ length: 1000 }, (_, i) => ({
    timestamp: i,
    value: i === 777 ? 9999 : Math.sin(i / 20) * 100
  }));
  const buckets = downsampleTelemetry(samples, 100);
  assert.equal(buckets.reduce((sum, row) => sum + row.count, 0), samples.length);
  assert.equal(Math.max(...buckets.map(row => row.max)), 9999);
  assert.equal(buckets.length, 10);
});
