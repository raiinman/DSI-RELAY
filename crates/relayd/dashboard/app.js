const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);

const $ = (selector) => document.querySelector(selector);
const details = $("#details");
const button = $("#refresh");
let refreshing = false;

async function command(name, argumentsValue = {}) {
  const response = await fetch("/api/execute", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Relay-Dashboard-Token": token
    },
    body: JSON.stringify({ command: name, arguments: argumentsValue }),
    cache: "no-store"
  });
  const envelope = await response.json();
  if (!response.ok || !envelope.ok) {
    throw new Error(envelope.error?.code ?? "RELAY_UNAVAILABLE");
  }
  return envelope.result;
}

function renderChecks(checks) {
  const list = $("#health-checks");
  list.replaceChildren();
  const problems = (checks ?? []).filter((check) => check.status !== "pass");
  const item = document.createElement("li");
  item.textContent = problems.length
    ? `${problems.length} check${problems.length === 1 ? "" : "s"} need attention.`
    : "All checks passed.";
  list.append(item);
  const labels = {
    "core.process": "RELAY needs attention.",
    "storage.integrity": "Project records need attention.",
    "diagnostics.capture": "Diagnostics need attention.",
    "transport.local": "The local connection needs attention.",
    "adapter.dependencies.parse": "The dependency parser needs attention."
  };
  for (const check of problems) {
    const problem = document.createElement("li");
    problem.textContent = `${labels[check.id] ?? "Another component needs attention."} See Advanced details for the component code.`;
    list.append(problem);
  }
}

function renderProjects(projects) {
  const list = $("#projects");
  list.replaceChildren();
  $("#uefn-inspection").textContent = "Choose a project to inspect its indexed UEFN files. Editor and runtime remain untested.";
  $("#asset-inspection").textContent = "Choose an asset manifest for a project to check local source and export links. Creator apps remain untested.";
  $("#krita-inspection").textContent = "Choose an asset manifest for a project to inspect Krita formats. Native export remains untested.";
  $("#project-count").textContent = projects.length
    ? `${projects.length} project${projects.length === 1 ? "" : "s"} connected`
    : "No projects connected yet.";
  for (const project of projects) {
    const item = document.createElement("li");
    const name = document.createElement("strong");
    name.textContent = project.name || "Unnamed project";
    item.append(name);
    const inspect = document.createElement("button");
    inspect.type = "button";
    inspect.textContent = "Inspect UEFN files";
    inspect.addEventListener("click", async () => {
      const result = $("#uefn-inspection");
      result.textContent = "Checking indexed project files…";
      inspect.disabled = true;
      try {
        const inspection = await command("uefn.static.inspect", { project_id: project.id });
        const marker = inspection.marker_state === "present"
          ? "UEFN project marker found"
          : inspection.marker_state === "ambiguous"
            ? "Multiple UEFN project markers found"
            : "No UEFN project marker found";
        result.textContent = `${project.name || "Project"}: ${marker}. ${count(inspection.verse_source_count)} Verse sources, ${count(inspection.unreal_asset_count)} assets, ${count(inspection.unreal_map_count)} maps in the ready index. Editor and runtime remain untested.`;
      } catch (_problem) {
        result.textContent = "Static UEFN inspection is unavailable. Check that this project's index is ready, then try again.";
      } finally {
        inspect.disabled = false;
      }
    });
    item.append(inspect);
    const manifestInput = document.createElement("input");
    manifestInput.type = "file";
    manifestInput.accept = ".json,application/json";
    manifestInput.hidden = true;
    const validate = document.createElement("button");
    validate.type = "button";
    validate.textContent = "Validate assets";
    let manifestCommand = "assets.manifest.validate";
    validate.addEventListener("click", () => {
      manifestCommand = "assets.manifest.validate";
      manifestInput.click();
    });
    const krita = document.createElement("button");
    krita.type = "button";
    krita.textContent = "Inspect Krita assets";
    krita.addEventListener("click", () => {
      manifestCommand = "assets.krita.inspect";
      manifestInput.click();
    });
    manifestInput.addEventListener("change", async () => {
      const result = manifestCommand === "assets.krita.inspect" ? $("#krita-inspection") : $("#asset-inspection");
      const file = manifestInput.files?.[0];
      if (!file) return;
      if (file.size > 32768) {
        result.textContent = "This manifest is too large for the current local command. Use a smaller manifest.";
        return;
      }
      result.textContent = "Checking asset links…";
      validate.disabled = true;
      krita.disabled = true;
      try {
        const report = await command(manifestCommand, {
          project_id: project.id,
          manifest_json: await file.text()
        });
        result.textContent = manifestCommand === "assets.krita.inspect"
          ? `${project.name || "Project"}: ${count(report.krita_asset_count)} Krita assets inspected; ${count(report.krita_format_finding_count)} declared format findings. Native Krita export remains untested.`
          : `${project.name || "Project"}: ${count(report.asset_count)} assets checked; ${count(report.finding_count)} local finding${report.finding_count === 1 ? "" : "s"}. Blender, Krita, and UEFN execution remain untested.`;
      } catch (_problem) {
        result.textContent = "Asset validation is unavailable. Check the manifest and project root, then try again.";
      } finally {
        validate.disabled = false;
        krita.disabled = false;
        manifestInput.value = "";
      }
    });
    item.append(validate, krita, manifestInput);
    list.append(item);
  }
}

function renderActivity(history) {
  const list = $("#activity-list");
  list.replaceChildren();
  if (!history) {
    $("#activity-summary").textContent = "Recent activity is unavailable.";
    return;
  }
  const transactions = Array.isArray(history.transactions) ? history.transactions.slice(0, 10) : [];
  $("#activity-summary").textContent = transactions.length
    ? `${transactions.length} recent operation${transactions.length === 1 ? "" : "s"}`
    : "No recorded operations yet.";
  for (const transaction of transactions) {
    const item = document.createElement("li");
    const commandName = typeof transaction.command === "string" && /^[a-z0-9._-]{1,64}$/.test(transaction.command)
      ? transaction.command : "Operation";
    const state = transaction.state === "verified" ? "verified"
      : transaction.state === "failed" ? "failed"
        : transaction.state === "pending" ? "pending" : "recorded";
    item.textContent = `${commandName}: ${state}`;
    list.append(item);
  }
}

function renderResults(listing) {
  const list = $("#results-list");
  list.replaceChildren();
  if (!listing) {
    $("#results-summary").textContent = "Stored results are unavailable.";
    return;
  }
  const results = Array.isArray(listing.results) ? listing.results.slice(0, 10) : [];
  $("#results-summary").textContent = results.length
    ? `${results.length} recent result${results.length === 1 ? "" : "s"}`
    : "No stored results yet.";
  for (const result of results) {
    const item = document.createElement("li");
    const kind = typeof result.kind === "string" && /^[A-Za-z0-9._-]{1,64}$/.test(result.kind)
      ? result.kind : "Result";
    const version = typeof result.producer_version === "string" && /^[A-Za-z0-9._+-]{1,64}$/.test(result.producer_version)
      ? result.producer_version : "unknown version";
    item.textContent = `${kind} · ${count(result.payload_bytes)} bytes · RELAY ${version}`;
    list.append(item);
  }
}

function renderUsage(usage) {
  if (!usage) {
    $("#usage-summary").textContent = "Usage information is unavailable.";
    $("#usage-detail").textContent = "";
    return;
  }
  $("#usage-summary").textContent = `${count(usage.command_count)} commands run · ${count(usage.failure_count)} failed`;
  $("#usage-detail").textContent = `${count(usage.remote_calls)} remote calls · ${count(usage.model_tokens_in)} input tokens · ${count(usage.model_tokens_out)} output tokens`;
}

function count(value) {
  return Number.isSafeInteger(value) && value >= 0 ? value.toLocaleString("en-US") : "Unknown";
}

function duration(milliseconds) {
  if (!Number.isSafeInteger(milliseconds) || milliseconds < 0) return "Unknown";
  const minutes = Math.floor(milliseconds / 60000);
  if (minutes < 1) return "Less than a minute";
  const hours = Math.floor(minutes / 60);
  if (hours < 1) return `${minutes} minute${minutes === 1 ? "" : "s"}`;
  const days = Math.floor(hours / 24);
  if (days < 1) return `${hours} hour${hours === 1 ? "" : "s"}`;
  return `${days} day${days === 1 ? "" : "s"}`;
}

function renderDiagnostics(status, doctor, summary) {
  const capture = status.diagnostics;
  $("#diagnostic-capture").textContent = capture?.ok === true
    ? "Working" : capture?.ok === false ? "Needs attention" : "Unknown";
  $("#diagnostic-detail").textContent = capture?.detail_active === true
    ? "Extra detail temporarily on" : capture?.detail_active === false ? "Standard" : "Unknown";
  $("#diagnostic-storage").textContent = Number.isSafeInteger(capture?.current_bytes)
    ? `${count(capture.current_bytes)} bytes in current file · ${count(capture.rotated_files)} older files`
    : "Unknown";
  $("#diagnostic-dropped").textContent = count(capture?.evicted_events);
  $("#diagnostic-events").textContent = summary.available ? count(summary.events?.total) : "Unavailable";
  $("#diagnostic-incomplete").textContent = summary.available ? count(summary.events?.incomplete) : "Unavailable";
  $("#relay-version").textContent = typeof status.version === "string" ? status.version : "Unknown";
  $("#relay-uptime").textContent = duration(status.uptime_ms);
  $("#diagnostic-guidance").textContent = capture?.ok === false
    ? "Diagnostic capture needs attention. Use RELAY's doctor command for the next step."
    : doctor.healthy === false
      ? "A component needs attention. Review Health and the suggested next step above."
      : "Diagnostic capture is available. A full workflow run must be checked separately.";
  details.textContent = JSON.stringify({
    relay_version: typeof status.version === "string" ? status.version : null,
    protocol_version: Number.isSafeInteger(status.protocol?.negotiated) ? status.protocol.negotiated : null,
    recovery_state: status.recovery_state === "Healthy" || status.recovery_state === "Degraded"
      ? status.recovery_state : "Unknown",
    diagnostic_capture: capture?.ok === true ? "working" : capture?.ok === false ? "needs_attention" : "unknown",
    diagnostic_detail_active: capture?.detail_active === true,
    diagnostic_evicted_events: Number.isSafeInteger(capture?.evicted_events) ? capture.evicted_events : null,
    diagnostic_events: summary.available && Number.isSafeInteger(summary.events?.total) ? summary.events.total : null,
    diagnostic_incomplete: summary.available && Number.isSafeInteger(summary.events?.incomplete) ? summary.events.incomplete : null,
    diagnostic_error_code: typeof summary.error_code === "string" && /^[A-Z0-9_]{1,64}$/.test(summary.error_code)
      ? summary.error_code : null,
    checks: (doctor.checks ?? []).map((check) => ({
      id: typeof check.id === "string" && /^[a-z0-9._-]{1,64}$/.test(check.id) ? check.id : "unknown",
      status: check.status === "pass" || check.status === "fail" ? check.status : "unknown"
    }))
  }, null, 2);
}

function clearDiagnostics() {
  for (const selector of [
    "#diagnostic-capture", "#diagnostic-detail", "#diagnostic-storage",
    "#diagnostic-dropped", "#diagnostic-events", "#diagnostic-incomplete",
    "#relay-version", "#relay-uptime"
  ]) $(selector).textContent = "Unavailable";
  $("#diagnostic-guidance").textContent = "Current diagnostic information is unavailable.";
  details.textContent = "No current details available.";
}

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  button.setAttribute("aria-disabled", "true");
  const message = $("#status-message");
  message.textContent = "Checking RELAY…";
  message.dataset.state = "Checking";
  try {
    if (!token) throw new Error("Open a fresh dashboard link from RELAY, then try again.");
    const [status, doctor, projectList, summary] = await Promise.all([
      command("system.status"), command("system.doctor"), command("project.list"), command("diagnostics.summary")
    ]);
    message.textContent = status.recovery_state === "Healthy"
      ? "RELAY is working."
      : "RELAY needs attention. See Health below.";
    message.dataset.state = status.recovery_state;
    $("#health-summary").textContent = doctor.healthy
      ? "All basic checks are working."
      : "One or more checks need attention.";
    renderChecks(doctor.checks);
    const nextAction = $("#next-action");
    nextAction.hidden = !doctor.next_action;
    nextAction.textContent = doctor.next_action ? `Suggested next step: ${doctor.next_action}` : "";
    renderProjects(projectList.projects ?? []);
    renderDiagnostics(status, doctor, summary);
    const [usage, history, results] = await Promise.allSettled([
      command("usage.summary"), command("transaction.list", { limit: 10 }), command("result.list", { limit: 10 })
    ]);
    renderUsage(usage.status === "fulfilled" ? usage.value : null);
    renderActivity(history.status === "fulfilled" ? history.value : null);
    renderResults(results.status === "fulfilled" ? results.value : null);
  } catch (problem) {
    message.textContent = !token || problem.message === "DASHBOARD_UNAUTHORIZED"
      ? "Open a fresh dashboard link from RELAY, then try again."
      : "Could not update the dashboard. Check that RELAY is running, then refresh this page.";
    message.dataset.state = "Degraded";
    $("#health-summary").textContent = "Current health is unavailable.";
    $("#health-checks").replaceChildren();
    $("#next-action").hidden = true;
    $("#next-action").textContent = "";
    $("#project-count").textContent = "Current projects are unavailable.";
    $("#projects").replaceChildren();
    $("#uefn-inspection").textContent = "Current project information is unavailable.";
    $("#asset-inspection").textContent = "Current project information is unavailable.";
    $("#krita-inspection").textContent = "Current project information is unavailable.";
    $("#uefn-connection").textContent = "UEFN editor connection has not been checked.";
    renderUsage(null);
    renderActivity(null);
    renderResults(null);
    clearDiagnostics();
  } finally {
    button.setAttribute("aria-disabled", "false");
    refreshing = false;
  }
}

$("#refresh").addEventListener("click", refresh);
$("#uefn-connect").addEventListener("click", async () => {
  const button = $("#uefn-connect");
  const result = $("#uefn-connection");
  button.disabled = true;
  result.textContent = "Checking the local UEFN editor endpoint…";
  try {
    const discovery = await command("uefn.mcp.discover");
    result.textContent = discovery.state === "discovered"
      ? `A local MCP endpoint responded with ${count(discovery.tool_count)} tools. UEFN editor identity and live workflows remain untested.`
      : "No supported local UEFN MCP connection is available. Check that UEFN is open with MCP enabled.";
  } catch (_problem) {
    result.textContent = "UEFN connection check is unavailable. Check that RELAY is running, then try again.";
  } finally {
    button.disabled = false;
  }
});
refresh();
