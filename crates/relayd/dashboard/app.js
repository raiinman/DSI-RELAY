const openedFromFile = location.protocol === "file:";
const token = openedFromFile ? "" : location.hash.slice(1);
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
let localTools = null;
let setupEpoch = 0;
let checkCatalog = null;
let checkCatalogMissingConfirmed = false;
let checkPlan = null;
let checkWorkflowBusy = false;
let checkWorkflowEpoch = 0;
let checkDisplayedProjectId = null;
let supportDownloadBusy = false;
let codexCandidates = [];
let codexAdded = new Set();

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
  const labels = {
    "core.process": ["Engine", "The RELAY background program responds.", "The background program needs attention."],
    "storage.integrity": ["Saved records", "The local database passed a quick integrity check.", "The local database needs inspection."],
    "diagnostics.capture": ["Debug recording", "RELAY can record bounded local diagnostics.", "Debug recording needs attention."],
    "transport.local": ["Local connection", "The engine reports its private local connection as available.", "The private local connection needs attention."],
    "adapter.dependencies.parse": ["Dependency parser", "No parser fault reported; it may be unconfigured.", "The optional dependency parser needs attention."]
  };
  for (const check of (Array.isArray(checks) ? checks : []).slice(0, 12)) {
    const [name, passExplanation, failExplanation] = labels[check.id] ??
      ["Other local component", "No fault reported. See Diagnostics for its code.", "See Diagnostics for its component code."];
    const item = document.createElement("li");
    const passed = check.status === "pass";
    item.textContent = `${name}: ${passed ? "OK" : "Needs attention"}. ${passed ? passExplanation : failExplanation}`;
    list.append(item);
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
  $("#blender-check").disabled = !selected || selected.lifecycle_state === "archived";
  $("#krita-export").disabled = !selected || selected.lifecycle_state === "archived";
  for (const id of ["asset-changed-browse", "blender-file-browse", "krita-source-browse", "krita-export-browse", "tests-add-file-browse"]) {
    $("#" + id).disabled = !selected || selected.lifecycle_state === "archived" || selected.lifecycle_state === "removed";
  }
  $("#asset-manifest").value = "";
  $("#asset-changed-path").value = "";
  $("#blender-file-path").value = "";
  $("#krita-source-path").value = "";
  $("#krita-export-path").value = "";
  $("#asset-result").textContent = "No manifest checked yet.";
  $("#asset-lineage").textContent = "Declared source and export lineage has not been checked.";
  $("#blender-check-status").textContent = selected ? "Enter a .blend file to check its meshes." : "Select an active project to check a Blender file.";
  $("#krita-export-status").textContent = selected ? "Enter a .kra source and a new .png output." : "Select an active project to use Krita export.";
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
      appendRemovalPlanList(project, item);
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
    build.textContent = "Scan project files";
    build.dataset.indexMode = "build";
    build.addEventListener("click", async () => {
      const mode = build.dataset.indexMode || "build";
      const action = mode === "build" ? "Scan" : "Verify";
      if (projectActionBusy || !window.confirm(`${action} the local files for ${project.name || "this project"}?`)) return;
      projectActionBusy = true;
      build.disabled = true;
      $("#project-action").textContent = mode === "build" ? "Scanning the local project files…" : "Verifying the local project files…";
      try {
        const report = mode === "build"
          ? await command("project.index.build", { project_id: project.id })
          : await command("project.index.reconcile", { project_id: project.id, verify_content: mode === "verify" });
        if (selectedProjectId !== project.id) return;
        let capability = await updateSetup();
        if (selectedProjectId !== project.id) return;
        if (mode === "build" && capability?.index_status === "stale") {
          $("#project-action").textContent = "The scan finished, but files changed during it. Verifying them now…";
          await command("project.index.reconcile", { project_id: project.id, verify_content: true });
          if (selectedProjectId !== project.id) return;
          capability = await updateSetup();
          if (selectedProjectId !== project.id) return;
        }
        $("#project-action").textContent = capability?.index_status === "ready" && capability?.content_verification_required !== true
          ? `${project.name || "Project"}: ${count(report.file_count)} files indexed and ready.`
          : "The scan finished, but files changed during it. Wait for changes to settle, then select Verify local files.";
      } catch (problem) {
        $("#project-action").textContent = projectError(problem);
      } finally {
        projectActionBusy = false;
        build.disabled = build.dataset.indexMode === "unavailable";
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
    remove.textContent = "Plan removal";
    item.append(remove);
    const plansRegion = appendRemovalPlanList(project, item);
    remove.addEventListener("click", () => planProjectRemoval(project, remove, plansRegion));
    list.append(item);
  }
}

function appendRemovalPlanList(project, item) {
    const plans = document.createElement("button");
    plans.type = "button";
    plans.textContent = "Removal plans";
    const plansRegion = document.createElement("div");
    plansRegion.className = "removal-plans";
    plansRegion.tabIndex = -1;
    plansRegion.setAttribute("role", "region");
    plansRegion.setAttribute("aria-label", "Removal plans for this project");
    plans.addEventListener("click", () => showRemovalPlans(project, plansRegion));
    item.append(plans);
    item.append(plansRegion);
    return plansRegion;
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
  const uefnInstallation = localTools?.uefn;
  $("#tool-uefn-status").textContent = uefnAvailability === "discovered"
    ? "Local endpoint found · editor not verified"
    : uefnInstallation === "detected" ? "Found here · live workflows untested"
      : uefnInstallation === "not_detected" ? "Not found in checked locations"
        : "Installation not checked";
  $("#setup-uefn").textContent = uefnAvailability === "discovered"
    ? "Last check: a local MCP endpoint responded. UEFN editor identity and live play-session workflows remain UNTESTED."
    : uefnAvailability === "unavailable"
      ? "Last check: UEFN connection was unavailable. Open UEFN, enable its MCP connection, then retry Check UEFN connection. Live workflows remain UNTESTED."
      : "UEFN connection has not been checked. For UEFN work, open the editor and use Check UEFN connection. Live workflows remain UNTESTED.";
}

function renderLocalTools(discovery) {
  localTools = null;
  if (discovery?.scope === "common_locations" && Array.isArray(discovery.tools)) {
    localTools = {};
    for (const tool of discovery.tools) {
      if (["uefn", "blender", "krita"].includes(tool.id) &&
          ["detected", "not_detected"].includes(tool.detection_status) &&
          tool.workflow_status === "untested") {
        localTools[tool.id] = tool.detection_status;
      }
    }
  }
  for (const id of ["blender", "krita"]) {
    $("#tool-" + id + "-status").textContent = localTools?.[id] === "detected"
      ? "Found here · native workflow untested"
      : localTools?.[id] === "not_detected" ? "Not found in checked locations" : "Installation not checked";
  }
  for (const id of ["uefn", "blender", "krita"]) {
    $("#launch-" + id).disabled = localTools?.[id] !== "detected";
  }
  $("#cli-open").disabled = false;
  $("#cli-status").textContent = "RELAY's command-line tool is installed with this program. Open a terminal here or use it from a local coding agent.";
  renderUefnGuidance();
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
  const buildButton = $("#project-index-action");
  showSetup("Step 3 of 3 · Check local files",
    "RELAY is checking whether this project's local file index is ready.",
    "Next: wait for the index check.", null, null);
  try {
    const capability = await command("project.capabilities", { project_id: projectId });
    if (epoch !== setupEpoch || selectedProjectId !== projectId) return;
    if (buildButton) {
      buildButton.dataset.indexMode = capability.index_status === "unavailable" || capability.baseline_state === "unavailable"
        ? "unavailable" : capability.index_status === "missing" || capability.baseline_state === "missing"
          ? "build" : capability.index_status === "stale" || capability.content_verification_required === true
          ? "verify" : "refresh";
      buildButton.textContent = buildButton.dataset.indexMode === "build" ? "Scan project files"
        : buildButton.dataset.indexMode === "verify" ? "Verify local files"
          : buildButton.dataset.indexMode === "unavailable" ? "Folder unavailable" : "Refresh file index";
      buildButton.disabled = buildButton.dataset.indexMode === "unavailable";
    }
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
        "Next: scan or verify this project's files below.", buildButton?.textContent ?? "Scan project files", "#project-index-action");
    }
    return capability;
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
  $("#blender-check").disabled = true;
  $("#krita-export").disabled = true;
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
      const available = selectedActiveProject(projectId);
      $("#asset-validate").disabled = !available;
      $("#krita-validate").disabled = !available;
      $("#asset-impact").disabled = !available;
      $("#blender-check").disabled = !available;
      $("#krita-export").disabled = !available;
    }
  }
}

function nativeAssetPath(value, extension) {
  if (typeof value !== "string" || value.length === 0 || value.length > 512 ||
      value.startsWith("/") || /[\\:\x00-\x1f\x7f]/.test(value) ||
      !value.toLowerCase().endsWith(`.${extension}`)) return false;
  return value.split("/").every((part) => part && part !== "." && part !== ".." && !/[ .]$/.test(part));
}

async function checkBlenderFile() {
  const projectId = selectedProjectId;
  if (assetActionBusy || !selectedActiveProject(projectId)) return;
  const blendPath = $("#blender-file-path").value.trim();
  const status = $("#blender-check-status");
  if (!nativeAssetPath(blendPath, "blend")) {
    status.textContent = "Enter a .blend file path inside this project, using forward slashes.";
    return;
  }
  assetActionBusy = true;
  $("#blender-check").disabled = true;
  status.textContent = "Checking Blender meshes…";
  try {
    const report = await command("assets.blender.mesh.validate", { project_id: projectId, blend_path: blendPath });
    if (selectedProjectId !== projectId) return;
    status.textContent = report.status === "passed" && report.native_workflow_status === "checked"
      ? `${count(report.total_meshes)} meshes checked. No supported mesh problems found.`
      : report.status === "failed" && report.native_workflow_status === "checked"
        ? `${count(report.issue_count)} supported mesh problem${report.issue_count === 1 ? "" : "s"} found. Review the file in Blender.`
        : report.status === "unavailable" ? "Blender was not available for this check. Native check: untested."
          : "Blender mesh check could not finish. Native result remains incomplete or untested.";
  } catch (_problem) {
    if (selectedProjectId === projectId) status.textContent = "Could not check this Blender file. Check its path and RELAY health, then try again.";
  } finally {
    assetActionBusy = false;
    if (selectedProjectId === projectId) $("#blender-check").disabled = !selectedActiveProject(projectId);
  }
}

async function exportKritaPng() {
  const projectId = selectedProjectId;
  if (assetActionBusy || !selectedActiveProject(projectId)) return;
  const sourcePath = $("#krita-source-path").value.trim();
  const exportPath = $("#krita-export-path").value.trim();
  const status = $("#krita-export-status");
  if (!nativeAssetPath(sourcePath, "kra") || !nativeAssetPath(exportPath, "png")) {
    status.textContent = "Enter a .kra source and a new .png destination inside this project, using forward slashes.";
    return;
  }
  if (!window.confirm("Create a new PNG with Krita? RELAY will not replace an existing file.")) return;
  assetActionBusy = true;
  $("#krita-export").disabled = true;
  status.textContent = "Asking Krita to create the new PNG…";
  try {
    const report = await command("assets.krita.export", { project_id: projectId, source_path: sourcePath, export_path: exportPath });
    if (selectedProjectId !== projectId) return;
    status.textContent = report.status === "exported" && report.native_workflow_status === "checked" && report.build_record_state === "stored"
      ? "New PNG created. RELAY saved a build record for this export."
      : report.status === "unavailable" ? "Krita was not available for this export. Native export: untested."
        : "Krita export could not finish. No new PNG was confirmed; check the project folder before trying again.";
  } catch (problem) {
    if (selectedProjectId === projectId) status.textContent = problem.message === "ASSET_BUILD_RECORD_FAILED"
      ? "A new PNG may have been created, but RELAY could not save its build record. Check the project folder before trying again."
      : problem.message === "ASSET_PATH_INVALID"
        ? "The source or new PNG destination is unavailable. The destination must not already exist."
        : "Krita export could not be confirmed. Check the project folder before trying again.";
  } finally {
    assetActionBusy = false;
    if (selectedProjectId === projectId) $("#krita-export").disabled = !selectedActiveProject(projectId);
  }
}

async function loadTestsForSelection() {
  const projectId = selectedProjectId;
  const list = $("#tests-list");
  list.replaceChildren();
  await loadCheckCatalog(projectId);
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

function selectedActiveProject(projectId) {
  return currentProjects.some((project) => project.id === projectId &&
    (!project.lifecycle_state || project.lifecycle_state === "active"));
}

function safeCheckId(value) {
  return typeof value === "string" && /^[A-Za-z0-9._-]{1,64}$/.test(value) ? value : null;
}

function safeResultId(value) {
  return typeof value === "string" && /^RES-[a-f0-9]{32}$/.test(value) ? value : null;
}

function safeProjectRelativeFilePath(value) {
  if (typeof value !== "string" || !value || value !== value.trim() ||
      new TextEncoder().encode(value).byteLength > 4096 ||
      value.includes("\\") || value.includes(":") || /[\u0000-\u001f]/.test(value)) return null;
  const parts = value.split("/");
  return parts.every((part) => part && part !== "." && part !== ".." &&
    !/[<>"|?*]/.test(part) && !/[. ]$/.test(part)) ? value : null;
}

function updateAddFileCheckButton() {
  $("#tests-add-file-save").disabled = checkWorkflowBusy || !selectedActiveProject(selectedProjectId) ||
    (!checkCatalog && !checkCatalogMissingConfirmed) ||
    (checkCatalog?.catalog?.checks.length ?? 0) >= 128;
}

function assertionLabel(assertion) {
  if (assertion?.kind === "indexed_file_present") return "Indexed file present (path hidden)";
  if (assertion?.kind === "indexed_file_digest") return "Indexed file digest matches (path hidden)";
  return "No index assertion declared · native workflow UNTESTED";
}

function resetCheckPlan() {
  checkPlan = null;
  $("#tests-plan").disabled = !checkCatalog || !selectedActiveProject(selectedProjectId) || checkWorkflowBusy;
  $("#tests-run").disabled = true;
  $("#tests-plan-status").textContent = "No current plan. Plan again after the index or catalog changes.";
  $("#tests-plan-list").replaceChildren();
  $("#tests-run-status").textContent = "No check run started.";
  $("#tests-run-list").replaceChildren();
}

async function loadCheckCatalog(projectId) {
  const epoch = ++checkWorkflowEpoch;
  if (checkDisplayedProjectId !== projectId) {
    $("#tests-catalog-file").value = "";
    $("#tests-add-file-id").value = "";
    $("#tests-add-file-path").value = "";
    checkDisplayedProjectId = projectId;
  }
  checkCatalog = null;
  checkCatalogMissingConfirmed = false;
  resetCheckPlan();
  $("#tests-catalog-list").replaceChildren();
  $("#tests-catalog-save").disabled = true;
  updateAddFileCheckButton();
  if (!projectId || !selectedActiveProject(projectId)) {
    $("#tests-add-file-status").textContent = "Select an active project to add a file check.";
    $("#tests-catalog-status").textContent = projectId
      ? "Select an active project to register and plan checks."
      : "Select an active project to inspect its declared checks.";
    return;
  }
  $("#tests-add-file-status").textContent = "Loading current checks before adding a file check…";
  $("#tests-catalog-status").textContent = "Loading declared checks…";
  try {
    const record = await command("project.check_catalog.get", { project_id: projectId });
    if (epoch !== checkWorkflowEpoch || selectedProjectId !== projectId) return;
    if (record.project_id !== projectId || !Number.isSafeInteger(record.revision) || record.revision < 1 ||
        !Number.isSafeInteger(record.index_generation) || record.index_generation < 1 ||
        record.catalog?.format_version !== 1 || !Array.isArray(record.catalog.checks) ||
        record.catalog.checks.length < 1 || record.catalog.checks.length > 128 ||
        record.catalog.checks.some((check) => !safeCheckId(check.id))) {
      throw new Error("CHECK_CATALOG_INVALID");
    }
    checkCatalog = record;
    updateAddFileCheckButton();
    $("#tests-add-file-status").textContent = record.catalog.checks.length >= 128
      ? "This catalog is full. Use the RELAY CLI to replace or manage checks."
      : "Ready to add an indexed-file presence check. Native and live workflows remain UNTESTED.";
    $("#tests-catalog-save").disabled = !$("#tests-catalog-file").files?.[0];
    $("#tests-catalog-status").textContent = `${record.catalog.checks.length} declared checks · revision ${record.revision} · registered at index generation ${record.index_generation}. Paths stay hidden. Select a new local JSON file to replace this catalog.`;
    for (const check of record.catalog.checks) {
      const item = document.createElement("li");
      item.textContent = `${check.id} · ${assertionLabel(check.assertion)}`;
      $("#tests-catalog-list").append(item);
    }
    $("#tests-plan").disabled = false;
  } catch (problem) {
    if (epoch !== checkWorkflowEpoch || selectedProjectId !== projectId) return;
    checkCatalogMissingConfirmed = problem.message === "CHECK_CATALOG_MISSING";
    updateAddFileCheckButton();
    $("#tests-add-file-status").textContent = checkCatalogMissingConfirmed
      ? "No checks are registered yet. Add the first indexed-file presence check. A built file index is required."
      : "Current checks are unavailable. Refresh before adding a file check.";
    $("#tests-catalog-save").disabled = !checkCatalogMissingConfirmed || !$("#tests-catalog-file").files?.[0];
    $("#tests-catalog-status").textContent = problem.message === "CHECK_CATALOG_MISSING"
      ? "No checks registered for this project. Select a local check catalog JSON file and register it."
      : "Could not inspect current checks. Refresh this page, then try again.";
  }
}

async function addFileCheck(event) {
  event.preventDefault();
  const projectId = selectedProjectId;
  const status = $("#tests-add-file-status");
  if (checkWorkflowBusy || !selectedActiveProject(projectId) ||
      (!checkCatalog && !checkCatalogMissingConfirmed)) return;
  const id = $("#tests-add-file-id").value.trim();
  const path = safeProjectRelativeFilePath($("#tests-add-file-path").value);
  if (!safeCheckId(id)) {
    status.textContent = "Use a check ID of up to 64 letters, numbers, dots, underscores, or dashes.";
    status.focus();
    return;
  }
  if (!path) {
    status.textContent = "Enter a project-relative file path with forward slashes and no parent-folder steps.";
    status.focus();
    return;
  }
  const existing = checkCatalog?.catalog?.checks ?? [];
  if (existing.length >= 128) {
    status.textContent = "This catalog is full. Use the RELAY CLI to manage its checks.";
    status.focus();
    return;
  }
  if (existing.some((check) => check.id === id)) {
    status.textContent = "That check ID already exists. Choose another ID or review the declared checks.";
    status.focus();
    return;
  }
  const expectedRevision = checkCatalog?.revision ?? 0;
  const nextCatalog = { format_version: 1, checks: [...existing, {
    id, roots: [path], leaves: [], dependency_mode: "direct",
    assertion: { kind: "indexed_file_present", path }
  }] };
  if (!validLocalCatalog(nextCatalog) ||
      new TextEncoder().encode(JSON.stringify(nextCatalog)).byteLength > 24 * 1024) {
    status.textContent = "The updated catalog is too large for this form. Use the RELAY CLI to manage it.";
    status.focus();
    return;
  }
  const requestEpoch = checkWorkflowEpoch;
  let expectedEpoch = requestEpoch;
  checkWorkflowBusy = true;
  resetCheckPlan();
  updateAddFileCheckButton();
  $("#tests-catalog-save").disabled = true;
  status.textContent = "Saving the file check…";
  try {
    const saved = await command("project.check_catalog.put", {
      project_id: projectId, expected_revision: expectedRevision, catalog: nextCatalog
    });
    if (requestEpoch !== checkWorkflowEpoch || selectedProjectId !== projectId) return;
    if (saved.project_id !== projectId || saved.revision !== expectedRevision + 1 ||
        saved.check_count !== nextCatalog.checks.length ||
        !Number.isSafeInteger(saved.index_generation) || saved.index_generation < 1) {
      throw new Error("CHECK_SAVE_UNCERTAIN");
    }
    expectedEpoch = requestEpoch + 1;
    await loadCheckCatalog(projectId);
    if (checkWorkflowEpoch !== expectedEpoch || selectedProjectId !== projectId) return;
    const recorded = checkCatalog?.catalog?.checks.find((check) => check.id === id);
    if (!recorded || recorded.dependency_mode !== "direct" ||
        recorded.assertion?.kind !== "indexed_file_present" || recorded.assertion.path !== path) {
      checkCatalog = null;
      checkCatalogMissingConfirmed = false;
      resetCheckPlan();
      status.textContent = "The save outcome needs review. Refresh current checks before trying again.";
      return;
    }
    $("#tests-add-file-id").value = "";
    $("#tests-add-file-path").value = "";
    status.textContent = `File check ${id} saved at revision ${checkCatalog.revision}. Plan it before running. Its result covers the file index only; native and live workflows remain UNTESTED.`;
  } catch (problem) {
    if (checkWorkflowEpoch !== expectedEpoch || selectedProjectId !== projectId) return;
    checkCatalog = null;
    checkCatalogMissingConfirmed = false;
    resetCheckPlan();
    status.textContent = problem.message === "CHECK_CATALOG_CONFLICT"
      ? "Checks changed since this page loaded. Refresh and review the current catalog before saving again."
      : problem.message === "INDEX_BASELINE_MISSING"
        ? "Build this project's file index, then refresh before adding a check. No workflow has been tested."
        : problem.message === "CHECK_CATALOG_INVALID"
          ? "The check was not accepted. Review its ID and relative file path, then refresh current checks."
          : "The save outcome is uncertain. Refresh current checks before retrying; the check may already be saved.";
  } finally {
    checkWorkflowBusy = false;
    if (checkWorkflowEpoch === expectedEpoch && selectedProjectId === projectId) {
      updateAddFileCheckButton();
      status.focus();
    }
  }
}

function validLocalCatalog(catalog) {
  return catalog?.format_version === 1 && Array.isArray(catalog.checks) &&
    catalog.checks.length >= 1 && catalog.checks.length <= 128 &&
    catalog.checks.every((check) => safeCheckId(check?.id) &&
      Array.isArray(check.roots) && check.roots.length >= 1 && check.roots.length <= 64 &&
      Array.isArray(check.leaves) && check.leaves.length <= 64 &&
      [...check.roots, ...check.leaves].every((path) => typeof path === "string" && path.length > 0 && path.length <= 4096) &&
      (!check.assertion || ["indexed_file_present", "indexed_file_digest"].includes(check.assertion.kind)));
}

async function registerCheckCatalog() {
  const projectId = selectedProjectId;
  const file = $("#tests-catalog-file").files?.[0];
  const status = $("#tests-catalog-status");
  if (checkWorkflowBusy || !selectedActiveProject(projectId) || !file ||
      (!checkCatalog && !checkCatalogMissingConfirmed)) return;
  if (file.size > 24 * 1024) {
    status.textContent = "This catalog is too large for the dashboard. Use the RELAY CLI for larger catalogs.";
    status.focus();
    return;
  }
  checkWorkflowBusy = true;
  $("#tests-catalog-save").disabled = true;
  $("#tests-plan").disabled = true;
  $("#tests-run").disabled = true;
  status.textContent = "Registering the selected catalog…";
  try {
    const catalog = JSON.parse(await file.text());
    if (!validLocalCatalog(catalog)) throw new Error("CHECK_CATALOG_INVALID");
    await command("project.check_catalog.put", {
      project_id: projectId, expected_revision: checkCatalog?.revision ?? 0, catalog
    });
    if (selectedProjectId !== projectId) return;
    await loadCheckCatalog(projectId);
    $("#tests-catalog-file").value = "";
    $("#tests-catalog-save").disabled = true;
    status.textContent = checkCatalog
      ? `Catalog registered at revision ${checkCatalog.revision}. Plan current checks before running.`
      : "Catalog registration may have succeeded, but its current state is unavailable. Refresh before planning.";
  } catch (problem) {
    if (selectedProjectId !== projectId) return;
    checkCatalog = null;
    checkCatalogMissingConfirmed = false;
    resetCheckPlan();
    status.textContent = problem.message === "CHECK_CATALOG_CONFLICT"
      ? "The catalog changed since it was loaded. Refresh and review its current revision before replacing it."
      : problem.message === "CHECK_CATALOG_INVALID" || problem instanceof SyntaxError
        ? "The selected JSON is not a supported check catalog. Review its format and choose it again."
        : problem.message === "DASHBOARD_REQUEST_TOO_LARGE"
          ? "This catalog is too large for the dashboard. Use the RELAY CLI for larger catalogs."
        : "Catalog registration outcome is uncertain. Refresh and inspect the current revision before retrying.";
  } finally {
    checkWorkflowBusy = false;
    if (selectedProjectId === projectId) $("#tests-catalog-save").disabled =
      !$("#tests-catalog-file").files?.[0] || (!checkCatalog && !checkCatalogMissingConfirmed);
    status.focus();
  }
}

const fallbackLabels = {
  index_not_ready: "The file index is not ready.",
  continuity_gap: "The change history is incomplete.",
  catalog_newer_than_delta: "The catalog is newer than the selected change baseline.",
  planning_bound_exceeded: "The change or dependency set exceeds the planning limit.",
  parser_configuration_unavailable: "A compatible dependency parser is unavailable.",
  parser_coverage_incomplete: "Dependency coverage is incomplete."
};

async function planDeclaredChecks() {
  const projectId = selectedProjectId;
  const catalog = checkCatalog;
  const status = $("#tests-plan-status");
  if (checkWorkflowBusy || !catalog || !selectedActiveProject(projectId)) return;
  checkWorkflowBusy = true;
  resetCheckPlan();
  $("#tests-plan").disabled = true;
  status.textContent = "Planning from the current file index…";
  try {
    const plan = await command("automation.checks.plan", {
      project_id: projectId, after_generation: catalog.index_generation
    });
    if (selectedProjectId !== projectId || checkCatalog !== catalog) return;
    const declaredIds = new Set(catalog.catalog.checks.map((check) => check.id));
    if (plan.project_id !== projectId || !/^PLAN-[a-f0-9]{32}$/.test(plan.plan_id ?? "") ||
        plan.catalog_revision !== catalog.revision || plan.after_generation !== catalog.index_generation ||
        !Number.isSafeInteger(plan.index_generation) || plan.index_generation < catalog.index_generation ||
        !["selective", "full_catalog_fallback"].includes(plan.mode) || !Array.isArray(plan.checks) ||
        plan.checks.length > 128 || new Set(plan.checks.map((entry) => entry.check_id)).size !== plan.checks.length ||
        plan.checks.some((entry) => !safeCheckId(entry.check_id) ||
          !declaredIds.has(entry.check_id) || entry.status !== "planned_not_run" || entry.result_id !== null)) {
      throw new Error("CHECK_PLAN_INVALID");
    }
    checkPlan = plan;
    const fallback = plan.mode === "full_catalog_fallback";
    status.textContent = fallback
      ? `Full catalog planned at current index generation ${plan.index_generation}; ${fallbackLabels[plan.fallback_reason] ?? "Planning evidence is incomplete."} All entries remain NOT RUN. Repair and plan again before execution.`
      : plan.checks.length === 0
        ? `No declared checks were selected at index generation ${plan.index_generation}. There is nothing to run for this change window. Native and live workflows remain UNTESTED.`
      : `${plan.checks.length} of ${catalog.catalog.checks.length} checks selected at current index generation ${plan.index_generation}. Planning only; checks are NOT RUN.`;
    const definitions = new Map(catalog.catalog.checks.map((check) => [check.id, check]));
    for (const entry of plan.checks) {
      const item = document.createElement("li");
      item.textContent = `${entry.check_id} · ${assertionLabel(definitions.get(entry.check_id)?.assertion)} · NOT RUN`;
      $("#tests-plan-list").append(item);
    }
    $("#tests-run").disabled = fallback || plan.checks.length === 0;
  } catch (problem) {
    if (selectedProjectId !== projectId) return;
    resetCheckPlan();
    status.textContent = ["INDEX_GENERATION_CONFLICT", "CHECK_CATALOG_MISSING", "CHECK_CATALOG_INVALID"].includes(problem.message)
      ? "The index or catalog changed. Refresh current checks, then plan again."
      : "Could not confirm a current plan. Refresh and plan again.";
  } finally {
    checkWorkflowBusy = false;
    if (selectedProjectId === projectId && checkCatalog) $("#tests-plan").disabled = false;
    status.focus();
  }
}

async function runDeclaredChecks() {
  const projectId = selectedProjectId;
  const plan = checkPlan;
  const status = $("#tests-run-status");
  if (checkWorkflowBusy || !plan || plan.mode !== "selective" || plan.checks.length === 0 ||
      !selectedActiveProject(projectId)) return;
  checkWorkflowBusy = true;
  $("#tests-run").disabled = true;
  $("#tests-plan").disabled = true;
  status.textContent = "Running selected indexed-file assertions…";
  try {
    const run = await command("automation.checks.execute", {
      project_id: projectId, after_generation: plan.after_generation, plan_id: plan.plan_id
    });
    if (selectedProjectId !== projectId || checkPlan !== plan) return;
    const plannedIds = new Set(plan.checks.map((entry) => entry.check_id));
    const runCounts = [run.passed_count, run.failed_count, run.untested_count];
    const actualCounts = ["passed", "failed", "untested"].map((statusValue) =>
      Array.isArray(run.checks) ? run.checks.filter((entry) => entry.status === statusValue).length : -1);
    if (run.project_id !== projectId || run.plan_id !== plan.plan_id ||
        run.index_generation !== plan.index_generation || run.mode !== "selective" ||
        run.evidence_scope !== "indexed_snapshot_at_generation" ||
        run.replayed !== true && run.replayed !== false ||
        !runCounts.every((value) => Number.isSafeInteger(value) && value >= 0 && value <= 128) ||
        runCounts.reduce((sum, value) => sum + value, 0) !== plan.checks.length ||
        actualCounts.some((value, index) => value !== runCounts[index]) ||
        !Array.isArray(run.checks) || run.checks.length !== plan.checks.length ||
        new Set(run.checks.map((entry) => entry.check_id)).size !== run.checks.length ||
        run.checks.some((entry) => !plannedIds.has(entry.check_id) ||
          !["passed", "failed", "untested"].includes(entry.status) ||
          (entry.status !== "untested" && !safeResultId(entry.result_id)))) {
      throw new Error("CHECK_EXECUTION_UNCERTAIN");
    }
    status.textContent = `${run.replayed ? "Prior run returned" : "Run recorded"}: ${run.passed_count} passed, ${run.failed_count} failed, ${run.untested_count} untested from the indexed snapshot at generation ${run.index_generation}. Refresh and plan again to assess later changes. PASS covers indexed-file assertions only; native and live workflows remain UNTESTED.`;
    $("#tests-run-list").replaceChildren();
    const reasonLabels = {
      INDEX_ASSERTION_SATISFIED: "indexed assertion satisfied",
      INDEXED_FILE_MISSING: "indexed file missing",
      INDEXED_DIGEST_MISMATCH: "indexed digest differs",
      ASSERTION_NOT_DECLARED: "no indexed assertion declared"
    };
    for (const check of run.checks) {
      const item = document.createElement("li");
      const outcome = ["passed", "failed", "untested"].includes(check.status) ? check.status.toUpperCase() : "UNKNOWN";
      const resultId = safeResultId(check.result_id);
      item.textContent = `${safeCheckId(check.check_id) ?? "Check"} · ${outcome} · ${reasonLabels[check.reason_code] ?? "reason unavailable"}${resultId ? ` · Saved result ${resultId}` : " · No indexed assertion result"}`;
      $("#tests-run-list").append(item);
    }
    checkPlan = null;
  } catch (problem) {
    if (selectedProjectId !== projectId) return;
    checkPlan = null;
    $("#tests-run-list").replaceChildren();
    status.textContent = problem.message === "CHECK_PLAN_CONFLICT" || problem.message === "INDEX_GENERATION_CONFLICT"
      ? "The plan is stale. Refresh current checks and make a new plan before running."
      : "The run outcome is uncertain. Refresh stored results and make a new plan before retrying; a previous run may already be saved.";
  } finally {
    checkWorkflowBusy = false;
    if (selectedProjectId === projectId && checkCatalog) $("#tests-plan").disabled = false;
    status.focus();
  }
}

async function changeProjectLifecycle(project, name, button) {
  if (projectActionBusy) return;
  const label = project.name || "this project";
  const warning = name === "project.archive"
    ? `Archive ${label}? Its files stay on this computer and its RELAY records are kept.`
    : name === "project.restore"
      ? `Restore ${label} to active projects?`
      : `Change ${label}?`;
  if (!window.confirm(warning)) return;
  projectActionBusy = true;
  button.disabled = true;
  $("#project-action").textContent = name === "project.archive" ? "Archiving the project…"
    : name === "project.restore" ? "Restoring the project…" : "Removing the project…";
  try {
    const argumentsValue = { project_id: project.id };
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

function removalSummary(plan) {
  const states = new Set(["pending", "approved_pending_execution", "rejected", "expired", "stale", "executed"]);
  const state = states.has(plan.state) ? plan.state.replaceAll("_", " ") : "unknown";
  const requester = plan.requester_client === "relay-dashboard" ? "Dashboard"
    : plan.requester_client === "relay-cli" ? "CLI" : "another client";
  const expiry = Number.isSafeInteger(plan.expires_at_ms) && plan.expires_at_ms > 0
    ? new Date(plan.expires_at_ms).toLocaleString() : "unknown";
  return `Project removal plan · ${state}. Requested for this project through ${requester}. Expires ${expiry}. High risk: the project registration will be removed from active use. Project files and RELAY history stay on this computer. The project registration must still match this plan. RELAY cannot reverse removal.`;
}

function setProjectStatus(message, focus = true) {
  const status = $("#project-action");
  status.textContent = message;
  if (focus) status.focus();
}

function renderRemovalPlans(project, region, plans) {
  region.replaceChildren();
  region.textContent = "";
  if (!plans.length) region.textContent = "No removal plans for this project.";
  for (const plan of plans) {
    const group = document.createElement("div");
    group.className = "removal-plan";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", "Project removal plan");
    const detail = document.createElement("p");
    detail.textContent = removalSummary(plan);
    group.append(detail);
    if (plan.state === "pending" || plan.state === "approved_pending_execution") {
      const actions = document.createElement("div");
      actions.className = "removal-plan-actions";
      const finish = document.createElement("button");
      finish.type = "button";
      finish.textContent = plan.state === "pending" ? "Approve and remove" : "Finish approved removal";
      finish.addEventListener("click", () => continueRemovalPlan(project, plan, finish));
      actions.append(finish);
      const reject = document.createElement("button");
      reject.type = "button";
      reject.textContent = "Reject plan";
      reject.addEventListener("click", () => rejectRemovalPlan(project, plan, reject, region));
      actions.append(reject);
      group.append(actions);
    }
    region.append(group);
  }
  region.focus();
}

async function showRemovalPlans(project, region) {
  $("#project-action").textContent = "Loading removal plans…";
  try {
    const result = await command("project.removal.list", { project_id: project.id, limit: 10 });
    const plans = Array.isArray(result.approvals) ? result.approvals : [];
    renderRemovalPlans(project, region, plans);
    $("#project-action").textContent = plans.length
      ? "Recent removal plans are shown under the project."
      : "No removal plans for this project.";
    return true;
  } catch (_problem) {
    setProjectStatus("Could not load current removal plans. Refresh RELAY, then try again.");
    return false;
  }
}

async function continueRemovalPlan(project, plan, button) {
  if (projectActionBusy) return;
  if (!window.confirm(`${removalSummary(plan)}\n\nRemove this project from RELAY?`)) {
    button.focus();
    return;
  }
  projectActionBusy = true;
  button.disabled = true;
  let step = plan.state === "pending" ? "approval" : "execution";
  try {
    if (plan.state === "pending") {
      const decision = await command("project.removal.decide", { project_id: project.id, approval_id: plan.approval_id, decision: "approve" });
      if (decision.state !== "approved_pending_execution") {
        setProjectStatus("This plan can no longer be approved. Reload removal plans to inspect its current state.");
        return;
      }
    }
    step = "execution";
    $("#project-action").textContent = "Approval recorded. Removing the project registration…";
    const removed = await command("project.remove", { project_id: project.id, approval_id: plan.approval_id });
    selectedProjectId = null;
    const updated = await refresh();
    setProjectStatus(removed.state === "executed"
      ? updated ? "Project removed from RELAY. Files and RELAY history were kept."
        : "Removal was recorded, but current projects could not be refreshed. Refresh RELAY to inspect its state."
      : "Removal outcome is uncertain. Refresh RELAY and inspect the removal plans.");
  } catch (_problem) {
    setProjectStatus(step === "approval"
      ? "Approval outcome is uncertain. Reload removal plans before another decision."
      : "Removal outcome is uncertain. Refresh RELAY and inspect the removal plans before trying again.");
  } finally {
    projectActionBusy = false;
    button.disabled = false;
  }
}

async function rejectRemovalPlan(project, plan, button, region) {
  if (projectActionBusy) return;
  projectActionBusy = true;
  button.disabled = true;
  try {
    const decision = await command("project.removal.decide", { project_id: project.id, approval_id: plan.approval_id, decision: "reject" });
    const refreshed = await showRemovalPlans(project, region);
    setProjectStatus(decision.state === "rejected"
      ? refreshed ? "Removal plan rejected. Project remains available."
        : "Removal plan rejected, but current plans could not be loaded. Refresh RELAY."
      : "The plan state changed before rejection. Review the current removal plans.", !refreshed);
  } catch (_problem) {
    setProjectStatus("Rejection outcome is uncertain. Reload removal plans before another decision.");
  } finally {
    projectActionBusy = false;
    button.disabled = false;
  }
}

async function planProjectRemoval(project, button, region) {
  if (projectActionBusy) return;
  projectActionBusy = true;
  button.disabled = true;
  $("#project-action").textContent = "Preparing a removal plan…";
  try {
    const plan = await command("project.removal.plan", { project_id: project.id });
    renderRemovalPlans(project, region, [plan]);
    $("#project-action").textContent = "Removal plan ready. Review its scope, then choose Approve and remove or Reject plan.";
  } catch (_problem) {
    setProjectStatus("Could not confirm whether a removal plan was created. Review removal plans before trying again.");
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
  const recent = $("#diagnostic-recent-list");
  recent.replaceChildren();
  if (!summary.available || !Array.isArray(summary.recent)) {
    $("#diagnostic-recent-summary").textContent = "Recent local events are unavailable.";
  } else {
    const labels = {
      "relay.core.started": "RELAY started",
      "relay.command.completed": "Command failed",
      "relay.host.component.failed": "Local component needs attention",
      "relay.host.component.recovered": "Local component recovered"
    };
    for (const event of summary.recent.slice(-12).reverse()) {
      const label = Object.hasOwn(labels, event?.event_code) ? labels[event.event_code] : null;
      if (!label) continue;
      const code = typeof event.error_code === "string" && /^[A-Z][A-Z0-9_]{0,63}$/.test(event.error_code)
        ? event.error_code : null;
      const commandName = typeof event.command === "string" && /^[a-z][a-z0-9._]{0,63}$/.test(event.command)
        ? event.command : null;
      const eventTime = Number.isSafeInteger(event.time_unix_ms) && event.time_unix_ms >= 0
        ? new Date(event.time_unix_ms) : null;
      const time = eventTime && Number.isFinite(eventTime.getTime())
        ? eventTime.toLocaleString() : "Time unavailable";
      const parts = [time, label];
      if (commandName) parts.push(commandName);
      if (code) parts.push(`Code: ${code}`);
      const item = document.createElement("li");
      item.textContent = parts.join(" · ");
      recent.append(item);
    }
    $("#diagnostic-recent-summary").textContent = recent.children.length
      ? `${recent.children.length} retained local event${recent.children.length === 1 ? "" : "s"}. These are observations, not workflow test results.`
      : "No retained failure or recovery events. This does not confirm that workflows passed.";
  }
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
  $("#diagnostic-recent-summary").textContent = "Recent local events are unavailable.";
  $("#diagnostic-recent-list").replaceChildren();
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
      const context = workflow.workflow_id === "context.cost_benchmark" ? workflow.context_cost_metric : null;
      if (context?.byte_scope === "sum_of_stored_payload_json_vs_compiled_result_json" &&
          Number.isSafeInteger(context.source_count) && context.source_count >= 1 && context.source_count <= 8 &&
          [context.full_payload_json_bytes, context.compiled_context_json_bytes,
           context.compiled_to_full_ratio_milli, context.full_payload_elapsed_ms,
           context.compiled_context_elapsed_ms].every((number) =>
             Number.isSafeInteger(number) && number >= 0 && number <= 1048576) &&
          context.full_payload_json_bytes > 0 && context.compiled_context_json_bytes > 0 &&
          Math.floor(context.compiled_context_json_bytes * 1000 / context.full_payload_json_bytes) === context.compiled_to_full_ratio_milli &&
          context.token_estimate_status === "not_measured" &&
          context.model_answer_quality_status === "untested" && context.remote_cost_status === "untested") {
        details.push(`Context: ${count(context.compiled_context_json_bytes)} compiled / ${count(context.full_payload_json_bytes)} source JSON bytes (${(context.compiled_to_full_ratio_milli / 10).toFixed(1)}%) across ${count(context.source_count)} results`);
        details.push(`Local time: ${count(context.compiled_context_elapsed_ms)} ms compiled / ${count(context.full_payload_elapsed_ms)} ms full`);
        details.push("Tokens not measured · answer quality and remote cost UNTESTED");
      }
      const host = workflow.workflow_id === "project.supported_host_resource" ? workflow.host_resource_metric : null;
      if (["minimum_candidate", "recommended_candidate"].includes(host?.host_tier_declaration) &&
          host.support_tier_budget_status === "untested" && host.foreground_interference_status === "untested" &&
          ["seen", "not_seen", "unavailable"].includes(host.foreground_observation) &&
          [host.physical_core_count, host.logical_processor_count, host.physical_memory_bytes,
           host.index_command_elapsed_ms, host.daemon_cpu_ms, host.cli_cpu_ms,
           host.daemon_peak_working_set_bytes, host.cli_peak_working_set_bytes,
           host.sample_count, host.sample_interval_ms, host.sampling_overhead_ms,
           host.host_probe_overhead_ms, host.foreground_probe_overhead_ms,
           host.foreground_creator_samples].every((number) => Number.isSafeInteger(number) && number >= 0) &&
          host.physical_core_count > 0 && host.physical_core_count <= 512 &&
          host.logical_processor_count >= host.physical_core_count && host.logical_processor_count <= 1024 &&
          host.physical_memory_bytes >= 1073741824 && host.physical_memory_bytes <= 4398046511104 &&
          host.index_command_elapsed_ms <= 600000 && host.daemon_cpu_ms <= 614400000 &&
          host.cli_cpu_ms <= 614400000 &&
          host.daemon_peak_working_set_bytes > 0 && host.daemon_peak_working_set_bytes <= 1099511627776 &&
          host.cli_peak_working_set_bytes > 0 && host.cli_peak_working_set_bytes <= 1099511627776 &&
          host.sample_count >= 2 && host.sample_count <= 6002 && host.sample_interval_ms === 100 &&
          host.sampling_overhead_ms <= 600000 && host.host_probe_overhead_ms <= 600000 &&
          host.foreground_probe_overhead_ms <= 600000 &&
          host.foreground_creator_samples <= host.sample_count) {
        details.push(`Host candidate: ${host.physical_core_count} physical cores, ${(host.physical_memory_bytes / 1073741824).toFixed(1)} GiB RAM · support budget UNTESTED`);
        details.push(`Index: ${count(host.index_command_elapsed_ms)} ms · daemon ${count(host.daemon_cpu_ms)} ms CPU / ${(host.daemon_peak_working_set_bytes / 1048576).toFixed(1)} MiB peak · CLI ${count(host.cli_cpu_ms)} ms CPU / ${(host.cli_peak_working_set_bytes / 1048576).toFixed(1)} MiB peak`);
        details.push(`Sampling: ${count(host.sample_count)} at ${count(host.sample_interval_ms)} ms · overhead ${count(host.sampling_overhead_ms + host.host_probe_overhead_ms + host.foreground_probe_overhead_ms)} ms`);
        details.push(`Foreground creator app: ${host.foreground_observation.replaceAll("_", " ")} · interference UNTESTED`);
      }
      item.textContent = `${workflow.workflow_id}: ${workflow.status.toUpperCase()}${code}${details.length ? ` · ${details.join(" · ")}` : ""}`;
      list.append(item);
    }
  } catch (_problem) {
    summary.textContent = "This is not a supported privacy-safe integrated report. Choose a RELAY report JSON file.";
  }
}

const safeUnsigned = (value) => Number.isSafeInteger(value) && value >= 0 ? value : null;
const safeCode = (value) => typeof value === "string" && /^[A-Z0-9_]{1,96}$/.test(value) ? value : null;
const safeRef = (value) => typeof value === "string" && /^[A-Za-z0-9._-]{1,96}$/.test(value) ? value : null;
const safeVersion = (value) => typeof value === "string" && value.length <= 32 &&
  /^(?:[0-9]|v[0-9])[A-Za-z0-9_.+-]*$/.test(value) ? value : null;
const supportEventLabels = {
  "relay.core.started": "relay_started",
  "relay.command.completed": "command_failed",
  "relay.host.component.failed": "component_failed",
  "relay.host.component.recovered": "component_recovered"
};

function checkedIntegratedSummary(report) {
  const statuses = ["passed", "failed", "blocked", "untested"];
  if (report?.schema_version !== 1 || !safeRef(report.run_id) ||
      safeUnsigned(report.started_unix_ms) === null || safeUnsigned(report.completed_unix_ms) === null ||
      report.completed_unix_ms < report.started_unix_ms ||
      report.duration_ms !== report.completed_unix_ms - report.started_unix_ms ||
      !statuses.includes(report.overall_status) || !Array.isArray(report.workflows) ||
      report.workflows.length < 1 || report.workflows.length > 256) throw new Error("INVALID_RUN_REPORT");
  const counts = { passed: 0, failed: 0, blocked: 0, untested: 0 };
  const reasons = new Map();
  const seen = new Set();
  const resources = { peak_rss_bytes: null, total_cpu_ms: null, io_read_bytes: null, io_write_bytes: null, peak_gpu_bytes: null };
  for (const workflow of report.workflows) {
    if (!safeRef(workflow?.workflow_id) || seen.has(workflow.workflow_id) ||
        !Number.isSafeInteger(workflow.phase) || workflow.phase < 0 || workflow.phase > 11 ||
        !statuses.includes(workflow.status) || !safeCode(workflow.reason_code) ||
        !Array.isArray(workflow.requirements) ||
        (workflow.status === "passed" && (workflow.evidence?.kind !== "observed" ||
          workflow.requirements.some((entry) => entry?.availability !== "available")))) {
      throw new Error("INVALID_RUN_REPORT");
    }
    seen.add(workflow.workflow_id);
    counts[workflow.status]++;
    reasons.set(workflow.reason_code, (reasons.get(workflow.reason_code) ?? 0) + 1);
    const use = workflow.resource_use;
    if (use !== null && use !== undefined) {
      for (const [source, target, sum] of [
        ["peak_rss_bytes", "peak_rss_bytes", false], ["cpu_ms", "total_cpu_ms", true],
        ["io_read_bytes", "io_read_bytes", true], ["io_write_bytes", "io_write_bytes", true],
        ["peak_gpu_bytes", "peak_gpu_bytes", false]
      ]) {
        const value = use[source];
        if (value === null || value === undefined) continue;
        if (safeUnsigned(value) === null) throw new Error("INVALID_RUN_RESOURCE");
        const next = resources[target] === null ? value : sum ? resources[target] + value : Math.max(resources[target], value);
        if (safeUnsigned(next) === null) throw new Error("RUN_RESOURCE_OVERFLOW");
        resources[target] = next;
      }
    }
  }
  const expected = counts.failed ? "failed" : counts.blocked ? "blocked" : counts.untested ? "untested" : "passed";
  if (expected !== report.overall_status || statuses.some((status) => report.counts?.[status] !== counts[status]) || reasons.size > 64) {
    throw new Error("INCONSISTENT_RUN_STATUS");
  }
  return {
    report_schema_version: 1, run_ref: report.run_id, completed_unix_ms: report.completed_unix_ms,
    duration_ms: report.duration_ms, overall_status: report.overall_status, counts,
    resources: Object.values(resources).every((value) => value === null) ? null : resources,
    outcome_reasons: [...reasons].sort(([left], [right]) => left.localeCompare(right))
      .map(([code, count]) => ({ code, count })),
    evidence_status: "selected_file_consistency_checked_not_independently_verified"
  };
}

async function selectedIntegratedSummary() {
  const file = $("#integrated-report").files?.[0];
  if (!file) return null;
  if (safeUnsigned(file.size) === null || file.size === 0 || file.size > 262144) throw new Error("INVALID_RUN_REPORT");
  const source = await file.text();
  if (new TextEncoder().encode(source).byteLength > 262144) throw new Error("INVALID_RUN_REPORT");
  return checkedIntegratedSummary(JSON.parse(source));
}

function checkedSupportSummary(status, doctor, summary, integratedRun) {
  const capture = status?.diagnostics;
  const events = summary?.events;
  if (!safeVersion(status?.version) || !safeVersion(summary?.relay_version) ||
      safeUnsigned(summary?.generated_unix_ms) === null ||
      typeof capture?.ok !== "boolean" || typeof capture?.detail_active !== "boolean" ||
      typeof doctor?.healthy !== "boolean" || !Array.isArray(doctor.checks) || doctor.checks.length > 32 ||
      typeof summary?.available !== "boolean") throw new Error("INVALID_SUPPORT_DATA");
  const number = (value) => safeUnsigned(value) ?? 0;
  const knownChecks = new Set(["core.process", "storage.integrity", "diagnostics.capture",
    "transport.local", "adapter.dependencies.parse"]);
  const checks = doctor.checks.filter((check) => knownChecks.has(check?.id)).map((check) => {
    if (!["pass", "fail"].includes(check.status)) throw new Error("INVALID_SUPPORT_DATA");
    return { id: check.id, status: check.status };
  });
  const total = summary.available ? safeUnsigned(events?.total) : 0;
  const incomplete = summary.available ? safeUnsigned(events?.incomplete) : 0;
  const sampled = summary.available ? safeUnsigned(events?.sampled) : 0;
  if (total === null || incomplete === null || sampled === null || incomplete > total || sampled > total) {
    throw new Error("INVALID_SUPPORT_DATA");
  }
  const recentEvents = [];
  if (summary.available && Array.isArray(summary.recent)) {
    for (const event of summary.recent.slice(-12)) {
      const label = Object.hasOwn(supportEventLabels, event?.event_code) ? supportEventLabels[event.event_code] : null;
      if (!label) continue;
      const time = safeUnsigned(event.time_unix_ms);
      recentEvents.push({ time_unix_ms: time, event: label, error_code: safeCode(event.error_code) });
    }
  }
  const bundle = {
    schema_version: 1, format: "relay_dashboard_support_summary", generated_unix_ms: summary.generated_unix_ms,
    relay_version: status.version, component_versions: [{ component_id: "relay-core", version: summary.relay_version }],
    protocol_version: safeUnsigned(status.protocol?.negotiated),
    diagnostic_health: {
      healthy: capture.ok, current_bytes: number(capture.current_bytes), rotated_files: number(capture.rotated_files),
      evicted_events: number(capture.evicted_events), evicted_files: number(capture.evicted_files),
      recovered_partial_bytes: number(capture.recovered_partial_bytes),
      last_sync_unix_ms: safeUnsigned(capture.last_sync_unix_ms), detail_active: capture.detail_active,
      last_error_code: safeCode(summary.error_code)
    },
    diagnostic_counts: {
      total_events: total, invalid_lines: number(events?.invalid_lines), incomplete_events: incomplete,
      sampled_events: sampled, untrusted_source_events: 0,
      retention_evicted_events: number(capture.evicted_events), by_code: []
    },
    component_checks: checks, other_component_check_count: doctor.checks.length - checks.length,
    recent_events: recentEvents, integrated_run: integratedRun,
    native_workflows: { uefn_editor: "UNTESTED", creator_apps: "UNTESTED", hardware_tiers: "UNTESTED",
      note: "Live native outcomes require an observed integrated run; local component health is not workflow evidence." },
    privacy: { policy: "summary_only_v1", includes_raw_logs: false, includes_local_paths: false,
      includes_secrets: false, includes_command_arguments: false, includes_project_file_contents: false }
  };
  if (new TextEncoder().encode(JSON.stringify(bundle)).byteLength > 65536) throw new Error("BUNDLE_TOO_LARGE");
  return bundle;
}

async function downloadSupportSummary() {
  if (supportDownloadBusy) return;
  supportDownloadBusy = true;
  const button = $("#support-download");
  const message = $("#support-download-status");
  button.disabled = true;
  message.textContent = "Preparing current support summary…";
  try {
    const integratedRun = await selectedIntegratedSummary();
    const [status, doctor, summary] = await Promise.all([
      command("system.status"), command("system.doctor"), command("diagnostics.summary")
    ]);
    const bundle = checkedSupportSummary(status, doctor, summary, integratedRun);
    const blob = new Blob([JSON.stringify(bundle, null, 2) + "\n"], { type: "application/json" });
    if (blob.size > 65536) throw new Error("BUNDLE_TOO_LARGE");
    const url = URL.createObjectURL(blob);
    let clicked = false;
    try {
      const link = document.createElement("a");
      link.href = url;
      link.download = `relay-support-summary-${summary.generated_unix_ms}.json`;
      link.click();
      clicked = true;
    } finally {
      if (clicked) setTimeout(() => URL.revokeObjectURL(url), 60000);
      else URL.revokeObjectURL(url);
    }
    message.textContent = "Support summary download started. Review the saved JSON before sharing it. Selected report outcomes are file-provided, not independently verified here.";
  } catch (_problem) {
    message.textContent = "Could not save the support summary. Refresh RELAY and choose a valid integrated report file if one is selected, then try again.";
  } finally {
    supportDownloadBusy = false;
    button.disabled = false;
    message.focus();
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
    if (openedFromFile) throw new Error("OPENED_AS_FILE");
    if (!token) throw new Error("Open a fresh dashboard link from RELAY, then try again.");
    const [status, doctor, projectList, summary] = await Promise.all([
      command("system.status"), command("system.doctor"), command("project.list", { include_inactive: true }), command("diagnostics.summary")
    ]);
    message.textContent = status.recovery_state === "Healthy"
      ? "RELAY's local engine is responding. Follow the setup steps below to check your project."
      : "RELAY's local engine needs attention. See Health below.";
    message.dataset.state = status.recovery_state;
    const basicChecks = Array.isArray(doctor.checks) ? doctor.checks : [];
    const passingChecks = basicChecks.filter((check) => check.status === "pass").length;
    $("#health-summary").textContent = doctor.healthy
      ? `${passingChecks} basic RELAY checks reported OK. Project files, creator apps, and AI connections are checked separately.`
      : `${basicChecks.length - passingChecks} basic RELAY check${basicChecks.length - passingChecks === 1 ? "" : "s"} need attention. Project files and apps are checked separately.`;
    renderChecks(doctor.checks);
    const nextAction = $("#next-action");
    nextAction.hidden = !doctor.next_action;
    nextAction.textContent = doctor.next_action ? `Suggested next step: ${doctor.next_action}` : "";
    renderProjects(projectList.projects ?? []);
    await updateSetup(projectList.projects ?? []);
    renderUefnGuidance();
    renderDiagnostics(status, doctor, summary);
    renderAutomation(status.automation_mode);
    const [usage, history, results, jobs, localToolReport] = await Promise.allSettled([
      command("usage.summary"), command("transaction.list", { limit: 10 }),
      command("result.list", { limit: 10 }), command("job.list", { limit: 10 }), command("tools.local.discover")
    ]);
    renderLocalTools(localToolReport.status === "fulfilled" ? localToolReport.value : null);
    renderUsage(usage.status === "fulfilled" ? usage.value : null);
    renderActivity(history.status === "fulfilled" ? history.value : null);
    renderResults(results.status === "fulfilled" ? results.value : null);
    renderJobs(jobs.status === "fulfilled" ? jobs.value : null);
    await loadTestsForSelection();
    return true;
  } catch (problem) {
    message.textContent = openedFromFile
      ? "This is the dashboard source file. Open the local dashboard link from RELAY to connect it."
      : !token || problem.message === "DASHBOARD_UNAUTHORIZED"
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
    $("#blender-check").disabled = true;
    $("#krita-export").disabled = true;
    for (const id of ["asset-changed-browse", "blender-file-browse", "krita-source-browse", "krita-export-browse", "tests-add-file-browse", "launch-uefn", "launch-blender", "launch-krita", "cli-open"]) {
      $("#" + id).disabled = true;
    }
    $("#cli-status").textContent = "RELAY's local connection is unavailable. Open RELAY again before using its terminal shortcut.";
    $("#app-launch-status").textContent = "App launch controls are unavailable until RELAY reconnects.";
    $("#asset-manifest").value = "";
    $("#asset-changed-path").value = "";
    $("#blender-file-path").value = "";
    $("#krita-source-path").value = "";
    $("#krita-export-path").value = "";
    $("#asset-result").textContent = "Current asset information is unavailable.";
    $("#asset-lineage").textContent = "Declared lineage has not been checked.";
    $("#blender-check-status").textContent = "Current project information is unavailable.";
    $("#krita-export-status").textContent = "Current project information is unavailable.";
    $("#asset-findings").replaceChildren();
    $("#tests-summary").textContent = "Stored test summaries are unavailable. Live UEFN test status: UNTESTED.";
    $("#tests-list").replaceChildren();
    ++checkWorkflowEpoch;
    checkCatalog = null;
    checkCatalogMissingConfirmed = false;
    $("#tests-add-file-id").value = "";
    $("#tests-add-file-path").value = "";
    $("#tests-add-file-status").textContent = "Current checks are unavailable. Refresh after RELAY is running.";
    updateAddFileCheckButton();
    resetCheckPlan();
    $("#tests-catalog-save").disabled = true;
    $("#tests-catalog-list").replaceChildren();
    $("#tests-catalog-status").textContent = "Current declared checks are unavailable. Refresh after RELAY is running.";
    $("#uefn-connection").textContent = "UEFN editor connection has not been checked.";
    uefnAvailability = "not_checked";
    renderLocalTools(null);
    if (openedFromFile) {
      showSetup("Open the live dashboard",
        "This file is only the dashboard source. It is not connected to RELAY when opened from a file path.",
        "Next: start RELAY, then run `relay dashboard-url` and open the printed http://127.0.0.1 link.", null, null);
    } else if (!token || problem.message === "DASHBOARD_UNAUTHORIZED") {
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

$("#refresh").addEventListener("click", async () => {
  await refresh();
  button.focus();
});
$("#tests-catalog-file").addEventListener("change", () => {
  $("#tests-catalog-save").disabled = !$("#tests-catalog-file").files?.[0] ||
    (!checkCatalog && !checkCatalogMissingConfirmed) || !selectedActiveProject(selectedProjectId);
  $("#tests-catalog-status").textContent = $("#tests-catalog-file").files?.[0]
    ? "Local catalog selected. Register it for the active project."
    : "No local catalog selected.";
});
$("#tests-catalog-save").addEventListener("click", registerCheckCatalog);
$("#tests-add-file-form").addEventListener("submit", addFileCheck);
$("#tests-plan").addEventListener("click", planDeclaredChecks);
$("#tests-run").addEventListener("click", runDeclaredChecks);
$("#asset-manifest").addEventListener("change", () => {
  $("#asset-result").textContent = $("#asset-manifest").files?.[0]
    ? "Manifest selected. Choose a local check below." : "No manifest checked yet.";
});
$("#asset-validate").addEventListener("click", () => validateAssets("assets.manifest.validate"));
$("#krita-validate").addEventListener("click", () => validateAssets("assets.krita.inspect"));
$("#asset-impact").addEventListener("click", () => validateAssets("assets.impact.analyze"));
$("#blender-check").addEventListener("click", checkBlenderFile);
$("#krita-export").addEventListener("click", exportKritaPng);
$("#automation-toggle").addEventListener("click", toggleAutomation);
$("#integrated-report").addEventListener("change", async () => {
  await previewIntegratedReport();
  $("#integrated-summary").focus();
});
$("#support-download").addEventListener("click", downloadSupportSummary);
async function addProject(name, rootPath) {
  if (projectActionBusy) return;
  if (!name || !rootPath) {
    $("#project-action").textContent = "Enter a project name and an existing local folder.";
    return false;
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
    return true;
  } catch (problem) {
    $("#project-action").textContent = projectError(problem);
    return false;
  } finally {
    projectActionBusy = false;
    $("#project-add").disabled = false;
  }
}
async function findCodexProjects() {
  const button = $("#codex-find");
  const status = $("#codex-find-status");
  const list = $("#codex-candidates");
  const add = $("#codex-add-selected");
  button.disabled = true;
  status.textContent = "Looking for local Codex work folders on this computer…";
  list.replaceChildren();
  list.value = "";
  list.disabled = true;
  add.disabled = true;
  codexCandidates = [];
  codexAdded = new Set();
  try {
    const result = await command("codex.workspaces.discover");
    const candidates = result?.status === "available" && Array.isArray(result.candidates)
      ? result.candidates.slice(0, 32) : [];
    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = "Choose a local folder";
    list.append(placeholder);
    for (const candidate of candidates) {
      if (typeof candidate.name !== "string" || !candidate.name || candidate.name.length > 120 ||
          typeof candidate.root_path !== "string" || !candidate.root_path || candidate.root_path.length > 2048) continue;
      const folder = typeof candidate.folder_name === "string" && candidate.folder_name.length <= 120
        ? candidate.folder_name : "";
      const origin = candidate.origin === "saved_project" ? "Saved project" :
        candidate.origin === "recent_task_folder" ? "Recent task folder" : "Local folder";
      const option = document.createElement("option");
      option.value = String(codexCandidates.length);
      option.textContent = `${folder && folder !== candidate.name ? `${candidate.name} · ${folder}` : candidate.name} · ${origin}`;
      list.append(option);
      codexCandidates.push(candidate);
    }
    list.disabled = codexCandidates.length === 0;
    status.textContent = codexCandidates.length
      ? `${codexCandidates.length} local Codex folder${codexCandidates.length === 1 ? "" : "s"} found. Choose one to add. RELAY does not open chats or account data.`
      : "No usable local Codex folders were found here. Chats or Work projects without local files cannot be indexed; you can still add a folder below.";
  } catch (_problem) {
    status.textContent = "Codex's local folder list is unavailable. You can still add a folder below.";
  } finally {
    button.disabled = false;
  }
}
$("#codex-find").addEventListener("click", findCodexProjects);
$("#codex-candidates").addEventListener("change", () => {
  const index = Number($("#codex-candidates").value);
  const added = codexAdded.has(index);
  $("#codex-add-selected").disabled = $("#codex-candidates").value === "" || !codexCandidates[index] || added;
  $("#codex-add-selected").textContent = added ? "Added to RELAY" : "Add to RELAY";
});
$("#codex-add-selected").addEventListener("click", async () => {
  const select = $("#codex-candidates");
  const index = Number(select.value);
  const candidate = select.value !== "" ? codexCandidates[index] : null;
  if (!candidate) return;
  const add = $("#codex-add-selected");
  add.disabled = true;
  const added = await addProject(candidate.name, candidate.root_path);
  if (added) {
    codexAdded.add(index);
    add.textContent = "Added to RELAY";
    select.children[index + 1].textContent += " · Added";
  } else {
    add.disabled = false;
  }
});
async function pickLocalPath(kind, fileType, inputId, statusId, maxLength) {
  const status = $("#" + statusId);
  const input = $("#" + inputId);
  if (kind !== "project_folder" && !selectedActiveProject(selectedProjectId)) {
    status.textContent = "Select an active project first, then choose a file inside it.";
    return;
  }
  const argumentsValue = { kind };
  if (kind !== "project_folder") argumentsValue.project_id = selectedProjectId;
  if (fileType) argumentsValue.file_type = fileType;
  status.textContent = kind === "project_folder" ? "Opening the folder chooser…" : "Opening the file chooser…";
  try {
    const picked = await command("local.path.pick", argumentsValue);
    if (picked?.status === "cancelled") {
      status.textContent = "Nothing selected. Your current entry was kept.";
    } else if (picked?.status === "selected" && typeof picked.path === "string" &&
               picked.path.length > 0 && picked.path.length <= maxLength) {
      input.value = picked.path;
      status.textContent = kind === "project_folder" ? "Folder selected. Add a project name, then choose Add this project." :
        kind === "project_output_png" ? "New PNG location selected. Choose Create new PNG when ready." :
        "File selected inside this project. Continue with its check.";
      input.focus();
    } else {
      status.textContent = "The selected path could not be used here. Choose a shorter path inside the project.";
    }
  } catch (_problem) {
    status.textContent = "The file chooser is unavailable. You can still type or paste the path.";
  }
}
for (const [inputId, buttonId, kind, fileType, statusId, maxLength] of [
  ["project-root", "project-browse", "project_folder", null, "path-picker-status", 2048],
  ["asset-changed-path", "asset-changed-browse", "project_file", "any", "asset-result", 512],
  ["blender-file-path", "blender-file-browse", "project_file", "blend", "blender-check-status", 512],
  ["krita-source-path", "krita-source-browse", "project_file", "kra", "krita-export-status", 512],
  ["krita-export-path", "krita-export-browse", "project_output_png", null, "krita-export-status", 512],
  ["tests-add-file-path", "tests-add-file-browse", "project_file", "any", "tests-add-file-status", 4096]
]) {
  const pick = () => pickLocalPath(kind, fileType, inputId, statusId, maxLength);
  $("#" + buttonId).addEventListener("click", pick);
  $("#" + inputId).addEventListener("click", pick);
}
async function launchLocalApp(app, buttonId, label, statusId = "app-launch-status") {
  const launchButton = $("#" + buttonId);
  const status = $("#" + statusId);
  launchButton.disabled = true;
  status.textContent = `Opening ${label}…`;
  try {
    const result = await command("local.app.launch", { app });
    status.textContent = result?.status === "started"
      ? `${label} was asked to open. Check its window; RELAY has not verified its workflow.`
      : `${label} was not found in supported local installation locations.`;
  } catch (_problem) {
    status.textContent = `${label} could not be opened here. Check RELAY health, then try again.`;
  } finally {
    launchButton.disabled = app === "relay_terminal" ? false : localTools?.[app] !== "detected";
  }
}
for (const [app, buttonId, label] of [
  ["uefn", "launch-uefn", "UEFN"],
  ["blender", "launch-blender", "Blender"],
  ["krita", "launch-krita", "Krita"]
]) {
  $("#" + buttonId).addEventListener("click", () => launchLocalApp(app, buttonId, label));
}
$("#cli-open").addEventListener("click", () => launchLocalApp("relay_terminal", "cli-open", "RELAY terminal", "cli-status"));
function revealHashTarget(id = location.hash.slice(1)) {
  if (!id) return;
  const target = document.getElementById(id);
  if (!target) return;
  if (target.nextElementSibling?.tagName === "DETAILS" &&
      target.nextElementSibling.classList.contains("panel-disclosure")) {
    target.nextElementSibling.open = true;
  }
  for (let parent = target.parentElement; parent; parent = parent.parentElement) {
    if (parent.tagName === "DETAILS") parent.open = true;
  }
}
window.addEventListener("hashchange", () => revealHashTarget());
document.addEventListener("click", (event) => {
  const anchor = event.target.closest?.('a[href^="#"]');
  if (anchor) revealHashTarget(anchor.getAttribute("href").slice(1));
});
revealHashTarget();
$("#project-add-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  await addProject($("#project-name").value.trim(), $("#project-root").value.trim());
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
