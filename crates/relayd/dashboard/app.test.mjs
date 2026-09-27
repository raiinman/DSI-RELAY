import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");
const script = readFileSync(new URL("./app.js", import.meta.url), "utf8");

function element() {
  return {
    textContent: "",
    hidden: false,
    dataset: {},
    attributes: {},
    children: [],
    replaceChildren(...children) { this.children = children; },
    append(...children) { this.children.push(...children); },
    setAttribute(name, value) { this.attributes[name] = value; },
    addEventListener(_name, listener) { this.listener = listener; }
  };
}

async function loadDashboard(results, hash = "#local-test-token") {
  const selectors = [
    "#details", "#refresh", "#status-message", "#health-summary", "#health-checks",
    "#next-action", "#project-count", "#projects"
  ];
  const nodes = Object.fromEntries(selectors.map((selector) => [selector, element()]));
  const calls = [];
  let fetchResult = results;
  const location = { hash, pathname: "/" };
  const history = { replaceState(_state, _title, path) { location.hash = ""; location.pathname = path; } };
  const document = {
    querySelector(selector) { return nodes[selector]; },
    createElement() { return element(); }
  };
  const fetch = async (_url, options) => {
    calls.push(options);
    return fetchResult(JSON.parse(options.body).command);
  };
  vm.runInNewContext(script, { document, fetch, history, location });
  await new Promise(setImmediate);
  return {
    nodes, calls, location,
    refresh: () => nodes["#refresh"].listener(),
    setResults(next) { fetchResult = next; }
  };
}

function success(command) {
  const results = {
    "system.status": { recovery_state: "Degraded" },
    "system.doctor": {
      summary: "A local component needs attention.",
      checks: [
        { id: "storage.integrity", status: "fail", detail: "raw diagnostic" },
        { id: "transport.local", status: "pass", detail: "local transport healthy" }
      ],
      next_action: "Inspect operational storage."
    },
    "project.list": { projects: [{ name: "<script>unsafe</script>", id: "private-id" }] }
  };
  return { ok: true, json: async () => ({ ok: true, result: results[command] }) };
}

test("keyboard landmarks and live status have semantic markup", () => {
  assert.match(html, /href="#main">Skip to dashboard content<\/a>/);
  assert.match(html, /<main id="main" tabindex="-1">/);
  assert.match(html, /id="status-message" role="status" aria-atomic="true"/);
  assert.match(html, /<button id="refresh" type="button">Refresh<\/button>/);
});

test("refresh uses read-only commands and renders named failures as text", async () => {
  const page = await loadDashboard(success);
  assert.equal(page.location.hash, "");
  assert.deepEqual(page.calls.map((call) => JSON.parse(call.body).command), [
    "system.status", "system.doctor", "project.list"
  ]);
  assert.ok(page.calls.every((call) => call.headers["X-Relay-Dashboard-Token"] === "local-test-token"));
  assert.equal(page.nodes["#status-message"].textContent, "RELAY needs attention. See Health below.");
  assert.equal(page.nodes["#health-checks"].children[1].textContent,
    "Project records need attention. See Advanced details for the exact result.");
  assert.equal(page.nodes["#projects"].children[0].children[0].textContent, "<script>unsafe</script>");
  assert.equal(page.nodes["#next-action"].hidden, false);
  assert.equal(page.nodes["#refresh"].attributes["aria-disabled"], "false");
});

test("a failed refresh clears stale health, projects, and advanced output", async () => {
  const page = await loadDashboard(success);
  page.setResults(async () => { throw new Error("network failed"); });
  await page.refresh();
  assert.equal(page.nodes["#status-message"].textContent,
    "Could not update the dashboard. Check that RELAY is running, then refresh this page.");
  assert.equal(page.nodes["#health-checks"].children.length, 0);
  assert.equal(page.nodes["#projects"].children.length, 0);
  assert.equal(page.nodes["#next-action"].hidden, true);
  assert.equal(page.nodes["#details"].textContent, "No current details available.");
  assert.equal(page.nodes["#refresh"].attributes["aria-disabled"], "false");
});

test("expired dashboard token gives a plain next step", async () => {
  const page = await loadDashboard(async () => ({
    ok: false,
    json: async () => ({ ok: false, error: { code: "DASHBOARD_UNAUTHORIZED" } })
  }));
  assert.equal(page.nodes["#status-message"].textContent,
    "Open a fresh dashboard link from RELAY, then try again.");
});
