import crypto from "node:crypto";
import fs from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  CAPABILITIES,
  DB_PATH,
  PIPE_NAME,
  PROTOCOL_MAX,
  PROTOCOL_MIN,
  RELAY_VERSION,
  SCHEMA_VERSION,
  STATE_DIR
} from "./config.mjs";
import { dispatchCommand } from "./dispatch.mjs";
import { RelayStorage } from "./storage.mjs";
import { clearState, writeState } from "./state.mjs";
import { attachJsonLines, negotiate, sendJson } from "./wire.mjs";

await fs.mkdir(STATE_DIR, { recursive: true });

let storage = null;
let storageHealth;
try {
  storage = new RelayStorage(DB_PATH);
  storageHealth = storage.integrity();
} catch (error) {
  storageHealth = {
    ok: false,
    check: "unavailable",
    error: `${error.code ?? "STORAGE_ERROR"}: ${error.message}`
  };
}

const authToken = crypto.randomBytes(32).toString("hex");
const hostContext = protocol => ({ protocol, storage, storageHealth });
let dashboard = null;
let stopping = false;

const server = net.createServer(socket => {
  let protocol = null;
  attachJsonLines(socket, message => {
    if (!protocol) {
      if (message.type !== "hello" || message.auth_token !== authToken) {
        sendJson(socket, { type: "hello_error", code: "UNAUTHORIZED" });
        return socket.end();
      }
      protocol = negotiate(message.protocol_min, message.protocol_max, PROTOCOL_MIN, PROTOCOL_MAX);
      if (!protocol) {
        sendJson(socket, {
          type: "hello_error",
          code: "PROTOCOL_INCOMPATIBLE",
          host: { min: PROTOCOL_MIN, max: PROTOCOL_MAX }
        });
        return socket.end();
      }
      return sendJson(socket, {
        type: "hello_ok",
        protocol,
        schema_version: SCHEMA_VERSION,
        server: { name: "relayd", version: RELAY_VERSION },
        capabilities: CAPABILITIES
      });
    }

    if (message.type !== "command") return sendJson(socket, { type: "error", code: "BAD_MESSAGE" });
    const response = dispatchCommand({
      requestId: message.request_id,
      command: message.command,
      arguments: message.arguments,
      host: hostContext(protocol)
    });
    sendJson(socket, response);
    if (message.command === "system.shutdown" && response.ok) setTimeout(() => shutdown(0), 25);
  }, () => sendJson(socket, { type: "error", code: "INVALID_JSON" }));
});

async function closeResources() {
  await dashboard?.close().catch(() => {});
  dashboard = null;
  try { storage?.close(); } catch {}
  await clearState();
}

async function shutdown(code) {
  if (stopping) return;
  stopping = true;
  server.close(async () => {
    await closeResources();
    process.exit(code);
  });
  setTimeout(async () => {
    await dashboard?.close().catch(() => {});
    process.exit(code);
  }, 1000).unref();
}

server.on("error", async error => {
  await dashboard?.close().catch(() => {});
  try { storage?.close(); } catch {}
  await clearState().catch(() => {});
  console.error(error.code ?? error.message);
  process.exit(1);
});

server.listen(PIPE_NAME, async () => {
  if (process.env.RELAY_DASHBOARD_MODE === "embedded") {
    const { startDashboardServer } = await import("./dashboard-http.mjs");
    const here = path.dirname(fileURLToPath(import.meta.url));
    dashboard = await startDashboardServer({
      assetsDir: path.resolve(here, "..", "dashboard"),
      host: "127.0.0.1",
      port: Number(process.env.RELAY_DASHBOARD_PORT ?? 0),
      mode: "embedded-host",
      dispatch: (command, args, requestId = crypto.randomUUID()) =>
        dispatchCommand({
          requestId,
          command,
          arguments: args,
          host: hostContext(PROTOCOL_MAX)
        })
    });
  }

  await writeState({
    pid: process.pid,
    pipe: PIPE_NAME,
    auth_token: authToken,
    version: RELAY_VERSION,
    protocol: { min: PROTOCOL_MIN, max: PROTOCOL_MAX },
    capabilities: CAPABILITIES,
    recovery_state: storageHealth.ok ? "Healthy" : "Degraded",
    storage_schema_version: storageHealth.schema_version ?? null,
    dashboard: dashboard ? {
      mode: dashboard.mode,
      url: dashboard.url,
      commands: dashboard.commands
    } : null,
    started_at: new Date().toISOString()
  });
  console.log(
    `relayd ${RELAY_VERSION} ready (${storageHealth.ok ? "Healthy" : "Degraded"})` +
    (dashboard ? ` dashboard=${dashboard.url}` : "")
  );
});

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));
