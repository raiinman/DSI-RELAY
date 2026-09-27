const token = location.hash.slice(1);
history.replaceState(null, "", location.pathname);

const $ = (selector) => document.querySelector(selector);
const details = $("#details");

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
    ? `${problems.length} check${problems.length === 1 ? "" : "s"} need attention. Open Advanced details for the exact results.`
    : "All checks passed.";
  list.append(item);
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
    name.textContent = project.name;
    const id = document.createElement("small");
    id.textContent = project.id;
    item.append(name, id);
    list.append(item);
  }
}

async function refresh() {
  const button = $("#refresh");
  button.disabled = true;
  try {
    if (!token) throw new Error("Open this page using the link from relay dashboard-url.");
    const [status, doctor, projectList] = await Promise.all([
      command("system.status"), command("system.doctor"), command("project.list")
    ]);
    $("#status-message").textContent = status.recovery_state === "Healthy"
      ? "RELAY is working."
      : "RELAY needs attention. See Health below.";
    $("#status-message").dataset.state = status.recovery_state;
    $("#health-summary").textContent = doctor.summary;
    renderChecks(doctor.checks);
    renderProjects(projectList.projects ?? []);
    details.textContent = JSON.stringify({ status, doctor, projects: projectList }, null, 2);
  } catch (problem) {
    $("#status-message").textContent = problem.message;
    $("#status-message").dataset.state = "Degraded";
  } finally {
    button.disabled = false;
  }
}

$("#refresh").addEventListener("click", refresh);
refresh();
