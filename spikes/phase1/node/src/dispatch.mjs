import { RELAY_VERSION, SCHEMA_VERSION } from "./config.mjs";
import { executeCommand } from "./commands.mjs";

export function dispatchCommand({ requestId, command, arguments: args = {}, host }) {
  const base = {
    type: "command_result",
    request_id: requestId,
    schema_version: SCHEMA_VERSION,
    producer: { version: RELAY_VERSION }
  };
  try {
    const result = executeCommand(command, args, host);
    return { ...base, ok: true, result };
  } catch (error) {
    return {
      ...base,
      ok: false,
      error: {
        code: error.code ?? "INTERNAL",
        message: error.message
      }
    };
  }
}
