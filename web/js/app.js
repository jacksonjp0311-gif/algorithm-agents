let catalog = { accepted: [], queue: [] };
let lastFocus = null;
let domainFilter = "";
const startupParams = new URLSearchParams(location.search);
if (startupParams.get("operator_token")) {
  sessionStorage.setItem("alchetron_operator_token", startupParams.get("operator_token"));
  startupParams.delete("operator_token");
  const cleanQuery = startupParams.toString();
  history.replaceState({}, "", `${location.pathname}${cleanQuery ? `?${cleanQuery}` : ""}${location.hash}`);
}
const operatorToken = sessionStorage.getItem("alchetron_operator_token") || "";
let actionResolver = null;

function showToast(message) {
  const toast = document.querySelector("#toast");
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add("is-visible");
  window.clearTimeout(showToast.timer);
  showToast.timer = window.setTimeout(() => toast.classList.remove("is-visible"), 3200);
}

function fieldHtml(field) {
  const required = field.required === false ? "" : " required";
  const value = esc(field.value || "");
  if (field.type === "textarea") {
    return `<div class="action-field"><label for="action-${esc(field.name)}">${esc(field.label)}</label><textarea id="action-${esc(field.name)}" name="${esc(field.name)}"${required}>${value}</textarea></div>`;
  }
  if (field.type === "select") {
    return `<div class="action-field"><label for="action-${esc(field.name)}">${esc(field.label)}</label><select id="action-${esc(field.name)}" name="${esc(field.name)}"${required}>${(field.options || []).map((option) => `<option value="${esc(option.value)}">${esc(option.label)}</option>`).join("")}</select></div>`;
  }
  return `<div class="action-field"><label for="action-${esc(field.name)}">${esc(field.label)}</label><input id="action-${esc(field.name)}" name="${esc(field.name)}" type="${esc(field.type || "text")}" value="${value}"${required}></div>`;
}

window.openGovernedAction = function openGovernedAction(config) {
  const dialog = document.querySelector("#action-dialog");
  if (!dialog) return Promise.resolve(null);
  document.querySelector("#action-kicker").textContent = config.kicker || "Governed action";
  document.querySelector("#action-title").textContent = config.title || "Review";
  document.querySelector("#action-explainer").textContent = config.explainer || "";
  document.querySelector("#action-boundary").textContent = config.boundary || "This action is recorded in the event ledger.";
  document.querySelector("#action-submit").textContent = config.submitLabel || "Continue";
  document.querySelector("#action-fields").innerHTML = (config.fields || []).map(fieldHtml).join("");
  if (dialog.open) dialog.close();
  dialog.showModal();
  requestAnimationFrame(() => dialog.querySelector("input, textarea, select")?.focus());
  return new Promise((resolve) => { actionResolver = resolve; });
};

function closeAction(result = null) {
  const dialog = document.querySelector("#action-dialog");
  if (dialog?.open) dialog.close();
  const resolve = actionResolver;
  actionResolver = null;
  if (resolve) resolve(result);
}

function esc(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function glowFor(item) {
  const domain = String(item.domain || "").toLowerCase();
  if (domain.includes("graph")) return "ice";
  if (domain.includes("state") || domain.includes("filter")) return "magenta";
  if (domain.includes("optim")) return "triad";
  if (domain.includes("numeric")) return "frost";
  if (domain.includes("monte") || domain.includes("sampl")) return "red";
  if (["pending", "needs_human", "blocked", "rejected"].includes(item.status)) return "vacant";
  return "alchemy";
}

function extract(item) {
  return item.extraction && typeof item.extraction === "object" ? item.extraction : {};
}

function matchesQuery(item, query, domain) {
  if (domain && item.domain !== domain) return false;
  if (!query) return true;
  const body = extract(item);
  const hay = [
    item.title,
    item.short_name,
    item.domain,
    item.summary,
    item.core_idea,
    item.archive_id,
    ...(item.tags || []),
    body.source?.title,
    ...(body.source?.authors || []),
    ...(body.known_uses || []),
  ]
    .join(" ")
    .toLowerCase();
  return hay.includes(query);
}

function clip(text, n) {
  const value = String(text || "").replace(/\s+/g, " ").trim();
  if (value.length <= n) return value;
  return `${value.slice(0, n).trim()}…`;
}

function sourceHref(item) {
  const ext = extract(item);
  const value = String(ext.source?.url || ext.source?.repository_url || "").trim();
  return /^https?:\/\//i.test(value) ? value : "";
}

function cardHtml(item) {
  const body = extract(item);
  const glow = glowFor(item);
  const uses = (body.known_uses || item.tags || []).slice(0, 3).join(" · ");
  const authors = (body.source?.authors || []).join(", ");
  const teaser = item.core_idea
    ? `<div class="archive-teaser">${esc(clip(item.core_idea, 180))}</div>`
    : "";
  const tags = item.tags || [];
  const tagList = tags.length
    ? `<ul class="software-tags">${tags.map((tag) => `<li>${esc(tag)}</li>`).join("")}</ul>`
    : "";
  const pending = item.status === "pending";
  const contested = item.status === "needs_human" || item.status === "blocked";
  const rejected = item.status === "rejected";
  const href = sourceHref(item);
  const actions = pending || contested
    ? `<button type="button" class="button" data-open="${esc(item.id)}">Inspect</button>
      <div class="button-row">
        <button type="button" class="button button-ghost" data-accept="${esc(item.id)}">Accept</button>
        <button type="button" class="button button-ghost" data-reject="${esc(item.id)}">Reject</button>
      </div>`
    : `<button type="button" class="button" data-open="${esc(item.id)}">Inspect</button>`;
  return `<article class="archive-card is-inspectable${pending ? " is-pending" : ""}${contested ? " is-contested" : ""}${rejected ? " is-rejected" : ""}" data-glow="${glow}" data-id="${esc(item.id)}" data-open-card="${esc(item.id)}" tabindex="0">
    <p class="status-pill">${esc(item.status || "accepted")}</p>
    <p class="archive-id">${esc(item.archive_id)}</p>
    <h2>${esc(item.title || "Untitled")}</h2>
    <p class="meta-row">${esc(item.domain || "Unsorted")}${body.source?.type ? " · " + esc(body.source.type) : ""}</p>
    <p>${esc(clip(item.summary || body.plain_language_explanation || "No summary yet.", 240))}</p>
    ${teaser}
    <p class="meta-row"><strong>Source</strong> ${esc(authors || body.source?.title || href || "—")}${body.source?.publication_date ? " · " + esc(body.source.publication_date) : ""}</p>
    <p class="meta-row"><strong>Uses</strong> ${esc(uses || "—")}</p>
    ${tagList}
    ${actions}
  </article>`;
}

function vacantCard() {
  return `<article class="archive-card is-vacant" data-glow="vacant" aria-label="Vacant berth">
    <p class="status-pill">vacant</p>
    <p class="archive-id">BERTH</p>
    <h2>Vacant</h2>
    <p class="meta-row">Awaiting harvest</p>
    <p>Nothing accepted in this archive yet. Run <code>algo harvest</code> or <code>algo scrape</code>, then accept a candidate.</p>
  </article>`;
}

function filtered(list) {
  const query = String(document.querySelector("#archive-search")?.value || "").toLowerCase();
  const sort = document.querySelector("#archive-sort")?.value || "domain";
  const items = list.filter((item) => matchesQuery(item, query, domainFilter));
  items.sort((a, b) => {
    if (sort === "title") return String(a.title).localeCompare(b.title);
    if (sort === "newest") return String(b.created_at || "").localeCompare(String(a.created_at || ""));
    return String(a.domain).localeCompare(b.domain) || String(a.title).localeCompare(b.title);
  });
  return items;
}

function renderMetrics() {
  const mount = document.querySelector("#archive-metrics");
  if (!mount) return;
  const pending = (catalog.queue || []).filter((item) => item.status === "pending").length;
  const contested = (catalog.queue || []).filter((item) => ["needs_human", "blocked"].includes(item.status)).length;
  mount.innerHTML = `
    <div class="archive-metric-row">
      <span class="archive-metric"><strong>${(catalog.accepted || []).length}</strong> in the archive</span>
      <span class="archive-metric"><strong>${pending}</strong> awaiting review</span>
      <span class="archive-metric"><strong>${contested}</strong> contested</span>
    </div>`;
}

function renderChips() {
  const mount = document.querySelector("#archive-chips");
  if (!mount) return;
  const items = [...(catalog.accepted || []), ...(catalog.queue || [])];
  const counts = {};
  for (const item of items) {
    const key = item.domain || "Unsorted";
    counts[key] = (counts[key] || 0) + 1;
  }
  const domains = Object.keys(counts).sort();
  mount.innerHTML = [`<button type="button" class="archive-chip${domainFilter ? "" : " is-on"}" data-domain="">All <em>${items.length}</em></button>`]
    .concat(domains.map((domain) => {
      const on = domainFilter === domain ? " is-on" : "";
      return `<button type="button" class="archive-chip${on}" data-domain="${esc(domain)}">${esc(domain)} <em>${counts[domain]}</em></button>`;
    }))
    .join("");
  mount.querySelectorAll("[data-domain]").forEach((button) => {
    button.addEventListener("click", () => {
      domainFilter = button.dataset.domain || "";
      paint();
    });
  });
}

function bindCards(root) {
  root.querySelectorAll("[data-open-card]").forEach((card) => {
    card.addEventListener("click", (event) => {
      if (event.target.closest("[data-accept], [data-reject], [data-open]")) return;
      openHud(card.dataset.openCard, card);
    });
    card.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openHud(card.dataset.openCard, card);
      }
    });
  });
  root.querySelectorAll("[data-open]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      openHud(button.dataset.open, button);
    });
  });
  root.querySelectorAll("[data-accept]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      accept(button.dataset.accept);
    });
  });
  root.querySelectorAll("[data-reject]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      reject(button.dataset.reject);
    });
  });
  root.querySelectorAll("[data-edit]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      editCandidate(button.dataset.edit);
    });
  });
}

function renderQueue() {
  const mount = document.querySelector("#queue-grid");
  if (!mount) return;
  const items = filtered(catalog.queue || []).filter((item) => item.status === "pending");
  if (!items.length) {
    mount.innerHTML = `<div class="empty-state">No candidates in the queue. Harvest or scrape a source, then it shows up here.</div>`;
    return;
  }
  mount.innerHTML = items.map(cardHtml).join("");
  bindCards(mount);
}

function renderArchive() {
  const mount = document.querySelector("#archive-grid");
  if (!mount) return;
  const items = filtered(catalog.accepted || []);
  const sort = document.querySelector("#archive-sort")?.value || "domain";
  if (!items.length) {
    mount.innerHTML = vacantCard();
    return;
  }
  if (sort === "domain") {
    const groups = new Map();
    for (const item of items) {
      const key = item.domain || "Unsorted";
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(item);
    }
    mount.innerHTML = [...groups.entries()]
      .map(([domain, rows]) => `<section class="archive-group"><h2 class="archive-group-title">${esc(domain)} <em>${rows.length}</em></h2><div class="archive-grid">${rows.map(cardHtml).join("")}</div></section>`)
      .join("");
  } else {
    mount.innerHTML = items.map(cardHtml).join("");
  }
  bindCards(mount);
}

function renderContested() {
  const mount = document.querySelector("#contested-grid");
  if (!mount) return;
  const items = filtered(catalog.queue || []).filter((item) => ["needs_human", "blocked"].includes(item.status));
  mount.innerHTML = items.length
    ? items.map(cardHtml).join("")
    : `<div class="empty-state">No contested candidates. Uncertainty and failed gates will remain visible here.</div>`;
  bindCards(mount);
}

function listHtml(items) {
  if (!items || !items.length) return "<p class='meta-row'>None listed.</p>";
  return `<ul>${items.map((item) => `<li>${esc(item)}</li>`).join("")}</ul>`;
}

function findItem(id) {
  return [...(catalog.accepted || []), ...(catalog.queue || [])].find((item) => item.id === id || item.archive_id === id);
}

function openHud(id, origin) {
  const item = findItem(id);
  const dialog = document.querySelector("#archive-hud");
  const body = document.querySelector("#hud-body");
  if (!item || !dialog || !body) return;
  lastFocus = origin || document.activeElement;
  const ext = extract(item);
  const sourceUrl = sourceHref(item);
  const pending = item.status === "pending";
  const reviewable = pending || item.status === "needs_human" || item.status === "blocked";
  body.innerHTML = `
    <p class="eyebrow">Inspect · ${esc(item.archive_id)}</p>
    <h2 id="hud-title">${esc(item.title)}</h2>
    <p class="meta-row">${esc(item.domain || "Unsorted")} · ${esc(item.status).toUpperCase()}</p>
    <nav class="inspect-nav" aria-label="Inspect sections">
      <a href="#inspect-overview">Overview</a>
      <a href="#inspect-verification">Verification</a>
      <a href="#inspect-evidence">Evidence</a>
      <a href="#inspect-math">Math</a>
      <a href="#inspect-code">Code</a>
      <a href="#inspect-source">Source</a>
    </nav>
    ${sourceUrl ? `<p><a class="text-link" href="${esc(sourceUrl)}" target="_blank" rel="noopener">Open original source</a></p>` : ""}
    <section id="inspect-overview"><h3>Overview</h3>
      <p>${esc(ext.plain_language_explanation || item.summary || "No overview was extracted.")}</p>
      <p><strong>Core idea.</strong> ${esc(item.core_idea || ext.core_idea || "—")}</p>
      <p><strong>Why it matters.</strong> ${esc(ext.why_it_matters || "—")}</p>
    </section>
    <section id="inspect-verification"><h3>Verification</h3>
      <p class="status-pill">${esc(ext.validation?.status || "UNVERIFIED")}</p>
      <p><strong>Lifecycle.</strong> ${esc(ext.validation?.lifecycle || "DRAFT")}</p>
      <p><strong>Mandatory gates.</strong> ${ext.validation?.mandatory_pass ? "Passed" : "Needs human judgment"}</p>
      <pre>${esc(JSON.stringify(ext.validation?.gates || {}, null, 2))}</pre>
      <p class="meta-row">Candidate hash ${esc(item.candidate_hash || "—")}</p>
    </section>
    <section id="inspect-evidence"><h3>Claim evidence</h3>
      <p class="meta-row">Snapshot ${esc(ext.source_snapshot?.snapshot_id || "legacy source")} · ${esc(String(ext.source_snapshot?.content_hash || "").slice(0, 18) || "hash unavailable")}</p>
      <div class="evidence-grid">${(ext.claims || []).length ? ext.claims.map((claim) => {
        const span = claim.evidence?.[0];
        return `<article class="evidence-claim${span ? "" : " is-inferred"}"><p class="status-pill">${esc(claim.epistemic_status || "UNKNOWN")}</p><strong>${esc(claim.field || "claim")}</strong><p>${esc(claim.text)}</p>${span ? `<blockquote>“${esc(span.quote)}”</blockquote><small>Lines ${esc(span.start_line)}–${esc(span.end_line)} · bytes ${esc(span.start_byte)}–${esc(span.end_byte)}</small>` : `<small>No exact source span. Human verification required.</small>`}</article>`;
      }).join("") : `<p class="empty-state">Legacy candidate: no claim-level evidence package.</p>`}</div>
    </section>
    <section id="inspect-math"><h3>Mathematics</h3>
      <pre>${esc(ext.math || item.core_idea || "No math field was extracted.")}</pre>
      <h4>Variables</h4>${listHtml(ext.variables)}
      <h4>Assumptions</h4>${listHtml(ext.assumptions)}
      <h4>Constraints</h4>${listHtml(ext.constraints)}
      ${ext.complexity ? `<p><strong>Complexity.</strong> ${esc(ext.complexity)}</p>` : ""}
    </section>
    <section id="inspect-code"><h3>Pseudocode</h3><pre>${esc(ext.pseudocode || "No pseudocode was extracted.")}</pre>
      <h3>Reference implementation</h3>
      <p class="meta-row">${esc(ext.reference_language || "unspecified")}</p>
      <pre>${esc(ext.reference_code || "No reference code was extracted.")}</pre>
    </section>
    <section id="inspect-source"><h3>Source</h3>
      <p>${esc(ext.source?.title || item.title)} (${esc(ext.source?.type || "source")})</p>
      <p>${esc((ext.source?.authors || []).join(", ") || "Authors not listed")} · ${esc(ext.source?.publication_date || "")}</p>
      ${sourceUrl ? `<p class="meta-row">${esc(sourceUrl)}</p>` : ""}
      <p>${esc(ext.provenance?.evidence_notes || "")}</p>
    </section>
    ${reviewable ? `<div class="button-row inspect-decide">
      <button type="button" class="button" data-accept="${esc(item.id)}">Accept into archive</button>
      <button type="button" class="button button-ghost" data-edit="${esc(item.id)}">Edit working candidate</button>
      <button type="button" class="button button-ghost" data-reject="${esc(item.id)}">Reject</button>
    </div>` : ""}
  `;
  bindCards(body);
  if (typeof dialog.showModal === "function") dialog.showModal();
  else dialog.setAttribute("open", "");
}

function closeHud() {
  const dialog = document.querySelector("#archive-hud");
  if (dialog?.open) dialog.close();
  else dialog?.removeAttribute("open");
  if (lastFocus && typeof lastFocus.focus === "function") lastFocus.focus();
}

async function accept(id) {
  const item = findItem(id);
  if (!item) return;
  if (!operatorToken) {
    window.alert("Open the operator URL printed by `algo ui`; a session token is required.");
    return;
  }
  const decision = await window.openGovernedAction({
    kicker: "Canonical publication boundary",
    title: `Review ${id}`,
    explainer: "This creates an immutable canonical revision, receipt, graph update, and rollback point.",
    submitLabel: "Publish reviewed candidate",
    boundary: "Type the exact candidate ID. Agent and model output cannot perform this action.",
    fields: [
      { name: "confirmation", label: `Exact confirmation · ${id}` },
      { name: "reviewer", label: "Human reviewer", value: "human-operator" },
      { name: "reason", label: "Evidence-based review reason", type: "textarea" },
    ],
  });
  if (!decision) return;
  const response = await fetch(`/api/v1/operator/queue/${encodeURIComponent(id)}/accept`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-alchetron-operator-token": operatorToken,
    },
    body: JSON.stringify({
      confirmation: decision.confirmation,
      reviewer: decision.reviewer,
      reason: decision.reason,
      candidate_hash: item.candidate_hash || "",
      override_contested: item.status !== "pending",
    }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: "Accept failed." }));
    showToast(body.error || "Accept failed.");
    return;
  }
  await loadCatalog();
  closeHud();
  showToast(`${id} entered immutable canon.`);
}

async function reject(id) {
  if (!operatorToken) {
    window.alert("Open the operator URL printed by `algo ui`; a session token is required.");
    return;
  }
  const decision = await window.openGovernedAction({
    kicker: "Private review decision",
    title: `Reject ${id}`,
    explainer: "The candidate remains in the audit trail but cannot enter canon.",
    submitLabel: "Record rejection",
    fields: [
      { name: "confirmation", label: `Exact confirmation · ${id}` },
      { name: "reason", label: "Rejection reason", type: "textarea", value: "insufficient evidence" },
    ],
  });
  if (!decision) return;
  const response = await fetch(`/api/v1/operator/queue/${encodeURIComponent(id)}/reject`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-alchetron-operator-token": operatorToken,
    },
    body: JSON.stringify({ reason: decision.reason, confirmation: decision.confirmation }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: "Reject failed." }));
    showToast(body.error || "Reject failed.");
    return;
  }
  await loadCatalog();
  closeHud();
  showToast(`${id} rejection recorded.`);
}

async function editCandidate(id) {
  const item = findItem(id);
  if (!item || !operatorToken) return;
  const ext = structuredClone(extract(item));
  const edit = await window.openGovernedAction({
    kicker: "Working candidate",
    title: `Edit ${id}`,
    explainer: "Edits change private working state and force human revalidation. Canon is untouched.",
    submitLabel: "Save private revision",
    fields: [
      { name: "title", label: "Title", value: ext.title || item.title },
      { name: "summary", label: "Summary", type: "textarea", value: ext.summary || item.summary },
      { name: "core_idea", label: "Core idea", type: "textarea", value: ext.core_idea || item.core_idea },
      { name: "math", label: "Mathematics", type: "textarea", value: ext.math || "", required: false },
      { name: "editor", label: "Human editor", value: "human-operator" },
      { name: "reason", label: "Why this edit is necessary", type: "textarea" },
      { name: "confirmation", label: `Exact confirmation · ${id}` },
    ],
  });
  if (!edit) return;
  Object.assign(ext, { title: edit.title, summary: edit.summary, core_idea: edit.core_idea, math: edit.math });
  const response = await fetch(`/api/v1/operator/queue/${encodeURIComponent(id)}/edit`, {
    method: "POST",
    headers: { "content-type": "application/json", "x-alchetron-operator-token": operatorToken },
    body: JSON.stringify({
      confirmation: edit.confirmation,
      editor: edit.editor,
      reason: edit.reason,
      candidate_hash: item.candidate_hash,
      normalized: ext,
    }),
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) { showToast(body.error || "Candidate edit failed."); return; }
  await loadCatalog();
  closeHud();
  showToast(`${id} private working state updated; revalidation required.`);
}

function paint() {
  renderMetrics();
  renderChips();
  renderQueue();
  renderContested();
  renderArchive();
}

async function loadCatalog() {
  const response = await fetch("/api/catalog", { cache: "no-store" });
  if (!response.ok) throw new Error("Could not load the archive.");
  catalog = await response.json();
  paint();
}

document.querySelector("#hud-close")?.addEventListener("click", closeHud);
document.querySelector("#action-close")?.addEventListener("click", () => closeAction());
document.querySelector("#action-cancel")?.addEventListener("click", () => closeAction());
document.querySelector("#action-dialog")?.addEventListener("cancel", (event) => { event.preventDefault(); closeAction(); });
document.querySelector("#action-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  const values = Object.fromEntries(new FormData(event.currentTarget).entries());
  closeAction(values);
});
document.querySelector("#archive-hud")?.addEventListener("click", (event) => {
  if (event.target.id === "archive-hud") closeHud();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeHud();
});
["#archive-search", "#archive-sort"].forEach((sel) => {
  document.querySelector(sel)?.addEventListener("input", paint);
});

loadCatalog()
  .then(() => {
    const inspectId = startupParams.get("inspect");
    if (inspectId) openHud(inspectId, null);
  })
  .catch((error) => {
    const mount = document.querySelector("#archive-grid");
    if (mount) mount.innerHTML = `<div class="empty-state">${esc(error.message)}</div>`;
  });
setInterval(() => {
  loadCatalog().catch(() => {});
}, 4000);
