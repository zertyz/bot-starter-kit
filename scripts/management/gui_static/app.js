const state = {
  model: null,
  selected: null,
  staticMode: Boolean(window.MANAGEMENT_STATIC_MODE),
  actionInFlight: false,
};

const localOnlyTooltip = "Only available in the local online mode.";
const capabilityToken = document.querySelector('meta[name="management-capability-token"]')?.content ?? "";

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

function attr(value) {
  return escapeHtml(value).replaceAll("\n", "&#10;");
}

function pill(text, kind = "", title = "") {
  const titleAttr = title ? ` title="${attr(title)}"` : "";
  return `<span class="pill ${kind}"${titleAttr}>${escapeHtml(text)}</span>`;
}

function stateKind(value) {
  if (["Merged", "Rolled Out"].includes(value)) return "green";
  if (["Rejected", "Cancelled"].includes(value) || value.startsWith("Superseded by ")) return "red";
  if (["Started", "In Code Review", "Integrated", "QA"].includes(value)) return "blue";
  return "orange";
}

function severityKind(severity) {
  if (severity === "BLOCKER") return "red";
  if (severity === "REVIEW") return "orange";
  if (severity === "INFO") return "blue";
  return "green";
}

function highestSeverity(items) {
  if (!items || !items.length) return "OK";
  if (items.some((item) => item.severity === "BLOCKER")) return "BLOCKER";
  if (items.some((item) => item.severity === "REVIEW")) return "REVIEW";
  return "INFO";
}

function issueTitle(items) {
  if (!items || !items.length) return "No proactive issue detected.";
  return items.map((item) => {
    const location = item.edit_hint ? ` (${item.edit_hint})` : "";
    const category = item.category ? `${item.category}: ` : "";
    return `${item.severity}: ${category}${item.message}${location}`;
  }).join("\n");
}

function auditPill(findings) {
  const severity = highestSeverity(findings);
  if (severity === "OK") return pill("OK", "green", "No audit finding for this requirement.");
  const count = findings.filter((item) => item.severity === severity).length;
  return pill(`${severity} ${count}`, severityKind(severity), issueTitle(findings));
}

function tracePill(req) {
  const auditTrace = req.audit_findings.filter((finding) => finding.category === "traceability");
  const title = [
    req.traceability_detail,
    req.traceability_lines.length ? `Rows: ${req.traceability_lines.join(", ")}` : "",
    issueTitle(auditTrace),
  ].filter(Boolean).join("\n");
  if (req.traceability_status === "direct") return pill("linked", "green", title);
  if (req.traceability_status === "child") return pill("child", "blue", title);
  if (req.traceability_status === "area") return pill("area", "orange", title);
  return pill("gap", "orange", title);
}

function rowClassForIssues(items) {
  const severity = highestSeverity(items);
  if (severity === "BLOCKER") return "row-blocker";
  if (severity === "REVIEW") return "row-review";
  return "";
}

function localOnlyMessage(action) {
  return `${action} is only available in the local online mode.`;
}

function embeddedModelPayload() {
  const embedded = $("#management-model");
  const text = embedded?.textContent?.trim();
  if (!text) return null;
  return JSON.parse(text);
}

async function loadModel() {
  const embedded = state.staticMode ? embeddedModelPayload() : null;
  if (embedded) {
    state.model = embedded;
  } else {
    const modelUrl = state.staticMode ? (window.MANAGEMENT_STATIC_MODEL_URL || "./model.json") : "/api/model";
    const response = await fetch(modelUrl);
    if (!response.ok) throw new Error(await response.text());
    state.model = await response.json();
  }
  state.staticMode = state.staticMode || state.model.surface?.mode === "static";
  render();
}

function render() {
  const model = state.model;
  renderMode();
  $("#repo-root").textContent = model.repo_root;
  $("#generated-at").textContent = `Generated ${model.generated_at}`;
  renderMetrics();
  renderStateCounts();
  renderAttention();
  renderRequirements();
  renderWorkItems();
  renderSelects();
  renderLedgers();
  renderTechDebt();
  renderDiagrams();
  renderSelection();
  applyRuntimeMode();
}

function renderMode() {
  const title = window.MANAGEMENT_TITLE || (state.staticMode ? "Management Report" : "Management Console");
  document.title = title;
  $("#app-title").textContent = title;
  const mode = $("#mode-label");
  mode.textContent = state.staticMode ? "Static offline report" : "Local online mode";
  mode.title = state.staticMode
    ? "Generated from git-controlled files. Server-backed refresh and actions are disabled."
    : "Connected to the local management GUI server.";
}

function applyRuntimeMode() {
  const localOnlyControls = [
    $("#refresh"),
    ...$all("[data-action]"),
    ...$all("[data-form-action]"),
    ...$all('form[data-form] button[type="submit"]'),
  ].filter(Boolean);
  const explanation = $("#local-only-explanation");
  explanation.hidden = !state.staticMode;
  $("#command-output").setAttribute("aria-busy", state.actionInFlight ? "true" : "false");
  for (const control of localOnlyControls) {
    if (state.staticMode) {
      control.disabled = true;
      control.classList.add("local-only");
      control.title = localOnlyTooltip;
      control.setAttribute("aria-describedby", "local-only-explanation");
      continue;
    }
    control.disabled = state.actionInFlight;
    control.classList.remove("local-only");
    control.removeAttribute("aria-describedby");
  }
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
    ["audit_blockers", "Audit Blockers"],
    ["audit_reviews", "Audit Reviews"],
    ["tech_debt_findings", "Debt Leads"],
  ];
  $("#metrics").innerHTML = labels.map(([key, label]) => `
    <div class="metric">
      <strong>${escapeHtml(state.model.counts[key])}</strong>
      <span>${escapeHtml(label)}</span>
    </div>
  `).join("");
}

function renderStateCounts() {
  const additionalStates = Object.keys(state.model.state_counts)
    .filter((name) => !stateOrder.includes(name))
    .sort();
  const rows = [...stateOrder, ...additionalStates]
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
  for (const finding of state.model.audit_findings.filter((item) => item.severity === "BLOCKER").slice(0, 8)) {
    items.push(`<div class="list-item"><strong class="error">${escapeHtml(finding.requirement_id)} ${escapeHtml(finding.category)}</strong><span>${escapeHtml(finding.message)} · ${escapeHtml(finding.edit_hint)}</span></div>`);
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

function blockHistoryText(item) {
  return item.block_history
    .map((event) => `${event.action}: ${event.entered_at} -- ${event.reason}`)
    .join("\n") || "None";
}

function gateOverrideText(item) {
  return item.gate_overrides
    .map((event) => `${event.target_state}: ${event.entered_at} -- ${event.reason}`)
    .join("\n") || "None";
}

function historyExceptionText(item) {
  return item.history_exceptions
    .map((event) => `${event.entered_at}: skipped ${event.missing_states.join(", ")} -- ${event.reason}`)
    .join("\n") || "None";
}

function renderRequirements() {
  const filter = $("#requirement-filter").value.toLowerCase();
  const rows = state.model.requirements
    .filter((req) => `${req.id} ${req.title} ${req.body}`.toLowerCase().includes(filter))
    .map((req) => `
      <tr class="${rowClassForIssues(req.audit_findings)}" title="${attr(issueTitle(req.audit_findings))}">
        <td><button type="button" data-select-requirement="${escapeHtml(req.id)}">${escapeHtml(req.id)}</button></td>
        <td>${escapeHtml(req.title)}</td>
        <td>${auditPill(req.audit_findings)}</td>
        <td>${req.work_items.length}</td>
        <td>${tracePill(req)}</td>
        <td class="actions">
          <button type="button" data-action="show-requirement" data-requirement-id="${escapeHtml(req.id)}" title="Show requirement text and source location.">Show</button>
          <button type="button" class="${highestSeverity(req.audit_findings) === "BLOCKER" ? "danger" : highestSeverity(req.audit_findings) === "REVIEW" ? "warn" : ""}" data-action="estimate-requirement" data-requirement-id="${escapeHtml(req.id)}" title="${attr(issueTitle(req.audit_findings))}">Estimate</button>
          <button type="button" class="${req.traceability_status === "missing" || req.traceability_status === "area" ? "warn" : ""}" data-action="sync-requirement" data-requirement-id="${escapeHtml(req.id)}" title="${attr(req.traceability_detail)}">Sync</button>
          <button type="button" class="${req.traceability_status === "missing" || req.traceability_status === "area" ? "warn" : ""}" data-action="trace-requirement" data-requirement-id="${escapeHtml(req.id)}" title="${attr(req.traceability_detail)}">Trace</button>
        </td>
      </tr>
    `).join("");
  $("#requirements-table").innerHTML = rows || `<tr><td colspan="6" class="empty">None</td></tr>`;
}

function renderWorkItems() {
  const filter = $("#work-filter").value.toLowerCase();
  const rows = state.model.work_items
    .filter((item) => `${item.id} ${item.requirement_id} ${item.title} ${item.state} ${item.owner ?? ""} ${item.blocked} ${blockHistoryText(item)} ${gateOverrideText(item)} ${historyExceptionText(item)}`.toLowerCase().includes(filter))
    .map((item) => `
      <tr class="${rowClassForIssues(item.attention)}" title="${attr(issueTitle(item.attention))}">
        <td><button type="button" data-select-work="${escapeHtml(item.id)}">${escapeHtml(item.id)}</button></td>
        <td>${escapeHtml(item.requirement_id)}</td>
        <td>${pill(item.state, stateKind(item.state))} ${item.blocked ? pill("Blocked", "red", blockHistoryText(item)) : ""}</td>
        <td>${escapeHtml(item.owner ?? "")}</td>
        <td>${escapeHtml(item.title)}</td>
        <td>${auditPill(item.attention)}</td>
        <td class="actions">
          <button type="button" data-action="show-work" data-ref="${escapeHtml(item.id)}">Show</button>
          <button type="button" class="${highestSeverity(item.attention) === "BLOCKER" ? "danger" : item.attention.length ? "warn" : ""}" data-action="evaluate-plan" data-ref="${escapeHtml(item.id)}" title="${attr(issueTitle(item.attention))}">Evaluate</button>
          <button type="button" class="${["Started", "In Code Review", "QA"].includes(item.state) ? "" : "warn"}" data-action="verification-check" data-ref="${escapeHtml(item.id)}" title="Most useful for Started, In Code Review, or QA work.">Verify</button>
          <button type="button" class="${["Started", "In Code Review", "QA"].includes(item.state) ? "" : "warn"}" data-action="review" data-ref="${escapeHtml(item.id)}" title="Normal implementation review usually happens after Started.">Review</button>
          <button type="button" class="${["Merged", "Rolled Out"].includes(item.state) ? "" : "warn"}" data-action="acceptance-packet" data-ref="${escapeHtml(item.id)}" title="Acceptance is normally final for Merged or Rolled Out work.">Accept</button>
        </td>
      </tr>
    `).join("");
  $("#work-table").innerHTML = rows || `<tr><td colspan="7" class="empty">None</td></tr>`;
}

function renderSelects() {
  const reqOptions = state.model.requirements.map((req) => `<option value="${escapeHtml(req.id)}">${escapeHtml(req.id)} -- ${escapeHtml(req.title)}</option>`).join("");
  const workOptions = state.model.work_items.map((item) => `<option value="${escapeHtml(item.id)}">${escapeHtml(item.id)} -- ${escapeHtml(item.state)}</option>`).join("");
  for (const select of $all('[data-select="requirements"]')) select.innerHTML = reqOptions;
  for (const select of $all('[data-select="work-items"]')) select.innerHTML = workOptions;
  renderAdvanceStateTargets();
}

function renderAdvanceStateTargets() {
  const currentWorkId = $("#advance-state-work").value;
  const lifecycleOptions = stateOrder.map((item) => `<option>${escapeHtml(item)}</option>`).join("");
  const supersessionOptions = state.model.work_items
    .filter((item) => item.id !== currentWorkId)
    .map((item) => {
      const targetState = `Superseded by ${item.id}`;
      return `<option value="${attr(targetState)}">${escapeHtml(targetState)}</option>`;
    }).join("");
  $("#advance-state-target").innerHTML = `<option value="">Next state</option>${lifecycleOptions}${supersessionOptions}`;
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
  const autoDebt = state.model.ledgers.tech_debts.auto_detected.slice(0, 8).map((finding) => `
    <div class="list-item" title="${attr(finding.message + "\n" + finding.edit_hint)}">
      <strong>${escapeHtml(finding.category)} · ${escapeHtml(finding.path)}:${finding.line}</strong>
      <span>${escapeHtml(finding.message)}</span>
    </div>
  `).join("");
  const confirmedDebt = state.model.ledgers.tech_debts.confirmed.slice(0, 8).map((item) => `
    <div class="list-item" title="${attr(item.edit_hint)}">
      <strong>${escapeHtml(item.id)} -- ${escapeHtml(item.title)}</strong>
      <span>${escapeHtml(item.kind)} ${escapeHtml(item.state || "")} ${escapeHtml(item.path)}:${item.line}</span>
    </div>
  `).join("");
  $("#ledger-grid").insertAdjacentHTML("beforeend", `
    <section class="panel">
      <h2>Auto-detected Tech Debt</h2>
      <div class="list">${autoDebt || '<p class="empty">None</p>'}</div>
    </section>
    <section class="panel">
      <h2>Confirmed Tech Debt</h2>
      <div class="list">${confirmedDebt || '<p class="empty">None</p>'}</div>
    </section>
  `);
}

function renderTechDebt() {
  const filter = ($("#tech-debt-filter")?.value ?? "").toLowerCase();
  const auto = state.model.tech_debts.auto_detected
    .filter((finding) => `${finding.category} ${finding.severity} ${finding.path} ${finding.message}`.toLowerCase().includes(filter))
    .map((finding) => `
      <div class="list-item ${finding.category}" title="${attr(finding.message + "\nEdit: " + finding.edit_hint)}">
        <strong>${pill(finding.severity, severityKind(finding.severity))} ${escapeHtml(finding.category)} · ${escapeHtml(finding.path)}:${finding.line}</strong>
        <span>${escapeHtml(finding.message)}</span>
      </div>
    `).join("");
  const confirmed = state.model.tech_debts.confirmed
    .filter((item) => `${item.kind} ${item.id} ${item.title} ${item.state} ${item.path}`.toLowerCase().includes(filter))
    .map((item) => `
      <div class="list-item" title="${attr(item.edit_hint)}">
        <strong>${escapeHtml(item.id)} -- ${escapeHtml(item.title)}</strong>
        <span>${escapeHtml(item.kind)} ${escapeHtml(item.state || "")} ${escapeHtml(item.path)}:${item.line}</span>
      </div>
    `).join("");
  $("#tech-debt-grid").innerHTML = `
    <section class="panel">
      <h2>Auto-detected</h2>
      <div class="list">${auto || '<p class="empty">None</p>'}</div>
    </section>
    <section class="panel">
      <h2>Confirmed</h2>
      <div class="list">${confirmed || '<p class="empty">None</p>'}</div>
    </section>
  `;
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
      <div class="kv"><span>Audit</span><span>${auditPill(req.audit_findings)}</span></div>
      <div class="kv"><span>Trace</span><span>${tracePill(req)}</span></div>
      <div class="list">${req.audit_findings.map((finding) => `
        <div class="list-item" title="${attr(finding.edit_hint)}">
          <strong>${escapeHtml(finding.severity)} ${escapeHtml(finding.category)}</strong>
          <span>${escapeHtml(finding.message)} · ${escapeHtml(finding.edit_hint)}</span>
        </div>
      `).join("") || '<p class="empty">No audit finding</p>'}</div>
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
    <div class="kv"><span>Blocked</span><span>${item.blocked ? pill("yes", "red") : pill("no", "green")}</span></div>
    <div class="kv"><span>Source</span><span>${escapeHtml(item.path)}:${item.line}</span></div>
    <div class="kv"><span>Attention</span><span>${auditPill(item.attention)}</span></div>
    <div class="list">${item.attention.map((signal) => `
      <div class="list-item" title="${attr(signal.edit_hint || "")}">
        <strong>${escapeHtml(signal.severity)}</strong>
        <span>${escapeHtml(signal.message)}${signal.edit_hint ? ` · ${escapeHtml(signal.edit_hint)}` : ""}</span>
      </div>
    `).join("") || '<p class="empty">No proactive signal</p>'}</div>
    <div class="list-item"><strong>Block History</strong><span class="history-text">${escapeHtml(blockHistoryText(item))}</span></div>
    <div class="list-item"><strong>Gate Overrides</strong><span class="history-text">${escapeHtml(gateOverrideText(item))}</span></div>
    <div class="list-item"><strong>History Exceptions</strong><span class="history-text">${escapeHtml(historyExceptionText(item))}</span></div>
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
  if (state.staticMode) {
    output.textContent = localOnlyMessage(action);
    return;
  }
  if (state.actionInFlight) {
    output.textContent = "An action is already running.";
    return;
  }
  state.actionInFlight = true;
  applyRuntimeMode();
  output.textContent = `Running ${action}...`;
  try {
    const response = await fetch("/api/action", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Management-Capability": capabilityToken,
      },
      body: JSON.stringify({ action, payload: normalizePayload(action, payload) }),
    });
    const responseText = await response.text();
    let result;
    try {
      result = JSON.parse(responseText);
    } catch (_err) {
      result = { error: responseText };
    }
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
  } catch (err) {
    output.textContent = err.stack || String(err);
  } finally {
    state.actionInFlight = false;
    applyRuntimeMode();
  }
}

async function refreshModel() {
  if (state.actionInFlight) return;
  state.actionInFlight = true;
  applyRuntimeMode();
  try {
    await loadModel();
  } catch (err) {
    $("#command-output").textContent = err.stack || String(err);
  } finally {
    state.actionInFlight = false;
    applyRuntimeMode();
  }
}

function buttonPayload(button) {
  const payload = {};
  if (button.dataset.requirementId) payload.requirement_id = button.dataset.requirementId;
  if (button.dataset.ref) payload.ref = button.dataset.ref;
  return payload;
}

function wireEvents() {
  $("#refresh").addEventListener("click", refreshModel);
  $("#clear-output").addEventListener("click", () => { $("#command-output").textContent = ""; });
  $("#requirement-filter").addEventListener("input", renderRequirements);
  $("#work-filter").addEventListener("input", renderWorkItems);
  $("#tech-debt-filter").addEventListener("input", renderTechDebt);
  $("#advance-state-work").addEventListener("change", renderAdvanceStateTargets);

  for (const item of $all(".nav-item")) {
    item.addEventListener("click", () => {
      for (const other of $all(".nav-item")) {
        other.classList.remove("active");
        other.removeAttribute("aria-current");
      }
      for (const view of $all(".view")) view.classList.remove("active");
      item.classList.add("active");
      item.setAttribute("aria-current", "page");
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
      if (state.staticMode) {
        $("#command-output").textContent = localOnlyMessage(actionButton.dataset.action);
        return;
      }
      runAction(actionButton.dataset.action, buttonPayload(actionButton));
    }
  });

  for (const form of $all("[data-form]")) {
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (state.staticMode) {
        $("#command-output").textContent = localOnlyMessage(form.dataset.form);
        return;
      }
      const payload = formPayload(form);
      const action = formAction(form, payload);
      if (["release", "branch-tools", "quick-tools", "block-tools"].includes(action)) return;
      runAction(action, payload);
    });
  }

  for (const button of $all("[data-form-action]")) {
    button.addEventListener("click", () => {
      if (state.staticMode) {
        $("#command-output").textContent = localOnlyMessage(button.dataset.formAction);
        return;
      }
      const form = button.closest("[data-form]");
      runAction(button.dataset.formAction, formPayload(form));
    });
  }
}

wireEvents();
renderMode();
applyRuntimeMode();
loadModel().catch((err) => {
  $("#command-output").textContent = err.stack || String(err);
});
