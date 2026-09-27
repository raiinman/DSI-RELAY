import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { callHostWithState } from "../src/client.mjs";

const root = path.resolve(import.meta.dirname, "..");
const node = process.execPath;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

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
  const child = spawn(node, [path.join(root, "src", "daemon.mjs")], {
    cwd: root,
    env: { ...process.env, ...env },
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  let stderr = "";
  child.stderr.on("data", chunk => stderr += chunk);
  const state = await waitForState(path.join(env.RELAY_STATE_DIR, "host.json"), child.pid);
  return { child, state, stderr: () => stderr };
}

async function stopHost(host) {
  if (host.child.exitCode !== null) return;
  const closing = new Promise(resolve => host.child.once("close", resolve));
  await callHostWithState(host.state, "system.shutdown");
  if (host.child.exitCode === null) await closing;
}

async function firstJsonLine(child, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(() => reject(new Error("process did not report ready")), timeoutMs);
    child.stdout.setEncoding("utf8");
    const onData = chunk => {
      buffer += chunk;
      const index = buffer.indexOf("\n");
      if (index < 0) return;
      clearTimeout(timer);
      child.stdout.off("data", onData);
      resolve(JSON.parse(buffer.slice(0, index)));
    };
    child.stdout.on("data", onData);
    child.once("error", error => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function startStandaloneDashboard(env) {
  const child = spawn(node, [path.join(root, "src", "dashboard.mjs")], {
    cwd: root,
    env: { ...process.env, ...env },
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const ready = await firstJsonLine(child);
  return { child, ...ready };
}

async function stopProcess(child) {
  if (child.exitCode !== null) return;
  const closing = new Promise(resolve => child.once("close", resolve));
  child.kill("SIGTERM");
  if (child.exitCode === null) await closing;
}

async function dashboardSession(url) {
  const response = await fetch(url);
  assert.equal(response.status, 200);
  const html = await response.text();
  const token = html.match(/name="relay-dashboard-token" content="([0-9a-f]+)"/)?.[1];
  assert.ok(token, "dashboard token not found");
  return {
    token,
    html,
    csp: response.headers.get("content-security-policy")
  };
}

async function dashboardCommand(url, token, command, args = {}, extraHeaders = {}) {
  const response = await fetch(url + "/api/execute", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-relay-dashboard-token": token,
      ...extraHeaders
    },
    body: JSON.stringify({ command, arguments: args })
  });
  return { response, body: await response.json() };
}

function semantic(value) {
  const result = value.result ? structuredClone(value.result) : value.result;
  if (result && Object.hasOwn(result, "uptime_ms")) delete result.uptime_ms;
  return {
    type: value.type,
    schema_version: value.schema_version,
    producer: value.producer,
    ok: value.ok,
    result,
    error: value.error
  };
}

test("Spike 6 standalone dashboard is a read-only proxy of the host command contract", async t => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike6-standalone-"));
  const env = {
    RELAY_STATE_DIR: stateDir,
    RELAY_INSTANCE: `dash-standalone-${process.pid}`
  };
  const host = await startHost(env);
  let dashboard;
  t.after(async () => {
    await stopProcess(dashboard?.child).catch(() => {});
    if (host.child.exitCode === null) host.child.kill("SIGKILL");
    await fs.rm(stateDir, { recursive: true, force: true });
  });

  await callHostWithState(host.state, "project.register", {
    id: "PRJ-dashboard-fixture",
    name: "Dashboard Fixture",
    root_uri: "file:///relay-dashboard-fixture"
  });
  const put = await callHostWithState(host.state, "result.put", {
    project_id: "PRJ-dashboard-fixture",
    kind: "DASHBOARD_TEST",
    payload: { exact_value: 42 }
  });
  const resultId = put.result.id;

  dashboard = await startStandaloneDashboard(env);
  assert.equal(dashboard.mode, "standalone-proxy");
  assert.deepEqual(dashboard.commands, [
    "system.status",
    "system.doctor",
    "project.list",
    "result.get"
  ]);

  const session = await dashboardSession(dashboard.url);
  assert.match(session.csp, /frame-ancestors 'none'/);
  assert.doesNotMatch(session.html, /https?:\/\/(?!127\.0\.0\.1)/i);

  const directStatus = await callHostWithState(host.state, "system.status");
  const webStatus = await dashboardCommand(dashboard.url, session.token, "system.status");
  assert.equal(webStatus.response.status, 200);
  assert.deepEqual(semantic(webStatus.body), semantic(directStatus));

  const projects = await dashboardCommand(dashboard.url, session.token, "project.list");
  assert.equal(projects.body.ok, true);
  assert.equal(projects.body.result.projects[0].id, "PRJ-dashboard-fixture");

  const directResult = await callHostWithState(host.state, "result.get", { result_id: resultId });
  const webResult = await dashboardCommand(dashboard.url, session.token, "result.get", { result_id: resultId });
  assert.deepEqual(semantic(webResult.body), semantic(directResult));

  const directMissing = await callHostWithState(host.state, "result.get", { result_id: "RES-missing" });
  const webMissing = await dashboardCommand(dashboard.url, session.token, "result.get", { result_id: "RES-missing" });
  assert.equal(webMissing.response.status, 200);
  assert.deepEqual(semantic(webMissing.body), semantic(directMissing));

  const forbidden = await dashboardCommand(dashboard.url, session.token, "system.shutdown");
  assert.equal(forbidden.response.status, 403);
  assert.equal(forbidden.body.error.code, "DASHBOARD_COMMAND_NOT_EXPOSED");
  assert.equal((await callHostWithState(host.state, "system.status")).ok, true);

  const unauthorized = await dashboardCommand(dashboard.url, "wrong", "system.status");
  assert.equal(unauthorized.response.status, 401);
  assert.equal(unauthorized.body.error.code, "DASHBOARD_UNAUTHORIZED");

  const options = await fetch(dashboard.url + "/api/execute", { method: "OPTIONS" });
  assert.equal(options.status, 403);

  await stopHost(host);
  const unavailable = await dashboardCommand(dashboard.url, session.token, "system.status");
  assert.equal(unavailable.response.status, 503);
  assert.equal(unavailable.body.error.code, "HOST_UNAVAILABLE");
});

test("Spike 6 embedded dashboard preserves degraded Core truth and does not persist its browser token", async t => {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike6-embedded-"));
  await fs.writeFile(path.join(stateDir, "relay.sqlite3"), "not a sqlite database", "utf8");
  const env = {
    RELAY_STATE_DIR: stateDir,
    RELAY_INSTANCE: `dash-embedded-${process.pid}`,
    RELAY_DASHBOARD_MODE: "embedded"
  };
  const host = await startHost(env);
  t.after(async () => {
    if (host.child.exitCode === null) host.child.kill("SIGKILL");
    await fs.rm(stateDir, { recursive: true, force: true });
  });

  assert.equal(host.state.recovery_state, "Degraded");
  assert.equal(host.state.dashboard.mode, "embedded-host");
  const session = await dashboardSession(host.state.dashboard.url);
  assert.equal(JSON.stringify(host.state).includes(session.token), false);

  const directStatus = await callHostWithState(host.state, "system.status");
  const webStatus = await dashboardCommand(host.state.dashboard.url, session.token, "system.status");
  assert.deepEqual(semantic(webStatus.body), semantic(directStatus));
  assert.equal(webStatus.body.result.recovery_state, "Degraded");

  const doctor = await dashboardCommand(host.state.dashboard.url, session.token, "system.doctor");
  assert.equal(doctor.body.ok, true);
  assert.equal(doctor.body.result.healthy, false);

  const projects = await dashboardCommand(host.state.dashboard.url, session.token, "project.list");
  assert.equal(projects.response.status, 200);
  assert.equal(projects.body.ok, false);
  assert.equal(projects.body.error.code, "STORAGE_UNAVAILABLE");

  await stopHost(host);
});
