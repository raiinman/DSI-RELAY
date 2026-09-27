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
    "#details", "#refresh", "#status-message", "#health-summary", "#health-checks", "#setup-progress", "#setup-guidance", "#setup-next", "#setup-action", "#setup-handoff", "#setup-uefn",
    "#next-action", "#automation-status", "#automation-toggle", "#project-count", "#projects", "#project-add-form", "#project-name", "#project-root", "#project-add", "#project-action", "#selected-project", "#uefn-connect", "#uefn-connection",
    "#uefn-inspection", "#asset-selected", "#asset-manifest", "#asset-changed-path", "#asset-validate", "#krita-validate", "#asset-impact", "#asset-result", "#asset-lineage", "#asset-findings", "#tests-summary", "#tests-list",
    "#activity-summary", "#activity-list", "#results-summary", "#results-list", "#jobs-summary", "#jobs-list", "#usage-summary", "#usage-detail", "#diagnostic-capture",
    "#diagnostic-detail", "#diagnostic-storage", "#diagnostic-dropped",
    "#diagnostic-events", "#diagnostic-incomplete", "#relay-version",
    "#relay-uptime", "#diagnostic-guidance", "#integrated-report", "#integrated-summary", "#integrated-workflows"
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
    const request = JSON.parse(options.body);
    return fetchResult(request.command, request.arguments);
  };
  vm.runInNewContext(script, { document, fetch, history, location, TextEncoder, window: { confirm: () => true } });
  await new Promise(setImmediate);
  return {
    nodes, calls, location,
    refresh: () => nodes["#refresh"].listener(),
    setResults(next) { fetchResult = next; }
  };
}

function success(command) {
  const results = {
    "system.status": {
      recovery_state: "Degraded", version: "0.1.0", uptime_ms: 125000, automation_mode: "running",
      diagnostics: { ok: true, detail_active: false, current_bytes: 80, rotated_files: 1, evicted_events: 2 }
    },
    "system.doctor": {
      summary: "A local component needs attention.",
      checks: [
        { id: "storage.integrity", status: "fail", detail: "raw diagnostic" },
        { id: "transport.local", status: "pass", detail: "local transport healthy" }
      ],
      next_action: "Inspect operational storage."
    },
    "project.list": { projects: [{ name: "<script>unsafe</script>", id: "private-id" }] },
    "diagnostics.summary": {
      available: true, events: { total: 12, incomplete: 1 }, error_code: null
    },
    "usage.summary": { command_count: 8, failure_count: 1, remote_calls: 0, model_tokens_in: 0, model_tokens_out: 0 },
    "transaction.list": { transactions: [{ command: "project.register", state: "verified" }] },
    "result.list": { results: [{ kind: "UEFN_STATIC", payload_bytes: 2048, producer_version: "0.1.0" }] },
    "job.list": { jobs: [{ command: "fixture.work", state: "CHECKPOINTED" }] }
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
    "system.status", "system.doctor", "project.list", "diagnostics.summary",
    "usage.summary", "transaction.list", "result.list", "job.list"
  ]);
  assert.ok(page.calls.every((call) => call.headers["X-Relay-Dashboard-Token"] === "local-test-token"));
  assert.equal(page.nodes["#status-message"].textContent, "RELAY needs attention. See Health below.");
  assert.equal(page.nodes["#health-checks"].children[1].textContent,
    "Project records need attention. See Advanced details for the component code.");
  assert.equal(page.nodes["#projects"].children[0].children[0].textContent, "<script>unsafe</script>");
  assert.equal(page.nodes["#selected-project"].textContent, "No project selected.");
  assert.equal(page.nodes["#setup-progress"].textContent, "Step 2 of 3 · Select a project");
  assert.equal(page.nodes["#setup-action"].href, "#projects");
  assert.equal(page.nodes["#next-action"].hidden, false);
  assert.equal(page.nodes["#diagnostic-events"].textContent, "12");
  assert.equal(page.nodes["#diagnostic-incomplete"].textContent, "1");
  assert.equal(page.nodes["#activity-list"].children[0].textContent, "project.register: verified");
  assert.equal(page.nodes["#results-list"].children[0].textContent, "UEFN_STATIC · 2,048 bytes · RELAY 0.1.0");
  assert.equal(page.nodes["#jobs-list"].children[0].textContent, "fixture.work: CHECKPOINTED");
  assert.equal(page.nodes["#usage-summary"].textContent, "8 commands run · 1 failed");
  assert.doesNotMatch(page.nodes["#details"].textContent, /private-id|raw diagnostic|<script>/);
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
  assert.equal(page.nodes["#selected-project"].textContent, "No current project selection available.");
  assert.equal(page.nodes["#next-action"].hidden, true);
  assert.equal(page.nodes["#diagnostic-events"].textContent, "Unavailable");
  assert.equal(page.nodes["#activity-summary"].textContent, "Recent activity is unavailable.");
  assert.equal(page.nodes["#usage-summary"].textContent, "Usage information is unavailable.");
  assert.equal(page.nodes["#details"].textContent, "No current details available.");
  assert.equal(page.nodes["#setup-progress"].textContent, "Connection needed");
  assert.equal(page.nodes["#refresh"].attributes["aria-disabled"], "false");
});

test("expired dashboard token gives a plain next step", async () => {
  const page = await loadDashboard(async () => ({
    ok: false,
    json: async () => ({ ok: false, error: { code: "DASHBOARD_UNAUTHORIZED" } })
  }));
  assert.equal(page.nodes["#status-message"].textContent,
    "Open a fresh dashboard link from RELAY, then try again.");
  assert.equal(page.nodes["#setup-action"].hidden, true);
});

test("project import, selection, and index build use the shared command path", async () => {
  const projects = [{ id: "existing", name: "Existing" }];
  let indexBuilt = false;
  const page = await loadDashboard((command, argumentsValue) => {
    if (command === "project.import") {
      assert.equal(argumentsValue.name, "New project");
      assert.equal(argumentsValue.root_path, "C:\\Work\\Project");
      projects.push({ id: "new-id", name: "New project" });
      return { ok: true, json: async () => ({ ok: true, result: { id: "new-id" } }) };
    }
    if (command === "project.index.build") {
      assert.equal(argumentsValue.project_id, "new-id");
      indexBuilt = true;
      return { ok: true, json: async () => ({ ok: true, result: { file_count: 12 } }) };
    }
    if (command === "project.capabilities") {
      return { ok: true, json: async () => ({ ok: true, result: {
        index_status: indexBuilt ? "ready" : "missing", baseline_state: indexBuilt ? "ready" : "missing",
        content_verification_required: false
      } }) };
    }
    if (command === "project.list") {
      return { ok: true, json: async () => ({ ok: true, result: { projects } }) };
    }
    return success(command);
  });
  page.nodes["#project-name"].value = "New project";
  page.nodes["#project-root"].value = "C:\\Work\\Project";
  await page.nodes["#project-add-form"].listener({ preventDefault() {} });
  assert.equal(page.nodes["#selected-project"].textContent, "Selected: New project");
  assert.equal(page.nodes["#project-action"].textContent, "New project added and selected. Build its file index when ready.");
  assert.equal(page.nodes["#setup-progress"].textContent, "Step 3 of 3 · Build local file index");
  assert.equal(page.nodes["#setup-action"].href, "#project-index-action");
  await page.nodes["#projects"].children[1].children[2].listener();
  assert.equal(page.nodes["#project-action"].textContent, "New project: 12 files indexed.");
  assert.deepEqual(page.calls.slice(-2).map((call) => JSON.parse(call.body).command), ["project.index.build", "project.capabilities"]);
  assert.equal(page.nodes["#setup-progress"].textContent, "Local setup ready");
  assert.equal(page.nodes["#setup-action"].href, "#assets-title");
});

test("archive and remove confirm through shared commands and retain a visible tombstone", async () => {
  const projects = [{ id: "project-1", name: "Sample", lifecycle_state: "active" }];
  const page = await loadDashboard((command, argumentsValue) => {
    if (command === "project.list") {
      assert.equal(argumentsValue.include_inactive, true);
      return { ok: true, json: async () => ({ ok: true, result: { projects } }) };
    }
    if (command === "project.archive" || command === "project.restore" || command === "project.remove") {
      assert.equal(argumentsValue.project_id, "project-1");
      if (command === "project.remove") assert.equal(argumentsValue.confirm_project_id, "project-1");
      projects[0].lifecycle_state = command === "project.archive" ? "archived" : command === "project.restore" ? "active" : "removed";
      return { ok: true, json: async () => ({ ok: true, result: { project_id: "project-1", lifecycle_state: projects[0].lifecycle_state, changed: true } }) };
    }
    return success(command);
  });
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Archive").listener();
  assert.equal(page.nodes["#selected-project"].textContent, "No project selected.");
  assert.match(page.nodes["#project-action"].textContent, /Sample archived/);
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Restore").listener();
  assert.match(page.nodes["#project-action"].textContent, /Sample restored/);
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Remove").listener();
  assert.equal(page.nodes["#projects"].children[0].children[1].textContent, "Removed from RELAY");
  assert.match(page.nodes["#project-action"].textContent, /files and RELAY history were kept/);
});

test("asset manifest checks show bounded findings without revealing paths or contents", async () => {
  const manifest = '{"schema_version":1,"project_id":"private-id","assets":[],"private":"C:\\secret"}';
  const page = await loadDashboard((command, args) => {
    if (command === "assets.manifest.validate") {
      assert.equal(args.project_id, "private-id");
      assert.equal(args.manifest_json, manifest);
      return { ok: true, json: async () => ({ ok: true, result: {
        asset_count: 1, finding_count: 2, findings_truncated: false,
        findings: [
          { code: "MISSING_LINEAGE", asset_id: "C:\\secret", message: "raw path" },
          { code: "MISSING_FILE", asset_id: "private-source" }
        ]
      } }) };
    }
    return success(command);
  });
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  page.nodes["#asset-manifest"].files = [{ size: manifest.length, text: async () => manifest }];
  await page.nodes["#asset-validate"].listener();
  assert.match(page.nodes["#asset-result"].textContent, /1 assets checked · 2 local findings.*UNTESTED/);
  assert.match(page.nodes["#asset-lineage"].textContent, /1 visible declared-lineage finding/);
  assert.equal(page.nodes["#asset-findings"].children[0].textContent, "Missing declared lineage");
  assert.doesNotMatch(page.nodes["#asset-result"].textContent + page.nodes["#asset-findings"].children.map((item) => item.textContent).join(""), /secret|private-source|raw path/);
  assert.match(html, /Native Blender, Krita, and UEFN asset workflows: UNTESTED/);
});

test("asset impact uses the shared command and displays counts without asset paths", async () => {
  const page = await loadDashboard((command, args) => {
    if (command === "assets.impact.analyze") {
      assert.equal(args.project_id, "private-id");
      assert.equal(args.changed_paths[0], "Assets/changed.png");
      return { ok: true, json: async () => ({ ok: true, result: {
        changed_path_count: 1, affected_asset_count: 2,
        assets: [{ asset_id: "C:\\private", reasons: [{ kind: "source_changed", via_asset_id: "secret" }] }]
      } }) };
    }
    return success(command);
  });
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  page.nodes["#asset-manifest"].files = [{ size: 16, text: async () => '{"assets":[]}' }];
  page.nodes["#asset-changed-path"].value = "Assets/changed.png";
  await page.nodes["#asset-impact"].listener();
  assert.match(page.nodes["#asset-result"].textContent, /2 declared assets affected/);
  assert.equal(page.nodes["#asset-findings"].children[0].textContent, "Source changed");
  assert.doesNotMatch(page.nodes["#asset-result"].textContent + page.nodes["#asset-findings"].children[0].textContent, /private|secret/);
});

test("background control uses shared pause and resume commands", async () => {
  let mode = "running";
  const page = await loadDashboard((command) => {
    if (command === "system.status") {
      const response = success(command);
      return { ok: true, json: async () => ({ ok: true, result: { ...(await response.json()).result, automation_mode: mode } }) };
    }
    if (command === "automation.pause" || command === "automation.resume") {
      mode = command === "automation.pause" ? "paused" : "running";
      return { ok: true, json: async () => ({ ok: true, result: { mode } }) };
    }
    return success(command);
  });
  await page.nodes["#automation-toggle"].listener();
  assert.equal(page.nodes["#automation-status"].textContent, "Background file checks are paused for this RELAY session.");
  await page.nodes["#automation-toggle"].listener();
  assert.equal(page.nodes["#automation-status"].textContent, "Background file checks are running.");
  assert.deepEqual(page.calls.slice(-2).map((call) => JSON.parse(call.body).command), ["automation.pause", "automation.resume"]);
});

test("selected integrated report preview never renders arbitrary fields", async () => {
  const page = await loadDashboard(success);
  page.nodes["#integrated-report"].files = [{ size: 200, text: async () => JSON.stringify({
    schema_version: 1, overall_status: "untested", hidden_path: "C:\\secret",
    workflows: [{ workflow_id: "uefn.live_runtime", status: "untested", reason_code: "NOT_EXECUTED", raw_log: "private output" }]
  }) }];
  await page.nodes["#integrated-report"].listener();
  assert.match(page.nodes["#integrated-summary"].textContent, /1 untested.*not verified/);
  assert.equal(page.nodes["#integrated-workflows"].children[0].textContent, "uefn.live_runtime: UNTESTED · NOT_EXECUTED");
  assert.doesNotMatch(page.nodes["#integrated-summary"].textContent + page.nodes["#integrated-workflows"].children[0].textContent, /secret|private output/);
});

test("tests view uses selected-project metadata only and keeps live UEFN untested", async () => {
  const page = await loadDashboard((command, args) => {
    if (command === "result.list" && args.project_id) {
      assert.equal(args.project_id, "private-id");
      return { ok: true, json: async () => ({ ok: true, result: { results: [
        { kind: "IMPORTED_VERSE_CAPTURE_ANALYSIS", producer_version: "0.1.0", payload: "PRIVATE-LOG" },
        { kind: "UEFN_STATIC_AUDIT", producer_version: "0.1.0" }
      ] } }) };
    }
    return success(command);
  });
  await page.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  assert.match(page.nodes["#tests-summary"].textContent, /1 recent imported Verse capture analysis.*UNTESTED/);
  assert.equal(page.nodes["#tests-list"].children.length, 1);
  assert.equal(page.nodes["#tests-list"].children[0].textContent, "Imported capture analysis · RELAY 0.1.0 · live UEFN UNTESTED");
  assert.doesNotMatch(page.nodes["#tests-list"].children[0].textContent, /PRIVATE-LOG|private-id/);
});

test("guided setup gives one next action and safe recovery for unavailable dependencies", async () => {
  const empty = await loadDashboard((command) => command === "project.list"
    ? { ok: true, json: async () => ({ ok: true, result: { projects: [] } }) }
    : success(command));
  assert.equal(empty.nodes["#setup-progress"].textContent, "Step 1 of 3 · Add a project");
  assert.equal(empty.nodes["#setup-action"].href, "#project-name");
  assert.equal(empty.nodes["#setup-handoff"].hidden, true);

  const unavailable = await loadDashboard((command) => {
    if (command === "project.capabilities") return { ok: true, json: async () => ({ ok: true, result: {
      index_status: "unavailable", baseline_state: "unavailable"
    } }) };
    if (command === "uefn.mcp.toolsets") return { ok: true, json: async () => ({ ok: true, result: { state: "unavailable" } }) };
    return success(command);
  });
  await unavailable.nodes["#projects"].children[0].children.find((child) => child.textContent === "Select").listener();
  assert.equal(unavailable.nodes["#setup-progress"].textContent, "Step 3 of 3 · Project folder unavailable");
  assert.equal(unavailable.nodes["#setup-action"].href, "#refresh");
  assert.doesNotMatch(unavailable.nodes["#setup-guidance"].textContent, /private-id|<script>/);
  await unavailable.nodes["#uefn-connect"].listener();
  assert.match(unavailable.nodes["#setup-uefn"].textContent, /Open UEFN, enable its MCP connection.*UNTESTED/);
});
