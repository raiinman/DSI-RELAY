import assert from "node:assert/strict";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const node = process.execPath;

function run(script, args = [], env = {}) {
  return new Promise(resolve => {
    const child = spawn(node, [path.join(root, script), ...args], {
      cwd: root, env: { ...process.env, ...env }, windowsHide: true
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", c => stdout += c);
    child.stderr.on("data", c => stderr += c);
    child.on("close", code => resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() }));
  });
}

async function waitForState(statePath, expectedPid, timeoutMs = 4000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (!expectedPid || state.pid === expectedPid) return state;
    } catch {}
    await new Promise(r => setTimeout(r, 20));
  }
  throw new Error("host state did not become current");
}

async function startHost(env) {
  const child = spawn(node, [path.join(root, "src/daemon.mjs")], {
    cwd: root, env: { ...process.env, ...env }, windowsHide: true
  });
  let stderr = "";
  child.stderr.on("data", c => stderr += c);
  const state = await waitForState(path.join(env.RELAY_STATE_DIR, "host.json"), child.pid);
  return { child, state, stderr: () => stderr };
}

async function rawHello(pipe, payload) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(pipe);
    let buffer = "";
    socket.setEncoding("utf8");
    socket.once("error", reject);
    socket.on("data", chunk => {
      buffer += chunk;
      const i = buffer.indexOf("\n");
      if (i >= 0) {
        const value = JSON.parse(buffer.slice(0, i));
        socket.destroy();
        resolve(value);
      }
    });
    socket.once("connect", () => socket.write(JSON.stringify(payload) + "\n"));
  });
}

test("Spike 1 round trip, doctor, restart, and fail-closed handshake", async t => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike1-"));
  const env = { RELAY_STATE_DIR: stateDir, RELAY_INSTANCE: `test-${process.pid}` };
  let host = await startHost(env);
  t.after(async () => {
    if (host.child.exitCode === null) host.child.kill("SIGKILL");
    await fs.rm(stateDir, { recursive: true, force: true });
  });

  const status = await run("src/cli.mjs", ["status", "--json"], env);
  assert.equal(status.code, 0, status.stderr);
  const statusJson = JSON.parse(status.stdout);
  assert.equal(statusJson.ok, true);
  assert.equal(statusJson.schema_version, 1);
  assert.equal(statusJson.result.recovery_state, "Healthy");
  assert.ok(statusJson.result.capabilities.includes("system.status@1"));

  const exec = await new Promise(resolve => {
    const child = spawn(node, [path.join(root, "src/cli.mjs"), "exec", "--stdin", "--json"], {
      cwd: root, env: { ...process.env, ...env }, windowsHide: true
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", c => stdout += c);
    child.stderr.on("data", c => stderr += c);
    child.on("close", code => resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() }));
    child.stdin.end(JSON.stringify({ command: "system.echo", arguments: { value: 42 } }));
  });
  assert.equal(exec.code, 0, exec.stderr);
  assert.deepEqual(JSON.parse(exec.stdout).result.echo, { value: 42 });

  const doctor = await run("src/cli.mjs", ["doctor", "--json"], env);
  assert.equal(doctor.code, 0, doctor.stderr);
  assert.equal(JSON.parse(doctor.stdout).result.healthy, true);

  const unauthorized = await rawHello(host.state.pipe, {
    type: "hello", auth_token: "wrong", protocol_min: 1, protocol_max: 1,
    client: { name: "test", version: "0" }
  });
  assert.equal(unauthorized.code, "UNAUTHORIZED");

  const incompatible = await rawHello(host.state.pipe, {
    type: "hello", auth_token: host.state.auth_token, protocol_min: 99, protocol_max: 99,
    client: { name: "test", version: "0" }
  });
  assert.equal(incompatible.code, "PROTOCOL_INCOMPATIBLE");

  const firstPid = host.state.pid;
  host.child.kill("SIGKILL");
  await new Promise(resolve => host.child.once("close", resolve));
  assert.equal((await fs.stat(path.join(stateDir, "host.json"))).isFile(), true);

  host = await startHost(env);
  assert.notEqual(host.state.pid, firstPid);
  const restarted = await run("src/cli.mjs", ["status", "--json"], env);
  assert.equal(restarted.code, 0, restarted.stderr);

  const stop = await run("src/daemon.mjs", ["--stop"], env);
  assert.equal(stop.code, 0, stop.stderr);
  if (host.child.exitCode === null) {
    await new Promise(resolve => host.child.once("close", resolve));
  }
  await assert.rejects(fs.access(path.join(stateDir, "host.json")));
});

test("machine mode rejects malformed input without prompting", async () => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike1-bad-"));
  const env = { RELAY_STATE_DIR: stateDir, RELAY_INSTANCE: `bad-${process.pid}` };
  const result = await new Promise(resolve => {
    const child = spawn(node, [path.join(root, "src/cli.mjs"), "exec", "--stdin", "--json"], {
      cwd: root, env: { ...process.env, ...env }, windowsHide: true
    });
    let stdout = "";
    child.stdout.on("data", c => stdout += c);
    child.on("close", code => resolve({ code, stdout: stdout.trim() }));
    child.stdin.end("{not-json");
  });
  assert.equal(result.code, 2);
  assert.equal(JSON.parse(result.stdout).error.code, "BAD_REQUEST");
  await fs.rm(stateDir, { recursive: true, force: true });
});
