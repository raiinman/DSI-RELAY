const token = document.querySelector('meta[name="relay-dashboard-token"]').content;
const raw = document.querySelector("#raw-output");

async function command(name, args = {}) {
  const response = await fetch("/api/execute", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-relay-dashboard-token": token
    },
    body: JSON.stringify({ command: name, arguments: args })
  });
  const payload = await response.json();
  if (!response.ok) throw new Error(payload?.error?.message ?? "Dashboard transport failed.");
  raw.textContent = JSON.stringify(payload, null, 2);
  return payload;
}

function text(value) {
  return document.createTextNode(String(value));
}

function renderStatus(payload) {
  const result = payload.result;
  const summary = document.querySelector("#status-summary");
  const badge = document.querySelector("#status-badge");
  const healthy = payload.ok && result.recovery_state === "Healthy";

  summary.textContent = healthy
    ? "RELAY is running and its current storage check passed."
    : `RELAY is running, but its recovery state is ${result.recovery_state ?? "unknown"}.`;
  badge.textContent = result.recovery_state ?? "Unknown";
  badge.dataset.state = healthy ? "healthy" : "attention";
  document.querySelector("#transport").textContent =
    `Host ${result.version} · protocol ${result.protocol.negotiated ?? "?"}`;
}

function renderDoctor(payload) {
  const result = payload.result;
  document.querySelector("#doctor-summary").textContent = result.summary;
  const list = document.querySelector("#doctor-checks");
  list.replaceChildren();
  for (const check of result.checks) {
    const item = document.createElement("li");
    item.className = `check check-${check.status}`;
    const label = document.createElement("strong");
    label.append(text(check.status === "pass" ? "OK" : check.status === "warning" ? "Check" : "Problem"));
    item.append(label, text(` — ${check.detail}`));
    list.append(item);
  }
}

function renderProjects(payload) {
  const container = document.querySelector("#projects");
  container.replaceChildren();
  if (!payload.ok) {
    const problem = document.createElement("p");
    problem.className = "muted";
    problem.textContent = payload.error?.message ?? "Project list is unavailable.";
    container.append(problem);
    return;
  }
  const projects = payload.result.projects;
  if (!projects.length) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "No projects are registered yet.";
    container.append(empty);
    return;
  }
  for (const project of projects) {
    const card = document.createElement("div");
    card.className = "project";
    const name = document.createElement("strong");
    name.textContent = project.name;
    const id = document.createElement("code");
    id.textContent = project.id;
    card.append(name, id);
    container.append(card);
  }
}

async function refresh() {
  const button = document.querySelector("#refresh");
  button.disabled = true;
  try {
    const [status, doctor, projects] = await Promise.all([
      command("system.status"),
      command("system.doctor"),
      command("project.list")
    ]);
    renderStatus(status);
    renderDoctor(doctor);
    renderProjects(projects);
  } catch (error) {
    document.querySelector("#status-summary").textContent = error.message;
    document.querySelector("#status-badge").textContent = "Unavailable";
    document.querySelector("#status-badge").dataset.state = "attention";
  } finally {
    button.disabled = false;
  }
}

document.querySelector("#refresh").addEventListener("click", refresh);
document.querySelector("#result-form").addEventListener("submit", async event => {
  event.preventDefault();
  const output = document.querySelector("#result-output");
  output.hidden = false;
  output.textContent = "Loading…";
  try {
    const resultId = document.querySelector("#result-id").value.trim();
    const payload = await command("result.get", { result_id: resultId });
    output.textContent = payload.ok
      ? JSON.stringify(payload.result, null, 2)
      : (payload.error?.message ?? "Result is unavailable.");
  } catch (error) {
    output.textContent = error.message;
  }
});

refresh();
