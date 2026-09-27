import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  checkUsnContinuity,
  diffSnapshots,
  parsePaths,
  parseUsnQuery,
  scanTree,
  watchTree
} from "../src/indexing.mjs";

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

test("Spike 4 full scan maps stable Windows file IDs and detects changes", async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike4-scan-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  await fs.mkdir(path.join(root, "nested"));
  await fs.writeFile(path.join(root, "a.txt"), "alpha");
  await fs.writeFile(path.join(root, "nested", "b.txt"), "bravo");

  const before = await scanTree(root);
  assert.equal(before.metrics.files, 2);
  assert.equal(before.inodeToPath.get(before.files.get("a.txt").inode), "a.txt");

  await fs.appendFile(path.join(root, "a.txt"), "-changed");
  await fs.rename(path.join(root, "nested", "b.txt"), path.join(root, "nested", "renamed.txt"));
  await fs.writeFile(path.join(root, "new.txt"), "new");

  const after = await scanTree(root);
  const diff = diffSnapshots(before, after);
  assert.deepEqual(diff.modified, ["a.txt"]);
  assert.equal(diff.renamed.length, 1);
  assert.equal(diff.renamed[0].from, "nested/b.txt");
  assert.equal(diff.renamed[0].to, "nested/renamed.txt");
  assert.deepEqual(diff.added, ["new.txt"]);
});

test("Spike 4 changed-only parse reads only candidate bytes", async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike4-parse-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  await fs.writeFile(path.join(root, "a.txt"), "a".repeat(1000));
  await fs.writeFile(path.join(root, "b.txt"), "b".repeat(2000));
  const parsed = await parsePaths(root, new Set(["b.txt"]));
  assert.equal(parsed.parsed.length, 1);
  assert.equal(parsed.logical_bytes_read, 2000);
  assert.equal(parsed.parsed[0].path, "b.txt");
});

test("Spike 4 recursive watcher observes nested writes", async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike4-watch-"));
  await fs.mkdir(path.join(root, "nested"));
  const seen = [];
  const watcher = watchTree(root, event => seen.push(event));
  t.after(async () => {
    watcher.close();
    await fs.rm(root, { recursive: true, force: true });
  });

  await sleep(50);
  await fs.writeFile(path.join(root, "nested", "watched.txt"), "watch me");
  const deadline = Date.now() + 2000;
  while (Date.now() < deadline && !seen.some(event => event.path.endsWith("nested/watched.txt"))) {
    await sleep(20);
  }
  assert.ok(seen.some(event => event.path.endsWith("nested/watched.txt")), JSON.stringify(seen));
});

test("Spike 4 USN continuity fails safely when journal history cannot bridge the checkpoint", () => {
  const text = `
Usn Journal ID   : 0x00000000000000aa
First Usn        : 0x0000000000001000
Next Usn         : 0x0000000000005000
Lowest Valid Usn : 0x0000000000000000
Max Usn          : 0x00000fffffff0000
`;
  const current = parseUsnQuery(text);
  assert.equal(current.journal_id, "0x00000000000000aa");
  assert.deepEqual(
    checkUsnContinuity({ journal_id: current.journal_id, next_usn: "0x2000" }, current),
    { valid: true, reason: "continuous" }
  );
  assert.deepEqual(
    checkUsnContinuity({ journal_id: current.journal_id, next_usn: "0x0800" }, current),
    { valid: false, reason: "checkpoint_trimmed" }
  );
  assert.deepEqual(
    checkUsnContinuity({ journal_id: "0xbb", next_usn: "0x2000" }, current),
    { valid: false, reason: "journal_id_changed" }
  );
});
