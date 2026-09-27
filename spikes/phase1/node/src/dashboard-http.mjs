import crypto from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { RELAY_VERSION, SCHEMA_VERSION } from "./config.mjs";

export const DASHBOARD_COMMANDS = Object.freeze([
  "system.status",
  "system.doctor",
  "project.list",
  "result.get"
]);

const DASHBOARD_COMMAND_SET = new Set(DASHBOARD_COMMANDS);
const MAX_REQUEST_BYTES = 64 * 1024;

function transportError(code, message) {
  return {
    type: "dashboard_transport_error",
    schema_version: SCHEMA_VERSION,
    producer: { version: RELAY_VERSION },
    ok: false,
    error: { code, message }
  };
}

function commonHeaders() {
  return {
    "cache-control": "no-store",
    "content-security-policy": "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-resource-policy": "same-origin",
    "referrer-policy": "no-referrer",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  };
}

function send(res, status, body, contentType = "application/json; charset=utf-8") {
  res.writeHead(status, { ...commonHeaders(), "content-type": contentType });
  res.end(body);
}

function secureEqual(actual, expected) {
  const a = Buffer.from(String(actual ?? ""));
  const b = Buffer.from(String(expected ?? ""));
  return a.length === b.length && crypto.timingSafeEqual(a, b);
}

async function readBody(req) {
  let total = 0;
  const chunks = [];
  for await (const chunk of req) {
    total += chunk.length;
    if (total > MAX_REQUEST_BYTES) {
      throw Object.assign(new Error("Dashboard request is too large."), { code: "REQUEST_TOO_LARGE", status: 413 });
    }
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

export async function startDashboardServer({
  dispatch,
  assetsDir,
  host = "127.0.0.1",
  port = 0,
  mode = "standalone-proxy"
}) {
  if (typeof dispatch !== "function") throw new TypeError("dispatch is required");
  const token = crypto.randomBytes(32).toString("hex");
  const [indexTemplate, appJs, stylesCss] = await Promise.all([
    fs.readFile(path.join(assetsDir, "index.html"), "utf8"),
    fs.readFile(path.join(assetsDir, "app.js"), "utf8"),
    fs.readFile(path.join(assetsDir, "styles.css"), "utf8")
  ]);
  const indexHtml = indexTemplate.replace("__RELAY_DASHBOARD_TOKEN__", token);

  const server = http.createServer(async (req, res) => {
    try {
      const url = new URL(req.url ?? "/", "http://localhost");

      if (req.method === "OPTIONS") {
        return send(res, 403, JSON.stringify(transportError("CROSS_ORIGIN_BLOCKED", "Cross-origin dashboard requests are not supported.")));
      }

      if (req.method === "GET" && url.pathname === "/") {
        return send(res, 200, indexHtml, "text/html; charset=utf-8");
      }
      if (req.method === "GET" && url.pathname === "/app.js") {
        return send(res, 200, appJs, "text/javascript; charset=utf-8");
      }
      if (req.method === "GET" && url.pathname === "/styles.css") {
        return send(res, 200, stylesCss, "text/css; charset=utf-8");
      }
      if (req.method === "GET" && url.pathname === "/healthz") {
        return send(res, 200, JSON.stringify({
          ok: true,
          component: "relay-dashboard-transport",
          mode,
          command_count: DASHBOARD_COMMANDS.length
        }));
      }

      if (req.method === "POST" && url.pathname === "/api/execute") {
        if (!secureEqual(req.headers["x-relay-dashboard-token"], token)) {
          return send(res, 401, JSON.stringify(transportError("DASHBOARD_UNAUTHORIZED", "Dashboard session token is missing or invalid.")));
        }
        if (!String(req.headers["content-type"] ?? "").toLowerCase().startsWith("application/json")) {
          return send(res, 415, JSON.stringify(transportError("UNSUPPORTED_MEDIA_TYPE", "Dashboard execution requires application/json.")));
        }

        const raw = await readBody(req);
        let request;
        try {
          request = JSON.parse(raw);
        } catch {
          return send(res, 400, JSON.stringify(transportError("BAD_REQUEST", "Dashboard request body must be valid JSON.")));
        }
        if (!request || typeof request.command !== "string") {
          return send(res, 400, JSON.stringify(transportError("BAD_REQUEST", "command is required.")));
        }
        if (!DASHBOARD_COMMAND_SET.has(request.command)) {
          return send(res, 403, JSON.stringify(transportError(
            "DASHBOARD_COMMAND_NOT_EXPOSED",
            "This read-only dashboard spike does not expose that command."
          )));
        }

        try {
          const result = await dispatch(request.command, request.arguments ?? {}, request.request_id);
          return send(res, 200, JSON.stringify(result));
        } catch (error) {
          return send(res, 503, JSON.stringify(transportError(
            error.code ?? "HOST_UNAVAILABLE",
            error.message ?? "RELAY host is unavailable."
          )));
        }
      }

      return send(res, 404, JSON.stringify(transportError("NOT_FOUND", "Dashboard route not found.")));
    } catch (error) {
      return send(res, error.status ?? 500, JSON.stringify(transportError(
        error.code ?? "DASHBOARD_INTERNAL",
        error.message ?? "Dashboard transport failed."
      )));
    }
  });

  server.maxHeadersCount = 48;
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });
  const address = server.address();
  const url = `http://${host}:${address.port}`;

  return {
    server,
    url,
    token,
    mode,
    commands: DASHBOARD_COMMANDS,
    close: () => new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
  };
}
