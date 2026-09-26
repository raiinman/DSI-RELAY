import crypto from "node:crypto";
import os from "node:os";
import path from "node:path";

export const RELAY_VERSION = "0.1.0-spike2-node";
export const PROTOCOL_MIN = 1;
export const PROTOCOL_MAX = 1;
export const SCHEMA_VERSION = 1;
export const CAPABILITIES = Object.freeze([
  "protocol.handshake@1",
  "system.status@1",
  "system.doctor@1",
  "system.echo@1",
  "system.shutdown@1",
  "storage.integrity@1",
  "project.register@1",
  "project.list@1",
  "result.put@1",
  "result.get@1",
  "job.checkpoint@1",
  "job.get@1"
]);

const identity = `${process.env.USERDOMAIN ?? ""}\\${os.userInfo().username}`;
const userHash = crypto.createHash("sha256").update(identity).digest("hex").slice(0, 16);
const instance = process.env.RELAY_INSTANCE ?? userHash;
const localAppData = process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local");
export const STATE_DIR = process.env.RELAY_STATE_DIR ?? path.join(localAppData, "DSI", "RELAY", "phase1");
export const STATE_PATH = path.join(STATE_DIR, "host.json");
export const DB_PATH = process.env.RELAY_DB_PATH ?? path.join(STATE_DIR, "relay.sqlite3");
export const PIPE_NAME = `\\\\.\\pipe\\dsi-relay-${instance}-v1`;
