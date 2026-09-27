const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);

const $ = (selector) => document.querySelector(selector);
const details = $("#details");
const button = $("#refresh");
let refreshing = false;

async function command(name) {
  const response = await fetch("/api/execute", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Relay-Dashboard-Token": token
    },
    body: JSON.stringify({ command: name, arguments: {} }),
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
    problem.textContent = `${labels[check.id] ?? "Another component needs attention."} See Advanced details for the exact result.`;
    list.append(problem);
  }
}

function renderProjects(projects) {
  const list = $("#projects");
  list.replaceChildren();
  $("#project-count").textContent = projects.length
    ? `${projects.length} project${projects.length === 1 ? "" : "s"} connected`
    : "No projects connected yet.";
  for (const project of projects) {
    const item = document.createElement("li");
    const name = document.createElement("strong");
    name.textContent = project.name || "Unnamed project";
    item.append(name);
    list.append(item);
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
    const [status, doctor, projectList] = await Promise.all([
      command("system.status"), command("system.doctor"), command("project.list")
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
    details.textContent = JSON.stringify({ status, doctor, projects: projectList }, null, 2);
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
    details.textContent = "No current details available.";
  } finally {
    button.setAttribute("aria-disabled", "false");
    refreshing = false;
  }
}

$("#refresh").addEventListener("click", refresh);
refresh();
