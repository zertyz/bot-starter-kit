const state = {
  model: null,
  selected: null,
};

const stateOrder = [
  "Under Planning",
  "Planned",
  "Started",
  "In Code Review",
  "Integrated",
  "QA",
  "Merged",
  "Rolled Out",
  "Rejected",
  "Cancelled",
];

function $(selector, root = document) {
  return root.querySelector(selector);
}

function $all(selector, root = document) {
  return Array.from(root.querySelectorAll(selector));
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function pill(text, kind = "") {
  return `<span class="pill ${kind}">${escapeHtml(text)}</span>`;
}

function stateKind(value) {
  if (["Merged", "Rolled Out"].includes(value)) return "green";
  if (["Rejected", "Cancelled"].includes(value)) return "red";
  if (["Started", "In Code Review", "Integrated", "QA"].includes(value)) return "blue";
  return "orange";
}

async function loadModel() {
  const response = await fetch("/api/model");
  if (!response.ok) throw new Error(await response.text());
  state.model = await response.json();
  render();
}

function render() {
  const model = state.model;
  $("#repo-root").textContent = model.repo_root;
  $("#generated-at").textContent = `Generated ${model.generated_at}`;
  renderMetrics();
  renderStateCounts();
  renderAttention();
  renderRequirements();
  renderWorkItems();
  renderSelects();
  renderLedgers();
  renderDiagrams();
  renderSelection();
}

function renderMetrics() {
  const labels = [
    ["requirements", "Requirements"],
    ["work_items", "Work Items"],
    ["traceability_gaps", "Trace Gaps"],
    ["unmapped_requirements", "Unmapped"],
    ["stale_work", "Stale Work"],
    ["open_risks", "Open Risks"],
    ["open_incidents", "Open Incidents"],
    ["active_experiments", "Experiments"],
  ];
  $("#metrics").innerHTML = labels.map(([key, label]) => `
    <div class="metric">
      <strong>${escapeHtml(state.model.counts[key])}</strong>
      <span>${escapeHtml(label)}</span>
    </div>
  `).join("");
}

function renderStateCounts() {
  const rows = stateOrder
    .filter((name) => state.model.state_counts[name])
    .map((name) => `<div class="list-item"><strong>${escapeHtml(name)}</strong><span>${state.model.state_counts[name]} item(s)</span></div>`)
    .join("");
  $("#state-counts").innerHTML = `<div class="list">${rows || '<p class="empty">None</p>'}</div>`;
}

function renderAttention() {
  const items = [];
  for (const issue of state.model.errors) {
    items.push(`<div class="list-item"><strong class="error">ERROR ${escapeHtml(issue.path)}:${issue.line}</strong><span>${escapeHtml(issue.message)}</span></div>`);
  }
  for (const issue of state.model.warnings) {
    items.push(`<div class="list-item"><strong>WARN ${escapeHtml(issue.path)}:${issue.line}</strong><span>${escapeHtml(issue.message)}</span></div>`);
  }
  for (const item of state.model.stale_work.slice(0, 8)) {
    items.push(`<div class="list-item"><strong>${escapeHtml(item.id)}</strong><span>${escapeHtml(item.reason)}</span></div>`);
  }
  for (const req of state.model.traceability_gaps.slice(0, 8)) {
    items.push(`<div class="list-item"><strong>${escapeHtml(req)}</strong><span>Missing traceability link</span></div>`);
  }
  $("#attention").innerHTML = `<div class="list">${items.join("") || '<p class="empty">None</p>'}</div>`;
}

function requirementById(id) {
  return state.model.requirements.find((item) => item.id === id);
}

function workById(id) {
  return state.model.work_items.find((item) => item.id === id);
}

function renderRequirements() {
  const filter = $("#requirement-filter").value.toLowerCase();
  const rows = state.model.requirements
    .filter((req) => `${req.id} ${req.title} ${req.body}`.toLowerCase().includes(filter))
    .map((req) => `
      <tr>
        <td><button type="button" data-select-requirement="${escapeHtml(req.id)}">${escapeHtml(req.id)}</button></td>
        <td>${escapeHtml(req.title)}</td>
        <td>${req.work_items.length}</td>
        <td>${req.traceability.length ? pill("linked", "green") : pill("gap", "orange")}</td>
        <td class="actions">
          <button type="button" data-action="show-requirement" data-requirement-id="${escapeHtml(req.id)}">Show</button>
          <button type="button" data-action="estimate-requirement" data-requirement-id="${escapeHtml(req.id)}">Estimate</button>
          <button type="button" data-action="sync-requirement" data-requirement-id="${escapeHtml(req.id)}">Sync</button>
          <button type="button" data-action="trace-requirement" data-requirement-id="${escapeHtml(req.id)}">Trace</button>
        </td>
      </tr>
    `).join("");
  $("#requirements-table").innerHTML = rows || `<tr><td colspan="5" class="empty">None</td></tr>`;
}

function renderWorkItems() {
  const filter = $("#work-filter").value.toLowerCase();
  const rows = state.model.work_items
    .filter((item) => `${item.id} ${item.requirement_id} ${item.title} ${item.state} ${item.owner ?? ""}`.toLowerCase().includes(filter))
    .map((item) => `
      <tr>
        <td><button type="button" data-select-work="${escapeHtml(item.id)}">${escapeHtml(item.id)}</button></td>
        <td>${escapeHtml(item.requirement_id)}</td>
        <td>${pill(item.state, stateKind(item.state))}</td>
        <td>${escapeHtml(item.owner ?? "")}</td>
        <td>${escapeHtml(item.title)}</td>
        <td class="actions">
          <button type="button" data-action="show-work" data-ref="${escapeHtml(item.id)}">Show</button>
          <button type="button" data-action="evaluate-plan" data-ref="${escapeHtml(item.id)}">Evaluate</button>
          <button type="button" data-action="verification-check" data-ref="${escapeHtml(item.id)}">Verify</button>
          <button type="button" data-action="review" data-ref="${escapeHtml(item.id)}">Review</button>
          <button type="button" data-action="acceptance-packet" data-ref="${escapeHtml(item.id)}">Accept</button>
        </td>
      </tr>
    `).join("");
  $("#work-table").innerHTML = rows || `<tr><td colspan="6" class="empty">None</td></tr>`;
}

function renderSelects() {
  const reqOptions = state.model.requirements.map((req) => `<option value="${escapeHtml(req.id)}">${escapeHtml(req.id)} -- ${escapeHtml(req.title)}</option>`).join("");
  const workOptions = state.model.work_items.map((item) => `<option value="${escapeHtml(item.id)}">${escapeHtml(item.id)} -- ${escapeHtml(item.state)}</option>`).join("");
  for (const select of $all('[data-select="requirements"]')) select.innerHTML = reqOptions;
  for (const select of $all('[data-select="work-items"]')) select.innerHTML = workOptions;
  $("#advance-state-target").innerHTML = `<option value="">Next state</option>${stateOrder.map((item) => `<option>${escapeHtml(item)}</option>`).join("")}`;
}

function renderLedgers() {
  const sections = [
    ["decisions", "Recent Decisions"],
    ["risks", "Open Risks"],
    ["incidents", "Open Incidents"],
    ["experiments", "Active Experiments"],
  ];
  $("#ledger-grid").innerHTML = sections.map(([key, label]) => `
    <section class="panel">
      <h2>${escapeHtml(label)}</h2>
      <div class="list">
        ${state.model.ledgers[key].map((entry) => `
          <div class="list-item">
            <strong>${escapeHtml(entry.id)} -- ${escapeHtml(entry.title)}</strong>
            <span>${escapeHtml(entry.status ?? "")} ${escapeHtml(entry.path)}:${entry.line}</span>
          </div>
        `).join("") || '<p class="empty">None</p>'}
      </div>
    </section>
  `).join("");
}

function renderDiagrams() {
  const stamp = encodeURIComponent(state.model.generated_at);
  $("#diagram-real").src = `${state.model.diagrams.architecture_real}?t=${stamp}`;
  $("#diagram-planned").src = `${state.model.diagrams.architecture_planned}?t=${stamp}`;
  $("#diagram-modules").src = `${state.model.diagrams.module_dependencies}?t=${stamp}`;
}

function renderSelection() {
  const target = $("#selection");
  if (!state.selected) {
    target.innerHTML = '<p class="empty">None</p>';
    return;
  }
  if (state.selected.type === "requirement") {
    const req = requirementById(state.selected.id);
    target.innerHTML = `
      <div class="kv"><span>ID</span><strong>${escapeHtml(req.id)}</strong></div>
      <div class="kv"><span>Title</span><span>${escapeHtml(req.title)}</span></div>
      <div class="kv"><span>Source</span><span>${escapeHtml(req.path)}:${req.line}</span></div>
      <div class="kv"><span>Work</span><span>${escapeHtml(req.work_items.join(", ") || "None")}</span></div>
      <div class="list-item"><strong>Body</strong><span>${escapeHtml(req.body || "No body")}</span></div>
    `;
    return;
  }
  const item = workById(state.selected.id);
  target.innerHTML = `
    <div class="kv"><span>ID</span><strong>${escapeHtml(item.id)}</strong></div>
    <div class="kv"><span>Requirement</span><span>${escapeHtml(item.requirement_id)}</span></div>
    <div class="kv"><span>State</span><span>${pill(item.state, stateKind(item.state))}</span></div>
    <div class="kv"><span>Owner</span><span>${escapeHtml(item.owner ?? "")}</span></div>
    <div class="kv"><span>Source</span><span>${escapeHtml(item.path)}:${item.line}</span></div>
    <div class="list-item"><strong>Body</strong><span>${escapeHtml(item.body || "No body")}</span></div>
  `;
}

function formPayload(form) {
  const data = {};
  for (const field of Array.from(new FormData(form).entries())) {
    const [name, value] = field;
    data[name] = value;
  }
  for (const checkbox of $all('input[type="checkbox"]', form)) {
    data[checkbox.name] = checkbox.checked;
  }
  return data;
}

function normalizePayload(action, payload) {
  const normalized = { ...payload };
  if (action === "verify-branch" && !normalized.branch) normalized.branch = normalized.ref;
  if (action === "start-work" && !normalized.branch) normalized.branch = normalized.ref;
  if (action === "close-risk") normalized.risk_id = normalized.entry_id;
  if (action === "close-incident") {
    normalized.incident_id = normalized.entry_id;
    normalized.closed = normalized.date;
  }
  if (action === "close-experiment") {
    normalized.experiment_id = normalized.entry_id;
    normalized.result = normalized.evidence;
  }
  return normalized;
}

function formAction(form, payload) {
  if (form.dataset.form === "close-ledger") {
    if (payload.entry_type === "incident") return "close-incident";
    if (payload.entry_type === "experiment") return "close-experiment";
    return "close-risk";
  }
  return form.dataset.form;
}

async function runAction(action, payload = {}) {
  const output = $("#command-output");
  output.textContent = `Running ${action}...`;
  const response = await fetch("/api/action", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action, payload: normalizePayload(action, payload) }),
  });
  const result = await response.json();
  if (!response.ok) {
    output.textContent = result.error || JSON.stringify(result, null, 2);
    return;
  }
  output.textContent = [
    `$ ${result.command}`,
    `exit ${result.returncode}`,
    "",
    result.stdout || "",
    result.stderr ? `stderr:\n${result.stderr}` : "",
  ].filter(Boolean).join("\n");
  if (result.returncode === 0) {
    await loadModel();
  }
}

function buttonPayload(button) {
  const payload = {};
  if (button.dataset.requirementId) payload.requirement_id = button.dataset.requirementId;
  if (button.dataset.ref) payload.ref = button.dataset.ref;
  return payload;
}

function wireEvents() {
  $("#refresh").addEventListener("click", loadModel);
  $("#clear-output").addEventListener("click", () => { $("#command-output").textContent = ""; });
  $("#requirement-filter").addEventListener("input", renderRequirements);
  $("#work-filter").addEventListener("input", renderWorkItems);

  for (const item of $all(".nav-item")) {
    item.addEventListener("click", () => {
      for (const other of $all(".nav-item")) other.classList.remove("active");
      for (const view of $all(".view")) view.classList.remove("active");
      item.classList.add("active");
      $(`#view-${item.dataset.view}`).classList.add("active");
    });
  }

  document.addEventListener("click", (event) => {
    const selectRequirement = event.target.closest("[data-select-requirement]");
    if (selectRequirement) {
      state.selected = { type: "requirement", id: selectRequirement.dataset.selectRequirement };
      renderSelection();
      return;
    }
    const selectWork = event.target.closest("[data-select-work]");
    if (selectWork) {
      state.selected = { type: "work", id: selectWork.dataset.selectWork };
      renderSelection();
      return;
    }
    const actionButton = event.target.closest("[data-action]");
    if (actionButton) {
      runAction(actionButton.dataset.action, buttonPayload(actionButton));
    }
  });

  for (const form of $all("[data-form]")) {
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const payload = formPayload(form);
      const action = formAction(form, payload);
      if (["release", "branch-tools", "quick-tools"].includes(action)) return;
      runAction(action, payload);
    });
  }

  for (const button of $all("[data-form-action]")) {
    button.addEventListener("click", () => {
      const form = button.closest("[data-form]");
      runAction(button.dataset.formAction, formPayload(form));
    });
  }
}

wireEvents();
loadModel().catch((err) => {
  $("#command-output").textContent = err.stack || String(err);
});
