import { CAPABILITIES, PROTOCOL_MAX, PROTOCOL_MIN, RELAY_VERSION } from "./config.mjs";

function requireStorage(host) {
  if (!host.storage) {
    throw Object.assign(new Error("RELAY storage is unavailable; run relay doctor for details."), { code: "STORAGE_UNAVAILABLE" });
  }
  return host.storage;
}

export function executeCommand(command, args = {}, host) {
  if (command === "system.status") {
    return {
      process_health: "running",
      recovery_state: host.storageHealth?.ok ? "Healthy" : "Degraded",
      pid: process.pid,
      version: RELAY_VERSION,
      protocol: { min: PROTOCOL_MIN, max: PROTOCOL_MAX, negotiated: host.protocol },
      capabilities: CAPABILITIES,
      storage: host.storageHealth ?? { ok: false, check: "unavailable" },
      uptime_ms: Math.round(process.uptime() * 1000)
    };
  }

  if (command === "system.doctor") {
    const storageOk = host.storageHealth?.ok === true;
    return {
      healthy: storageOk,
      summary: storageOk
        ? "RELAY host, IPC, protocol, and durable storage checks passed."
        : "RELAY host is reachable, but durable storage needs attention.",
      checks: [
        { id: "host.process", status: "pass", detail: `PID ${process.pid}` },
        { id: "ipc.named_pipe", status: "pass", detail: "Authenticated named-pipe round trip succeeded." },
        { id: "ipc.explicit_acl", status: "warning", detail: "Spike 1 has not yet proven an explicit Windows pipe DACL or cross-user denial." },
        { id: "protocol.handshake", status: "pass", detail: `Protocol ${host.protocol}` },
        {
          id: "storage.integrity",
          status: storageOk ? "pass" : "fail",
          detail: storageOk ? `SQLite quick_check: ${host.storageHealth.check}` : (host.storageHealth?.error ?? "Storage unavailable")
        }
      ],
      next_action: storageOk ? null : "Protect the damaged store, inspect recovery options, and avoid writes until storage is repaired or restored."
    };
  }

  if (command === "system.echo") return { echo: args };
  if (command === "system.shutdown") return { shutting_down: true };

  if (command === "storage.integrity") return requireStorage(host).integrity();

  if (command === "project.register") return requireStorage(host).registerProject(args);
  if (command === "project.list") return { projects: requireStorage(host).listProjects() };

  if (command === "result.put") return requireStorage(host).putResult(args);
  if (command === "result.get") {
    const result = requireStorage(host).getResult(args.result_id);
    if (!result) throw Object.assign(new Error("Result not found"), { code: "RESULT_NOT_FOUND" });
    return result;
  }

  if (command === "job.checkpoint") return requireStorage(host).checkpointJob(args);
  if (command === "job.get") {
    const job = requireStorage(host).getJob(args.job_id);
    if (!job) throw Object.assign(new Error("Job not found"), { code: "JOB_NOT_FOUND" });
    return job;
  }

  throw Object.assign(new Error(`Unknown command: ${command}`), { code: "COMMAND_UNKNOWN" });
}
