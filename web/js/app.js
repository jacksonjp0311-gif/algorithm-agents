let catalog = { accepted: [], queue: [] };
let lastFocus = null;
let domainFilter = "";

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
  if (item.status === "pending" || item.status === "rejected") return "vacant";
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

function cardHtml(item) {
  const body = extract(item);
  const glow = glowFor(item);
  const uses = (body.known_uses || item.tags || []).slice(0, 3).join(" · ");
  const authors = (body.source?.authors || []).join(", ");
  const teaser = item.core_idea
    ? `<div class="archive-teaser">${esc(item.core_idea)}</div>`
    : "";
  const tags = item.tags || [];
  const tagList = tags.length
    ? `<ul class="software-tags">${tags.map((tag) => `<li>${esc(tag)}</li>`).join("")}</ul>`
    : "";
  const pending = item.status === "pending";
  const rejected = item.status === "rejected";
  const actions = pending
    ? `<div class="button-row">
        <button type="button" class="button" data-accept="${esc(item.id)}">Accept</button>
        <button type="button" class="button button-ghost" data-reject="${esc(item.id)}">Reject</button>
      </div>
      <button type="button" class="button button-ghost" data-open="${esc(item.id)}">Open</button>`
    : `<button type="button" class="button" data-open="${esc(item.id)}">Open</button>`;
  return `<article class="archive-card${pending ? " is-pending" : ""}${rejected ? " is-rejected" : ""}" data-glow="${glow}" data-id="${esc(item.id)}">
    <p class="status-pill">${esc(item.status || "accepted")}</p>
    <p class="archive-id">${esc(item.archive_id)}</p>
    <h2>${esc(item.title || "Untitled")}</h2>
    <p class="meta-row">${esc(item.domain || "Unsorted")}${body.source?.type ? " · " + esc(body.source.type) : ""}</p>
    <p>${esc(item.summary || "No summary yet.")}</p>
    ${teaser}
    <p class="meta-row"><strong>Source</strong> ${esc(authors || body.source?.title || "—")}${body.source?.publication_date ? " · " + esc(body.source.publication_date) : ""}</p>
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
  mount.innerHTML = `
    <div class="archive-metric-row">
      <span class="archive-metric"><strong>${(catalog.accepted || []).length}</strong> in the archive</span>
      <span class="archive-metric"><strong>${pending}</strong> awaiting review</span>
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
  root.querySelectorAll("[data-open]").forEach((button) => {
    button.addEventListener("click", () => openHud(button.dataset.open, button));
  });
  root.querySelectorAll("[data-accept]").forEach((button) => {
    button.addEventListener("click", () => accept(button.dataset.accept));
  });
  root.querySelectorAll("[data-reject]").forEach((button) => {
    button.addEventListener("click", () => reject(button.dataset.reject));
  });
}

function renderQueue() {
  const mount = document.querySelector("#queue-grid");
  if (!mount) return;
  const items = filtered(catalog.queue || []).filter((item) => item.status !== "accepted");
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
  const sourceUrl = ext.source?.url || "";
  body.innerHTML = `
    <p class="eyebrow">${esc(item.archive_id)}</p>
    <h2 id="hud-title">${esc(item.title)}</h2>
    <p class="meta-row">Domain ${esc(item.domain)} · ${esc(item.status).toUpperCase()}</p>
    ${sourceUrl ? `<p><a href="${esc(sourceUrl)}" target="_blank" rel="noopener">Open source</a></p>` : ""}
    <section><h3>Overview</h3>
      <p>${esc(ext.plain_language_explanation || item.summary)}</p>
      <p><strong>Core idea.</strong> ${esc(item.core_idea || ext.core_idea || "—")}</p>
      <p><strong>Why it matters.</strong> ${esc(ext.why_it_matters || "—")}</p>
    </section>
    <section><h3>Mathematics</h3>
      <pre>${esc(ext.math || item.core_idea || "")}</pre>
      <h4>Variables</h4>${listHtml(ext.variables)}
      <h4>Assumptions</h4>${listHtml(ext.assumptions)}
      <h4>Constraints</h4>${listHtml(ext.constraints)}
      ${ext.complexity ? `<p><strong>Complexity.</strong> ${esc(ext.complexity)}</p>` : ""}
    </section>
    <section><h3>Pseudocode</h3><pre>${esc(ext.pseudocode || "")}</pre></section>
    <section><h3>Reference implementation</h3>
      <p class="meta-row">${esc(ext.reference_language || "python")}</p>
      <pre>${esc(ext.reference_code || "")}</pre>
    </section>
    <section><h3>Uses</h3>
      <h4>Known common uses</h4>${listHtml(ext.known_uses)}
      <h4>Possible uses</h4>${listHtml(ext.potential_uses)}
    </section>
    <section><h3>Provenance</h3>
      <p>${esc(ext.source?.title || "")} (${esc(ext.source?.type || "")})</p>
      <p>${esc((ext.source?.authors || []).join(", "))} · ${esc(ext.source?.publication_date || "")}</p>
      <p>${esc(ext.provenance?.evidence_notes || "")}</p>
    </section>
  `;
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
  const response = await fetch(`/api/queue/${encodeURIComponent(id)}/accept`, { method: "POST" });
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: "Accept failed." }));
    window.alert(body.error || "Accept failed.");
    return;
  }
  await loadCatalog();
}

async function reject(id) {
  const response = await fetch(`/api/queue/${encodeURIComponent(id)}/reject`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ reason: "rejected from UI" }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: "Reject failed." }));
    window.alert(body.error || "Reject failed.");
    return;
  }
  await loadCatalog();
}

function paint() {
  renderMetrics();
  renderChips();
  renderQueue();
  renderArchive();
}

async function loadCatalog() {
  const response = await fetch("/api/catalog", { cache: "no-store" });
  if (!response.ok) throw new Error("Could not load the archive.");
  catalog = await response.json();
  paint();
}

document.querySelector("#hud-close")?.addEventListener("click", closeHud);
document.querySelector("#archive-hud")?.addEventListener("click", (event) => {
  if (event.target.id === "archive-hud") closeHud();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeHud();
});
["#archive-search", "#archive-sort"].forEach((sel) => {
  document.querySelector(sel)?.addEventListener("input", paint);
});

loadCatalog().catch((error) => {
  const mount = document.querySelector("#archive-grid");
  if (mount) mount.innerHTML = `<div class="empty-state">${esc(error.message)}</div>`;
});
setInterval(() => {
  loadCatalog().catch(() => {});
}, 4000);
