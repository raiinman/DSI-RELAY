import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const node = process.execPath;
const sleep = ms => new Promise(r => setTimeout(r, ms));

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

async function startHost(env) {
  const child = spawn(node, [path.join(root, "src/daemon.mjs")], {
    cwd: root, env: { ...process.env, ...env }, windowsHide: true
  });
  const state = await waitForState(path.join(env.RELAY_STATE_DIR, "host.json"), child.pid);
  return { child, state };
}

async function stopHost(host, env) {
  const stop = await runClient(env, "system.shutdown");
  assert.equal(stop.code, 0);
  if (host.child.exitCode === null) {
    await new Promise(resolve => host.child.once("close", resolve));
  }
}

function runClient(env, command, args = {}) {
  return new Promise((resolve, reject) => {
    const request = JSON.stringify({ command, arguments: args });
    const child = spawn(node, [path.join(root, "src/cli.mjs"), "exec", "--stdin", "--json"], {
      cwd: root, env: { ...process.env, ...env }, windowsHide: true
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", c => stdout += c);
    child.stderr.on("data", c => stderr += c);
    child.on("error", reject);
    child.on("close", code => {
      if (code !== 0 && !stdout) return reject(new Error(stderr || `client exited ${code}`));
      resolve({ code, value: JSON.parse(stdout.trim()) });
    });
    child.stdin.end(request);
  });
}

test("Spike 2 persists projects, results, and checkpoints across restart", async t => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike2-"));
  const env = { RELAY_STATE_DIR: stateDir, RELAY_INSTANCE: `spike2-${process.pid}` };
  let host = await startHost(env);
  t.after(async () => {
    if (host.child.exitCode === null) host.child.kill("SIGKILL");
    await fs.rm(stateDir, { recursive: true, force: true });
  });

  const projectId = "PRJ-fixture-phase1";
  const project = await runClient(env, "project.register", {
    id: projectId,
    name: "Phase 1 Fixture",
    root_uri: "file:///C:/relay-fixture"
  });
  assert.equal(project.code, 0);
  assert.equal(project.value.result.id, projectId);

  const resultPut = await runClient(env, "result.put", {
    project_id: projectId,
    kind: "TEST",
    payload: { exact_number: 42, message: "durable round trip" }
  });
  assert.equal(resultPut.code, 0);
  const resultId = resultPut.value.result.id;
  assert.match(resultId, /^RES-/);
  assert.equal(resultPut.value.result.payload_sha256.length, 64);

  const checkpoint = await runClient(env, "job.checkpoint", {
    project_id: projectId,
    command: "fixture.work",
    state: "CHECKPOINTED",
    checkpoint: { completed_stage: 2 },
    result_id: resultId
  });
  assert.equal(checkpoint.code, 0);
  const jobId = checkpoint.value.result.id;
  assert.match(jobId, /^JOB-/);

  const integrity = await runClient(env, "storage.integrity");
  assert.equal(integrity.value.result.ok, true);
  assert.equal(integrity.value.result.schema_version, 1);

  host.child.kill("SIGKILL");
  await new Promise(resolve => host.child.once("close", resolve));
  host = await startHost(env);

  const resultGet = await runClient(env, "result.get", { result_id: resultId });
  assert.equal(resultGet.code, 0);
  assert.deepEqual(resultGet.value.result.payload, { exact_number: 42, message: "durable round trip" });

  const jobGet = await runClient(env, "job.get", { job_id: jobId });
  assert.equal(jobGet.code, 0);
  assert.deepEqual(jobGet.value.result.checkpoint, { completed_stage: 2 });

  const projects = await runClient(env, "project.list");
  assert.equal(projects.value.result.projects.length, 1);
  assert.equal(projects.value.result.projects[0].id, projectId);

  const doctor = await runClient(env, "system.doctor");
  assert.equal(doctor.value.result.healthy, true);
  await stopHost(host, env);
});

test("Spike 2 starts degraded instead of hiding a damaged database", async t => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike2-corrupt-"));
  const env = { RELAY_STATE_DIR: stateDir, RELAY_INSTANCE: `corrupt-${process.pid}` };
  await fs.writeFile(path.join(stateDir, "relay.sqlite3"), "this is not sqlite", "utf8");
  const host = await startHost(env);
  t.after(async () => {
    if (host.child.exitCode === null) host.child.kill("SIGKILL");
    await fs.rm(stateDir, { recursive: true, force: true });
  });

  assert.equal(host.state.recovery_state, "Degraded");
  const status = await runClient(env, "system.status");
  assert.equal(status.value.result.recovery_state, "Degraded");
  const doctor = await runClient(env, "system.doctor");
  assert.equal(doctor.value.result.healthy, false);
  const write = await runClient(env, "result.put", { kind: "TEST", payload: { should_not: "write" } });
  assert.equal(write.code, 3);
  assert.equal(write.value.error.code, "STORAGE_UNAVAILABLE");
});
