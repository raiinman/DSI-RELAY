import crypto from "node:crypto";
import net from "node:net";
import { PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION, SCHEMA_VERSION } from "./config.mjs";
import { readState } from "./state.mjs";
import { attachJsonLines, sendJson } from "./wire.mjs";

export function callHostWithState(state, command, args = {}, timeoutMs = 3000, clientName = "relay-client") {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(state.pipe);
    const requestId = crypto.randomUUID();
    const timer = setTimeout(() => fail("TIMEOUT", "RELAY host did not respond in time."), timeoutMs);
    let greeted = false;

    const finish = value => {
      clearTimeout(timer);
      socket.end();
      resolve(value);
    };
    const fail = (code, message) => {
      clearTimeout(timer);
      socket.destroy();
      reject(Object.assign(new Error(message), { code }));
    };

    socket.once("error", () => fail("HOST_UNAVAILABLE", "RELAY host is not reachable."));
    attachJsonLines(socket, message => {
      if (!greeted) {
        if (message.type === "hello_error") return fail(message.code, "RELAY protocol handshake failed.");
        if (message.type !== "hello_ok") return fail("BAD_HANDSHAKE", "Unexpected RELAY handshake response.");
        greeted = true;
        return sendJson(socket, {
          type: "command",
          request_id: requestId,
          schema_version: SCHEMA_VERSION,
          command,
          arguments: args
        });
      }
      if (message.type === "command_result" && message.request_id === requestId) finish(message);
    }, () => fail("INVALID_JSON", "RELAY returned invalid JSON."));
    socket.once("connect", () => sendJson(socket, {
      type: "hello",
      auth_token: state.auth_token,
      protocol_min: PROTOCOL_MIN,
      protocol_max: PROTOCOL_MAX,
      client: { name: clientName, version: RELAY_VERSION }
    }));
  });
}

export async function callHost(command, args = {}, timeoutMs = 3000) {
  const state = await readState().catch(error => {
    throw Object.assign(new Error("RELAY host state is unavailable."), {
      code: "HOST_UNAVAILABLE",
      cause: error
    });
  });
  return callHostWithState(state, command, args, timeoutMs, "relay-cli");
}
