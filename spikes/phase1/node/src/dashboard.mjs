#!/usr/bin/env node
import path from "node:path";
import { fileURLToPath } from "node:url";
import { callHost } from "./client.mjs";
import { startDashboardServer } from "./dashboard-http.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const assetsDir = path.resolve(here, "..", "dashboard");
const port = Number(process.env.RELAY_DASHBOARD_PORT ?? 0);

const dashboard = await startDashboardServer({
  assetsDir,
  port,
  mode: "standalone-proxy",
  dispatch: (command, args) => callHost(command, args)
});

process.stdout.write(JSON.stringify({
  component: "relay-dashboard",
  mode: dashboard.mode,
  url: dashboard.url,
  commands: dashboard.commands
}) + "\n");

async function stop() {
  await dashboard.close().catch(() => {});
  process.exit(0);
}
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
