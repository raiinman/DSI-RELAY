import crypto from "node:crypto";
import fs from "node:fs/promises";
import net from "node:net";
import { CAPABILITIES, DB_PATH, PIPE_NAME, PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION, SCHEMA_VERSION, STATE_DIR } from "./config.mjs";
import { executeCommand } from "./commands.mjs";
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
        sendJson(socket, { type: "hello_error", code: "PROTOCOL_INCOMPATIBLE", host: { min: PROTOCOL_MIN, max: PROTOCOL_MAX } });
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
    const base = {
      type: "command_result",
      request_id: message.request_id,
      schema_version: SCHEMA_VERSION,
      producer: { version: RELAY_VERSION }
    };
    try {
      const result = executeCommand(message.command, message.arguments, { protocol, storage, storageHealth });
      sendJson(socket, { ...base, ok: true, result });
      if (message.command === "system.shutdown") setTimeout(() => shutdown(0), 25);
    } catch (error) {
      sendJson(socket, { ...base, ok: false, error: { code: error.code ?? "INTERNAL", message: error.message } });
    }
  }, () => sendJson(socket, { type: "error", code: "INVALID_JSON" }));
});

async function shutdown(code) {
  if (stopping) return;
  stopping = true;
  server.close(async () => {
    try { storage?.close(); } catch {}
    await clearState();
    process.exit(code);
  });
  setTimeout(() => process.exit(code), 1000).unref();
}

server.on("error", async error => {
  try { storage?.close(); } catch {}
  await clearState().catch(() => {});
  console.error(error.code ?? error.message);
  process.exit(1);
});

server.listen(PIPE_NAME, async () => {
  await writeState({
    pid: process.pid,
    pipe: PIPE_NAME,
    auth_token: authToken,
    version: RELAY_VERSION,
    protocol: { min: PROTOCOL_MIN, max: PROTOCOL_MAX },
    capabilities: CAPABILITIES,
    recovery_state: storageHealth.ok ? "Healthy" : "Degraded",
    storage_schema_version: storageHealth.schema_version ?? null,
    started_at: new Date().toISOString()
  });
  console.log(`relayd ${RELAY_VERSION} ready (${storageHealth.ok ? "Healthy" : "Degraded"})`);
});

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));
