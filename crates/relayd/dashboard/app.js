const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);

const $ = (selector) => document.querySelector(selector);
const details = $("#details");
const button = $("#refresh");
let refreshing = false;
let projectActionBusy = false;
let selectedProjectId = null;
let assetActionBusy = false;
let automationActionBusy = false;
let automationMode = "unknown";
let currentProjects = [];
let uefnAvailability = "not_checked";
let setupEpoch = 0;

async function command(name, argumentsValue = {}) {
  const body = JSON.stringify({ command: name, arguments: argumentsValue });
  if (new TextEncoder().encode(body).byteLength > 32768) throw new Error("DASHBOARD_REQUEST_TOO_LARGE");
  const response = await fetch("/api/execute", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Relay-Dashboard-Token": token
    },
    body,
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
  currentProjects = projects;
  const list = $("#projects");
  list.replaceChildren();
  if (!projects.some((project) => project.id === selectedProjectId && project.lifecycle_state !== "removed")) selectedProjectId = null;
  const selected = projects.find((project) => project.id === selectedProjectId);
  $("#selected-project").textContent = selected
    ? `Selected: ${selected.name || "Unnamed project"}${selected.lifecycle_state === "archived" ? " (archived)" : ""}`
    : "No project selected.";
  $("#uefn-inspection").textContent = "Choose a project to inspect its indexed UEFN files. Editor and runtime remain untested.";
  $("#asset-selected").textContent = selected?.lifecycle_state === "active" || (selected && !selected.lifecycle_state)
    ? `Checking assets for ${selected.name || "Unnamed project"}.` : "Select an active project to check its asset manifest.";
  $("#asset-validate").disabled = !selected || selected.lifecycle_state === "archived";
  $("#krita-validate").disabled = !selected || selected.lifecycle_state === "archived";
  $("#asset-impact").disabled = !selected || selected.lifecycle_state === "archived";
  $("#asset-manifest").value = "";
  $("#asset-changed-path").value = "";
  $("#asset-result").textContent = "No manifest checked yet.";
  $("#asset-lineage").textContent = "Declared source and export lineage has not been checked.";
  $("#asset-findings").replaceChildren();
  const activeCount = projects.filter((project) => project.lifecycle_state !== "archived" && project.lifecycle_state !== "removed").length;
  const archivedCount = projects.filter((project) => project.lifecycle_state === "archived").length;
  $("#project-count").textContent = projects.length
    ? `${activeCount} active · ${archivedCount} archived`
    : "No projects connected yet.";
  for (const project of projects) {
    const item = document.createElement("li");
    const name = document.createElement("strong");
    name.textContent = project.name || "Unnamed project";
    item.append(name);
    if (project.lifecycle_state === "removed") {
      const state = document.createElement("span");
      state.textContent = "Removed from RELAY";
      item.append(state);
      list.append(item);
      continue;
    }
    if (project.lifecycle_state === "archived") {
      const state = document.createElement("span");
      state.textContent = "Archived";
      item.append(state);
    }
    const select = document.createElement("button");
    select.type = "button";
    select.textContent = project.id === selectedProjectId ? "Selected" : "Select";
    select.disabled = project.id === selectedProjectId;
    select.addEventListener("click", async () => {
      selectedProjectId = project.id;
      renderProjects(projects);
      $("#project-action").textContent = `${project.name || "Project"} selected.`;
      await Promise.all([loadTestsForSelection(), updateSetup(projects)]);
    });
    item.append(select);
    if (project.id !== selectedProjectId) {
      list.append(item);
      continue;
    }
    if (project.lifecycle_state === "active" || !project.lifecycle_state) {
    const build = document.createElement("button");
    build.id = "project-index-action";
    build.type = "button";
    build.textContent = "Build file index";
    build.addEventListener("click", async () => {
      if (projectActionBusy || !window.confirm(`Build the file index for ${project.name || "this project"}?`)) return;
      projectActionBusy = true;
      build.disabled = true;
      $("#project-action").textContent = "Building the local file index…";
      try {
        const report = await command("project.index.build", { project_id: project.id });
        $("#project-action").textContent = `${project.name || "Project"}: ${count(report.file_count)} files indexed.`;
        await updateSetup();
      } catch (problem) {
        $("#project-action").textContent = projectError(problem);
      } finally {
        projectActionBusy = false;
        build.disabled = false;
      }
    });
    item.append(build);
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
    const archive = document.createElement("button");
    archive.type = "button";
    archive.textContent = "Archive";
    archive.addEventListener("click", () => changeProjectLifecycle(project, "project.archive", archive));
    item.append(archive);
    }
    if (project.lifecycle_state === "archived") {
      const restore = document.createElement("button");
      restore.type = "button";
      restore.textContent = "Restore";
      restore.addEventListener("click", () => changeProjectLifecycle(project, "project.restore", restore));
      item.append(restore);
    }
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "Remove";
    remove.addEventListener("click", () => changeProjectLifecycle(project, "project.remove", remove));
    item.append(remove);
    list.append(item);
  }
}

function showSetup(progress, guidance, next, actionLabel, target, ready = false) {
  $("#setup-progress").textContent = progress;
  $("#setup-guidance").textContent = guidance;
  $("#setup-next").textContent = next;
  const action = $("#setup-action");
  action.hidden = !actionLabel;
  if (actionLabel) {
    action.textContent = actionLabel;
    action.href = target;
  }
  $("#setup-handoff").hidden = !ready;
}

function renderUefnGuidance() {
  $("#setup-uefn").textContent = uefnAvailability === "discovered"
    ? "Last check: a local MCP endpoint responded. UEFN editor identity and live play-session workflows remain UNTESTED."
    : uefnAvailability === "unavailable"
      ? "Last check: UEFN connection was unavailable. Open UEFN, enable its MCP connection, then retry Check UEFN connection. Live workflows remain UNTESTED."
      : "UEFN connection has not been checked. For UEFN work, open the editor and use Check UEFN connection. Live workflows remain UNTESTED.";
}

async function updateSetup(projects = currentProjects) {
  const epoch = ++setupEpoch;
  const active = projects.filter((project) => project.lifecycle_state === "active" || !project.lifecycle_state);
  if (!active.length) {
    showSetup("Step 1 of 3 · Add a project",
      "Choose an existing local project folder. RELAY keeps its files in place.",
      "Next: enter a project name and folder below.", "Add a project", "#project-name");
    return;
  }
  const selected = active.find((project) => project.id === selectedProjectId);
  if (!selected) {
    showSetup("Step 2 of 3 · Select a project",
      "Choose which active project RELAY should show in Assets and Tests.",
      "Next: select an active project below.", "Choose a project", "#projects");
    return;
  }
  const projectId = selected.id;
  showSetup("Step 3 of 3 · Check local files",
    "RELAY is checking whether this project's local file index is ready.",
    "Next: wait for the index check.", null, null);
  try {
    const capability = await command("project.capabilities", { project_id: projectId });
    if (epoch !== setupEpoch || selectedProjectId !== projectId) return;
    if (capability.index_status === "unavailable" || capability.baseline_state === "unavailable") {
      showSetup("Step 3 of 3 · Project folder unavailable",
        "RELAY cannot open this project's saved folder. Put it back at its original location or add the folder as a new project.",
        "Next: restore the folder, then refresh.", "Refresh after restoring", "#refresh");
    } else if (capability.index_status === "ready" && capability.content_verification_required !== true) {
      showSetup("Local setup ready",
        "RELAY can use this project's local file index. Creator apps and live UEFN workflows still need their own checks.",
        "Next: open Assets to check a local manifest.", "Open Assets", "#assets-title", true);
    } else {
      showSetup("Step 3 of 3 · Build local file index",
        capability.index_status === "stale" || capability.content_verification_required === true
          ? "The local file index needs a fresh build before RELAY can rely on it."
          : "RELAY needs to build a local file index for this project.",
        "Next: build the file index below.", "Build file index", "#project-index-action");
    }
  } catch (_problem) {
    if (epoch === setupEpoch && selectedProjectId === projectId) showSetup("Step 3 of 3 · Index check unavailable",
      "RELAY could not check this project's local index. Review Health and try again.",
      "Next: refresh RELAY's current state.", "Refresh", "#refresh");
  }
}

function findingLabel(code) {
  const labels = {
    MISSING_FILE: "Missing source or export",
    INACCESSIBLE_FILE: "Source or export inaccessible",
    LINKED_PATH: "Linked path needs review",
    PATH_OUTSIDE_PROJECT: "Link outside project",
    MISSING_LINEAGE: "Missing declared lineage",
    INCOMPLETE_LINEAGE: "Incomplete declared lineage",
    STALE_EXPORT: "Export revision differs from source",
    UNKNOWN_RELATED_ASSET: "Related asset was not declared",
    UNSUPPORTED_SOURCE_FORMAT: "Unsupported Krita source format",
    UNSUPPORTED_EXPORT_FORMAT: "Unsupported Krita export format"
  };
  return labels[code] ?? "Another local finding";
}

function renderAssetReport(report, krita) {
  const findings = krita
    ? [...(Array.isArray(report.generic_findings) ? report.generic_findings : []),
       ...(Array.isArray(report.format_findings) ? report.format_findings : [])]
    : (Array.isArray(report.findings) ? report.findings : []);
  const total = krita
    ? (Number.isSafeInteger(report.generic_finding_count) ? report.generic_finding_count : 0)
      + (Number.isSafeInteger(report.krita_format_finding_count) ? report.krita_format_finding_count : 0)
    : report.finding_count;
  const assetCount = krita ? report.krita_asset_count : report.asset_count;
  $("#asset-result").textContent = `${count(assetCount)} ${krita ? "Krita " : ""}assets checked · ${count(total)} local findings. Native app execution: UNTESTED.`;
  const lineageCodes = new Set(["MISSING_LINEAGE", "INCOMPLETE_LINEAGE", "STALE_EXPORT", "UNKNOWN_RELATED_ASSET"]);
  const lineageCount = findings.filter((finding) => lineageCodes.has(finding.code)).length;
  const truncated = report.findings_truncated === true || report.generic_findings_truncated === true || report.format_findings_truncated === true || findings.length < total;
  $("#asset-lineage").textContent = `${lineageCount} visible declared-lineage finding${lineageCount === 1 ? "" : "s"}${truncated ? "; more findings may be omitted" : ""}. Source and export contents were not opened by a creator app.`;
  const list = $("#asset-findings");
  list.replaceChildren();
  for (const finding of findings.slice(0, 5)) {
    const item = document.createElement("li");
    item.textContent = findingLabel(finding.code);
    list.append(item);
  }
  if (total > 5) {
    const item = document.createElement("li");
    item.textContent = `${count(total - 5)} additional finding${total - 5 === 1 ? "" : "s"} not shown here.`;
    list.append(item);
  }
}

async function validateAssets(name) {
  if (assetActionBusy || !selectedProjectId) return;
  const file = $("#asset-manifest").files?.[0];
  if (!file) {
    $("#asset-result").textContent = "Choose a local JSON asset manifest first.";
    return;
  }
  if (file.size > 24576) {
    $("#asset-result").textContent = "This manifest is too large for the local dashboard. Use the RELAY CLI for larger manifests.";
    return;
  }
  const projectId = selectedProjectId;
  const changedPath = $("#asset-changed-path").value.trim();
  if (name === "assets.impact.analyze" && (!changedPath || changedPath.length > 512 ||
      /^[a-z]:|^[/\\]|\\|(^|\/)\.\.(\/|$)/i.test(changedPath))) {
    $("#asset-result").textContent = "Enter one safe project-relative changed file, such as Assets/example.png.";
    return;
  }
  assetActionBusy = true;
  $("#asset-validate").disabled = true;
  $("#krita-validate").disabled = true;
  $("#asset-impact").disabled = true;
  $("#asset-result").textContent = name === "assets.impact.analyze" ? "Checking affected assets…" : "Checking local asset links…";
  try {
    const manifest = await file.text();
    if (selectedProjectId !== projectId) return;
    const args = { project_id: projectId, manifest_json: manifest };
    if (name === "assets.impact.analyze") args.changed_paths = [changedPath];
    const report = await command(name, args);
    if (selectedProjectId === projectId) {
      if (name === "assets.impact.analyze") {
        $("#asset-result").textContent = `${count(report.affected_asset_count)} declared assets affected by ${count(report.changed_path_count)} changed file. Creator apps: UNTESTED.`;
        $("#asset-lineage").textContent = "Impact follows declared asset relationships. Review the source and exports in their creator apps before shipping.";
        const labels = { source_changed: "Source changed", export_changed: "Export changed", project_target_changed: "Project target changed", related_dependency: "Related asset dependency" };
        const reasons = new Set((Array.isArray(report.assets) ? report.assets : []).flatMap((asset) =>
          (Array.isArray(asset.reasons) ? asset.reasons : []).map((reason) => reason.kind)).filter((kind) => labels[kind]));
        const list = $("#asset-findings");
        list.replaceChildren();
        for (const reason of reasons) { const item = document.createElement("li"); item.textContent = labels[reason]; list.append(item); }
      } else renderAssetReport(report, name === "assets.krita.inspect");
    }
  } catch (problem) {
    if (selectedProjectId === projectId) $("#asset-result").textContent = problem.message === "DASHBOARD_REQUEST_TOO_LARGE"
      ? "This manifest is too large for the local dashboard. Use the RELAY CLI for larger manifests."
      : problem.message === "ASSET_PROJECT_MISMATCH"
        ? "The manifest belongs to another project. Select the matching project and try again."
        : "Asset validation is unavailable. Check the manifest and project, then try again.";
  } finally {
    assetActionBusy = false;
    if (selectedProjectId === projectId) {
      $("#asset-validate").disabled = false;
      $("#krita-validate").disabled = false;
      $("#asset-impact").disabled = false;
    }
  }
}

async function loadTestsForSelection() {
  const projectId = selectedProjectId;
  const list = $("#tests-list");
  list.replaceChildren();
  if (!projectId) {
    $("#tests-summary").textContent = "Select a project to see imported Verse capture analyses.";
    return;
  }
  $("#tests-summary").textContent = "Loading stored analysis summaries…";
  try {
    const listing = await command("result.list", { project_id: projectId, limit: 100 });
    if (selectedProjectId !== projectId) return;
    const imported = (Array.isArray(listing.results) ? listing.results : [])
      .filter((result) => result.kind === "IMPORTED_VERSE_CAPTURE_ANALYSIS").slice(0, 10);
    $("#tests-summary").textContent = imported.length
      ? `${imported.length} recent imported Verse capture analys${imported.length === 1 ? "is" : "es"}. Live UEFN test status remains UNTESTED.`
      : "No imported Verse capture analyses for this project. Live UEFN test status: UNTESTED.";
    for (const result of imported) {
      const item = document.createElement("li");
      const version = typeof result.producer_version === "string" && /^[A-Za-z0-9._+-]{1,64}$/.test(result.producer_version)
        ? result.producer_version : "unknown version";
      item.textContent = `Imported capture analysis · RELAY ${version} · live UEFN UNTESTED`;
      list.append(item);
    }
  } catch (_problem) {
    if (selectedProjectId === projectId) $("#tests-summary").textContent = "Stored test summaries are unavailable. Live UEFN test status: UNTESTED.";
  }
}

async function changeProjectLifecycle(project, name, button) {
  if (projectActionBusy) return;
  const label = project.name || "this project";
  const warning = name === "project.archive"
    ? `Archive ${label}? Its files stay on this computer and its RELAY records are kept.`
    : name === "project.restore"
      ? `Restore ${label} to active projects?`
      : `Remove ${label} from RELAY? Its files and RELAY history are kept, but this project will no longer be active.`;
  if (!window.confirm(warning)) return;
  projectActionBusy = true;
  button.disabled = true;
  $("#project-action").textContent = name === "project.archive" ? "Archiving the project…"
    : name === "project.restore" ? "Restoring the project…" : "Removing the project…";
  try {
    const argumentsValue = { project_id: project.id };
    if (name === "project.remove") argumentsValue.confirm_project_id = project.id;
    const result = await command(name, argumentsValue);
    selectedProjectId = null;
    const updated = await refresh();
    const action = result.lifecycle_state === "archived" ? "archived" : result.lifecycle_state === "removed" ? "removed from RELAY" : result.lifecycle_state === "active" ? "restored" : "updated";
    $("#project-action").textContent = updated
      ? `${label} ${action}. Project files and RELAY history were kept.`
      : `${label} ${action}, but the project list could not be refreshed. Try Refresh.`;
  } catch (problem) {
    $("#project-action").textContent = projectError(problem);
  } finally {
    projectActionBusy = false;
    button.disabled = false;
  }
}

function projectError(problem) {
  if (problem.message === "DASHBOARD_UNAUTHORIZED") return "Open a fresh dashboard link from RELAY, then try again.";
  if (problem.message === "PROJECT_PATH_INVALID" || problem.message === "PROJECT_PATH_NOT_DIRECTORY") {
    return "That folder is unavailable. Enter an existing local project folder and try again.";
  }
  if (problem.message === "PROJECT_FILE_UNREADABLE" || problem.message === "PROJECT_PATH_ESCAPE") {
    return "Some project files could not be indexed safely. Check the folder and try again.";
  }
  return "The project action could not finish. Check RELAY health, then try again.";
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

function renderJobs(listing) {
  const list = $("#jobs-list");
  list.replaceChildren();
  if (!listing) {
    $("#jobs-summary").textContent = "Jobs are unavailable.";
    return;
  }
  const jobs = Array.isArray(listing.jobs) ? listing.jobs.slice(0, 10) : [];
  $("#jobs-summary").textContent = jobs.length
    ? `${jobs.length} recent job${jobs.length === 1 ? "" : "s"}`
    : "No saved jobs yet.";
  for (const job of jobs) {
    const item = document.createElement("li");
    const commandName = typeof job.command === "string" && /^[a-z0-9._-]{1,64}$/.test(job.command)
      ? job.command : "Job";
    const state = typeof job.state === "string" && /^[A-Za-z0-9._-]{1,64}$/.test(job.state)
      ? job.state : "unknown state";
    item.textContent = `${commandName}: ${state}`;
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

function renderAutomation(mode) {
  automationMode = ["running", "paused", "unavailable"].includes(mode) ? mode : "unknown";
  const control = $("#automation-toggle");
  control.disabled = automationActionBusy || !["running", "paused"].includes(automationMode);
  control.textContent = automationMode === "paused" ? "Resume background checks" : "Pause background checks";
  $("#automation-status").textContent = automationMode === "running"
    ? "Background file checks are running."
    : automationMode === "paused" ? "Background file checks are paused for this RELAY session."
      : "Background file checks are unavailable. Local commands still work.";
}

async function toggleAutomation() {
  if (automationActionBusy || !["running", "paused"].includes(automationMode)) return;
  const next = automationMode === "running" ? "automation.pause" : "automation.resume";
  automationActionBusy = true;
  renderAutomation(automationMode);
  $("#automation-status").textContent = next === "automation.pause" ? "Pausing background checks…" : "Resuming background checks…";
  try {
    const result = await command(next);
    const expectedMode = next === "automation.pause" ? "paused" : "running";
    if (result.mode === expectedMode || result.mode === "unavailable") {
      renderAutomation(result.mode);
    } else {
      renderAutomation("unknown");
      $("#automation-status").textContent = "Could not confirm whether background checks changed. Refresh RELAY before trying again.";
    }
  } catch (_problem) {
    renderAutomation("unknown");
    $("#automation-status").textContent = "Could not confirm whether background checks changed. Refresh RELAY before trying again.";
  } finally {
    automationActionBusy = false;
    $("#automation-toggle").disabled = !["running", "paused"].includes(automationMode);
  }
}

async function previewIntegratedReport() {
  const summary = $("#integrated-summary");
  const list = $("#integrated-workflows");
  list.replaceChildren();
  const file = $("#integrated-report").files?.[0];
  if (!file) { summary.textContent = "No integrated report selected. Workflow tests have not been verified here."; return; }
  if (file.size > 1048576) { summary.textContent = "The selected report is too large to preview."; return; }
  try {
    const report = JSON.parse(await file.text());
    const statuses = ["passed", "failed", "blocked", "untested"];
    if (report.schema_version !== 1 || !statuses.includes(report.overall_status) ||
        !Array.isArray(report.workflows) || report.workflows.length < 1 || report.workflows.length > 256 ||
        report.workflows.some((item) => typeof item.workflow_id !== "string" ||
          !/^[a-z0-9._-]{1,96}$/.test(item.workflow_id) || !statuses.includes(item.status))) {
      throw new Error("INVALID_REPORT");
    }
    const counts = Object.fromEntries(statuses.map((status) => [status, report.workflows.filter((item) => item.status === status).length]));
    const expectedOverall = counts.failed ? "failed" : counts.blocked ? "blocked" : counts.untested ? "untested" : "passed";
    if (new Set(report.workflows.map((item) => item.workflow_id)).size !== report.workflows.length ||
        expectedOverall !== report.overall_status ||
        statuses.some((status) => report.counts?.[status] !== counts[status]) ||
        report.workflows.some((item) => item.status === "passed" &&
          (item.evidence?.kind !== "observed" ||
            (Array.isArray(item.requirements) && item.requirements.some((entry) => entry.availability !== "available"))))) {
      throw new Error("INCONSISTENT_REPORT");
    }
    summary.textContent = `Selected report preview: ${counts.passed} passed · ${counts.failed} failed · ${counts.blocked} blocked · ${counts.untested} untested. File-provided outcomes are not verified by this dashboard.`;
    for (const workflow of report.workflows) {
      const item = document.createElement("li");
      const code = typeof workflow.reason_code === "string" && /^[A-Z][A-Z0-9_]{0,95}$/.test(workflow.reason_code)
        ? ` · ${workflow.reason_code}` : "";
      const details = [];
      if (Number.isSafeInteger(workflow.timing?.duration_ms) && workflow.timing.duration_ms >= 0) {
        details.push(`${count(workflow.timing.duration_ms)} ms`);
      }
      if (Number.isSafeInteger(workflow.resource_use?.peak_rss_bytes) && workflow.resource_use.peak_rss_bytes >= 0) {
        details.push(`${(workflow.resource_use.peak_rss_bytes / 1048576).toFixed(1)} MiB peak memory`);
      }
      if (Number.isSafeInteger(workflow.resource_use?.cpu_ms) && workflow.resource_use.cpu_ms >= 0) {
        details.push(`${count(workflow.resource_use.cpu_ms)} ms CPU`);
      }
      const diagnostics = Array.isArray(workflow.diagnostic_codes)
        ? workflow.diagnostic_codes.filter((value) => typeof value === "string" && /^[A-Z][A-Z0-9_]{0,95}$/.test(value)).slice(0, 4) : [];
      if (diagnostics.length) details.push(`Codes: ${diagnostics.join(", ")}`);
      const versions = Array.isArray(workflow.component_versions)
        ? workflow.component_versions.filter((value) => typeof value?.component_id === "string" &&
            /^[a-z0-9._-]{1,64}$/.test(value.component_id) && typeof value?.version === "string" &&
            /^[A-Za-z0-9_.:+-]{1,64}$/.test(value.version)).slice(0, 4) : [];
      if (versions.length) details.push(`Versions: ${versions.map((value) => `${value.component_id} ${value.version}`).join(", ")}`);
      const reproduction = workflow.reproduction;
      if (typeof reproduction?.command_id === "string" && /^[a-z0-9._-]{1,96}$/.test(reproduction.command_id) &&
          typeof reproduction?.scenario_ref === "string" && /^[a-z0-9._-]{1,96}$/i.test(reproduction.scenario_ref)) {
        details.push(`Retry: ${reproduction.command_id} · ${reproduction.scenario_ref}`);
      }
      if (typeof workflow.evidence?.log_ref === "string" && /^sha256:[a-f0-9]{64}$/.test(workflow.evidence.log_ref)) {
        details.push(`Journal: ${workflow.evidence.log_ref}`);
      }
      const transport = Array.isArray(workflow.transport_metrics)
        ? workflow.transport_metrics.filter((value) => ["cli_stdin", "local_mcp_http"].includes(value?.path_id) &&
            value.byte_scope === "application_json" &&
            [value.request_bytes, value.response_bytes, value.elapsed_ms].every((number) => Number.isSafeInteger(number) && number >= 0 && number <= 1048576)).slice(0, 4) : [];
      for (const metric of transport) {
        const pathLabel = metric.path_id === "cli_stdin" ? "CLI" : "local MCP";
        details.push(`${pathLabel}: ${count(metric.request_bytes)} in / ${count(metric.response_bytes)} out JSON bytes · ${count(metric.elapsed_ms)} ms`);
      }
      item.textContent = `${workflow.workflow_id}: ${workflow.status.toUpperCase()}${code}${details.length ? ` · ${details.join(" · ")}` : ""}`;
      list.append(item);
    }
  } catch (_problem) {
    summary.textContent = "This is not a supported privacy-safe integrated report. Choose a RELAY report JSON file.";
  }
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
      command("system.status"), command("system.doctor"), command("project.list", { include_inactive: true }), command("diagnostics.summary")
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
    await updateSetup(projectList.projects ?? []);
    renderUefnGuidance();
    renderDiagnostics(status, doctor, summary);
    renderAutomation(status.automation_mode);
    const [usage, history, results, jobs] = await Promise.allSettled([
      command("usage.summary"), command("transaction.list", { limit: 10 }),
      command("result.list", { limit: 10 }), command("job.list", { limit: 10 })
    ]);
    renderUsage(usage.status === "fulfilled" ? usage.value : null);
    renderActivity(history.status === "fulfilled" ? history.value : null);
    renderResults(results.status === "fulfilled" ? results.value : null);
    renderJobs(jobs.status === "fulfilled" ? jobs.value : null);
    await loadTestsForSelection();
    return true;
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
    currentProjects = [];
    selectedProjectId = null;
    setupEpoch++;
    $("#selected-project").textContent = "No current project selection available.";
    $("#uefn-inspection").textContent = "Current project information is unavailable.";
    $("#asset-selected").textContent = "Current project information is unavailable.";
    $("#asset-validate").disabled = true;
    $("#krita-validate").disabled = true;
    $("#asset-impact").disabled = true;
    $("#asset-manifest").value = "";
    $("#asset-changed-path").value = "";
    $("#asset-result").textContent = "Current asset information is unavailable.";
    $("#asset-lineage").textContent = "Declared lineage has not been checked.";
    $("#asset-findings").replaceChildren();
    $("#tests-summary").textContent = "Stored test summaries are unavailable. Live UEFN test status: UNTESTED.";
    $("#tests-list").replaceChildren();
    $("#uefn-connection").textContent = "UEFN editor connection has not been checked.";
    if (!token || problem.message === "DASHBOARD_UNAUTHORIZED") {
      showSetup("Connection needed",
        "This dashboard link is no longer available. Open a fresh dashboard link from RELAY.",
        "Next: reopen the dashboard from RELAY.", null, null);
    } else {
      showSetup("Connection needed",
        "RELAY is unavailable. Start RELAY or restore its local connection, then try again.",
        "Next: refresh after RELAY is running.", "Refresh", "#refresh");
    }
    $("#setup-uefn").textContent = "UEFN connection cannot be checked until RELAY is available. Live workflows remain UNTESTED.";
    renderUsage(null);
    renderActivity(null);
    renderResults(null);
    renderJobs(null);
    clearDiagnostics();
    renderAutomation("unknown");
    return false;
  } finally {
    button.setAttribute("aria-disabled", "false");
    refreshing = false;
  }
}

$("#refresh").addEventListener("click", refresh);
$("#asset-manifest").addEventListener("change", () => {
  $("#asset-result").textContent = $("#asset-manifest").files?.[0]
    ? "Manifest selected. Choose a local check below." : "No manifest checked yet.";
});
$("#asset-validate").addEventListener("click", () => validateAssets("assets.manifest.validate"));
$("#krita-validate").addEventListener("click", () => validateAssets("assets.krita.inspect"));
$("#asset-impact").addEventListener("click", () => validateAssets("assets.impact.analyze"));
$("#automation-toggle").addEventListener("click", toggleAutomation);
$("#integrated-report").addEventListener("change", previewIntegratedReport);
$("#project-add-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (projectActionBusy) return;
  const name = $("#project-name").value.trim();
  const rootPath = $("#project-root").value.trim();
  if (!name || !rootPath) {
    $("#project-action").textContent = "Enter a project name and an existing local folder.";
    return;
  }
  projectActionBusy = true;
  $("#project-add").disabled = true;
  $("#project-action").textContent = "Adding the project…";
  try {
    const imported = await command("project.import", { name, root_path: rootPath });
    selectedProjectId = imported.id;
    $("#project-name").value = "";
    $("#project-root").value = "";
    const updated = await refresh();
    $("#project-action").textContent = updated
      ? `${name} added and selected. Build its file index when ready.`
      : `${name} was added, but the project list could not be refreshed. Try Refresh.`;
  } catch (problem) {
    $("#project-action").textContent = projectError(problem);
  } finally {
    projectActionBusy = false;
    $("#project-add").disabled = false;
  }
});
$("#uefn-connect").addEventListener("click", async () => {
  const button = $("#uefn-connect");
  const result = $("#uefn-connection");
  button.disabled = true;
  result.textContent = "Checking the local UEFN editor endpoint…";
  try {
    const discovery = await command("uefn.mcp.toolsets");
    uefnAvailability = discovery.state === "discovered" ? "discovered" : "unavailable";
    result.textContent = discovery.state === "discovered"
      ? `A local MCP endpoint advertised ${count(discovery.toolset_count)} toolsets. UEFN editor identity and live workflows remain untested.`
      : "No supported local UEFN MCP connection is available. Check that UEFN is open with MCP enabled.";
  } catch (_problem) {
    uefnAvailability = "unavailable";
    result.textContent = "UEFN connection check is unavailable. Check that RELAY is running, then try again.";
  } finally {
    renderUefnGuidance();
    button.disabled = false;
  }
});
refresh();
