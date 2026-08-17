const consoleState = {
  dashboard: { sessions: [], events: [], publication_receipts: [], relationship_proposals: [] },
  graph: { nodes: [], edges: [] },
  scale: 1,
  offsetX: 0,
  offsetY: 0,
  rotation: 0,
  rotating: false,
  selected: null,
  dragging: null,
  lastPointer: null,
};

const consoleToken = sessionStorage.getItem("alchetron_operator_token") || "";

function compactDate(value) {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString();
}

function renderDashboard() {
  const data = consoleState.dashboard;
  const sessions = data.sessions || [];
  const events = data.events || [];
  const receipts = data.publication_receipts || [];
  const proposals = data.relationship_proposals || [];
  const active = sessions.filter((item) => item.state === "ACTIVE").length;
  const runs = sessions.reduce((total, item) => total + Number(item.agent_runs_used || 0), 0);
  const mount = document.querySelector("#dashboard-metrics");
  if (mount) {
    mount.innerHTML = [
      ["Active sessions", active],
      ["Agent runs", runs],
      ["Canonical revisions", (data.revisions || []).length],
      ["Publication receipts", receipts.length],
      ["Pending relations", proposals.filter((item) => item.state === "PENDING").length],
    ]
      .map(([label, value]) => `<article class="dashboard-stat"><strong>${esc(value)}</strong><span>${esc(label)}</span></article>`)
      .join("");
  }
  const sessionMount = document.querySelector("#session-stream");
  if (sessionMount) {
    sessionMount.innerHTML = sessions.length
      ? sessions
          .slice(0, 12)
          .map(
            (session) => `<article class="ledger-row">
              <span class="status-dot state-${esc(String(session.state).toLowerCase())}"></span>
              <div><strong>${esc(session.session_id)}</strong><p>${esc(session.objective)}</p>
              <small>${esc(session.state)} · ${esc(session.agent_runs_used)} runs · ${esc(session.candidates_used)} candidates</small></div>
            </article>`,
          )
          .join("")
      : `<p class="empty-state">No sessions recorded.</p>`;
  }
  const eventMount = document.querySelector("#event-stream");
  if (eventMount) {
    eventMount.innerHTML = events.length
      ? events
          .slice(0, 30)
          .map(
            (event) => `<article class="event-row"><time>${esc(compactDate(event.created_at))}</time>
              <strong>${esc(event.kind)}</strong><span>${esc(event.session_id || "runtime")}</span></article>`,
          )
          .join("")
      : `<p class="empty-state">The event ledger is quiet.</p>`;
  }
  renderReceipts(receipts);
  renderRelationshipProposals(proposals);
}

function renderReceipts(receipts) {
  const mount = document.querySelector("#receipt-stream");
  if (!mount) return;
  mount.innerHTML = receipts.length
    ? receipts
        .map(
          (receipt) => `<article class="receipt-row">
            <p class="eyebrow">${esc(receipt.action)}</p>
            <strong>${esc(receipt.revision_id)}</strong>
            <p>${esc(receipt.reason)}</p>
            <small>${esc(receipt.reviewer)} · ${esc(compactDate(receipt.created_at))}</small>
            <code>${esc(String(receipt.canonical_hash || "").slice(0, 16))}</code>
            <button class="button button-ghost" type="button" data-rollback="${esc(receipt.revision_id)}">Rollback here</button>
          </article>`,
        )
        .join("")
    : `<p class="empty-state">No canonical publication receipts yet.</p>`;
  mount.querySelectorAll("[data-rollback]").forEach((button) => {
    button.addEventListener("click", () => rollbackRevision(button.dataset.rollback));
  });
}

function renderRelationshipProposals(proposals) {
  const mount = document.querySelector("#relationship-stream");
  if (!mount) return;
  mount.innerHTML = proposals.length
    ? proposals
        .map(
          (proposal) => `<article class="receipt-row">
            <p class="status-pill">${esc(proposal.state)}</p>
            <strong>${esc(proposal.relation)}</strong>
            <p>${esc(proposal.from_node)} → ${esc(proposal.to_node)}</p>
            <small>Proposed by ${esc(proposal.proposed_by)}</small>
            ${
              proposal.state === "PENDING"
                ? `<div class="button-row"><button class="button" data-relation-accept="${esc(proposal.proposal_id)}">Accept</button>
                   <button class="button button-ghost" data-relation-reject="${esc(proposal.proposal_id)}">Reject</button></div>`
                : `<p>${esc(proposal.review_reason || "")}</p>`
            }
          </article>`,
        )
        .join("")
    : `<p class="empty-state">No relationship proposals.</p>`;
  mount.querySelectorAll("[data-relation-accept]").forEach((button) => {
    button.addEventListener("click", () => reviewRelationship(button.dataset.relationAccept, true));
  });
  mount.querySelectorAll("[data-relation-reject]").forEach((button) => {
    button.addEventListener("click", () => reviewRelationship(button.dataset.relationReject, false));
  });
}

async function operatorPost(url, body) {
  if (!consoleToken) throw new Error("Open the tokenized operator URL printed by `algo ui`.");
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-alchetron-operator-token": consoleToken,
    },
    body: JSON.stringify(body),
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || "Operator request failed.");
  return data;
}

async function reviewRelationship(id, accept) {
  const confirmation = window.prompt(`Type ${id} to confirm.`);
  if (confirmation !== id) return;
  const reviewer = window.prompt("Reviewer", "human-operator") || "";
  const reason = window.prompt(accept ? "Why is this relation canonical?" : "Why reject this relation?") || "";
  try {
    await operatorPost(`/api/relationships/${encodeURIComponent(id)}/review`, {
      confirmation,
      reviewer,
      reason,
      accept,
    });
    await loadConsoleData();
  } catch (error) {
    window.alert(error.message);
  }
}

async function rollbackRevision(revisionId) {
  const confirmation = window.prompt(`Type ${revisionId} to confirm rollback.`);
  if (confirmation !== revisionId) return;
  const reviewer = window.prompt("Reviewer", "human-operator") || "";
  const reason = window.prompt("Why is rollback necessary?") || "";
  try {
    await operatorPost("/api/archive/rollback", {
      revision_id: revisionId,
      confirmation,
      reviewer,
      reason,
    });
    await Promise.all([loadCatalog(), loadConsoleData()]);
  } catch (error) {
    window.alert(error.message);
  }
}

function initializeGraph() {
  const nodes = consoleState.graph.nodes || [];
  const kinds = [...new Set(nodes.map((node) => node.kind))].sort();
  const relations = [...new Set((consoleState.graph.edges || []).map((edge) => edge.relation))].sort();
  const kindSelect = document.querySelector("#graph-kind");
  const relationSelect = document.querySelector("#graph-relation");
  if (kindSelect) {
    const previous = kindSelect.value;
    kindSelect.innerHTML = `<option value="">All knowledge</option>${kinds.map((kind) => `<option>${esc(kind)}</option>`).join("")}`;
    kindSelect.value = previous;
  }
  if (relationSelect) {
    const previous = relationSelect.value;
    relationSelect.innerHTML = `<option value="">All relationships</option>${relations
      .map((relation) => `<option>${esc(relation)}</option>`)
      .join("")}`;
    relationSelect.value = previous;
  }
  const radius = Math.max(150, nodes.length * 16);
  nodes.forEach((node, index) => {
    if (Number.isFinite(node.x)) return;
    const angle = (index / Math.max(nodes.length, 1)) * Math.PI * 2;
    node.x = Math.cos(angle) * radius;
    node.y = Math.sin(angle) * radius;
  });
  drawGraph();
}

function graphView() {
  const kind = document.querySelector("#graph-kind")?.value || "";
  const relation = document.querySelector("#graph-relation")?.value || "";
  const edges = (consoleState.graph.edges || []).filter((edge) => !relation || edge.relation === relation);
  const connected = new Set(edges.flatMap((edge) => [edge.from, edge.to]));
  const nodes = (consoleState.graph.nodes || []).filter(
    (node) => (!kind || node.kind === kind) && (!relation || connected.has(node.id)),
  );
  const visible = new Set(nodes.map((node) => node.id));
  return { nodes, edges: edges.filter((edge) => visible.has(edge.from) && visible.has(edge.to)) };
}

function drawGraph() {
  const canvas = document.querySelector("#knowledge-canvas");
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const ratio = window.devicePixelRatio || 1;
  if (canvas.width !== Math.floor(rect.width * ratio) || canvas.height !== Math.floor(rect.height * ratio)) {
    canvas.width = Math.floor(rect.width * ratio);
    canvas.height = Math.floor(rect.height * ratio);
  }
  const context = canvas.getContext("2d");
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, rect.width, rect.height);
  const centerX = rect.width / 2 + consoleState.offsetX;
  const centerY = rect.height / 2 + consoleState.offsetY;
  const transform = (node) => {
    const cos = Math.cos(consoleState.rotation);
    const sin = Math.sin(consoleState.rotation);
    return {
      x: centerX + (node.x * cos - node.y * sin) * consoleState.scale,
      y: centerY + (node.x * sin + node.y * cos) * consoleState.scale,
    };
  };
  const { nodes, edges } = graphView();
  const byId = new Map(nodes.map((node) => [node.id, node]));
  context.lineWidth = 1;
  for (const edge of edges) {
    const from = byId.get(edge.from);
    const to = byId.get(edge.to);
    if (!from || !to) continue;
    const a = transform(from);
    const b = transform(to);
    context.strokeStyle = "rgba(86, 224, 255, .22)";
    context.shadowColor = "rgba(86, 224, 255, .18)";
    context.shadowBlur = 7;
    context.beginPath();
    context.moveTo(a.x, a.y);
    context.lineTo(b.x, b.y);
    context.stroke();
  }
  for (const node of nodes) {
    const point = transform(node);
    const selected = consoleState.selected?.id === node.id;
    const color = node.kind === "ALGORITHM" ? "#7df9ff" : node.kind === "SOURCE" ? "#f379ff" : "#b9ffcf";
    context.fillStyle = color;
    context.shadowColor = color;
    context.shadowBlur = selected ? 24 : 12;
    context.beginPath();
    context.arc(point.x, point.y, selected ? 8 : 5, 0, Math.PI * 2);
    context.fill();
    if (selected || consoleState.scale > 1.15) {
      context.shadowBlur = 0;
      context.fillStyle = "rgba(232,247,255,.9)";
      context.font = "12px system-ui";
      context.fillText(node.label, point.x + 11, point.y + 4);
    }
    node.screenX = point.x;
    node.screenY = point.y;
  }
  const readout = document.querySelector("#graph-readout");
  if (readout) readout.textContent = `${nodes.length} nodes · ${edges.length} canonical relations · ${consoleState.scale.toFixed(2)}×`;
  if (consoleState.rotating) {
    consoleState.rotation += 0.0018;
    requestAnimationFrame(drawGraph);
  }
}

function bindGraph() {
  const canvas = document.querySelector("#knowledge-canvas");
  if (!canvas) return;
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    consoleState.scale = Math.min(3.5, Math.max(0.25, consoleState.scale * (event.deltaY > 0 ? 0.9 : 1.1)));
    drawGraph();
  }, { passive: false });
  canvas.addEventListener("pointerdown", (event) => {
    const node = graphView().nodes.find((item) => Math.hypot(item.screenX - event.offsetX, item.screenY - event.offsetY) < 14);
    consoleState.dragging = node || "field";
    consoleState.lastPointer = { x: event.clientX, y: event.clientY };
    canvas.setPointerCapture(event.pointerId);
    if (node) {
      consoleState.selected = node;
      renderGraphInspector(node);
      drawGraph();
    }
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!consoleState.dragging || !consoleState.lastPointer) return;
    const dx = event.clientX - consoleState.lastPointer.x;
    const dy = event.clientY - consoleState.lastPointer.y;
    if (consoleState.dragging === "field") {
      consoleState.offsetX += dx;
      consoleState.offsetY += dy;
    } else {
      consoleState.dragging.x += dx / consoleState.scale;
      consoleState.dragging.y += dy / consoleState.scale;
    }
    consoleState.lastPointer = { x: event.clientX, y: event.clientY };
    drawGraph();
  });
  const release = () => {
    consoleState.dragging = null;
    consoleState.lastPointer = null;
  };
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointercancel", release);
  document.querySelector("#graph-rotate")?.addEventListener("click", () => {
    consoleState.rotating = !consoleState.rotating;
    drawGraph();
  });
  document.querySelector("#graph-reset")?.addEventListener("click", () => {
    consoleState.scale = 1;
    consoleState.offsetX = 0;
    consoleState.offsetY = 0;
    consoleState.rotation = 0;
    initializeGraph();
  });
  ["#graph-kind", "#graph-relation"].forEach((selector) => {
    document.querySelector(selector)?.addEventListener("change", drawGraph);
  });
  window.addEventListener("resize", drawGraph);
}

function renderGraphInspector(node) {
  const mount = document.querySelector("#graph-inspector");
  if (!mount) return;
  mount.innerHTML = `<p class="eyebrow">${esc(node.kind)}</p><h3>${esc(node.label)}</h3>
    <p>${esc(node.id)}</p><pre>${esc(JSON.stringify(node.body || {}, null, 2))}</pre>
    <p class="meta-row">Canonical · provenance preserved</p>`;
}

async function loadConsoleData() {
  const [dashboardResponse, graphResponse] = await Promise.all([
    fetch("/api/dashboard", { cache: "no-store" }),
    fetch("/api/graph", { cache: "no-store" }),
  ]);
  if (!dashboardResponse.ok || !graphResponse.ok) throw new Error("Could not load Alchetron console data.");
  consoleState.dashboard = await dashboardResponse.json();
  consoleState.graph = await graphResponse.json();
  renderDashboard();
  initializeGraph();
}

document.querySelector("#operator-state").textContent = consoleToken
  ? "OPERATOR AUTHORITY · ACTIVE IN THIS TAB"
  : "READ-ONLY VIEW · OPEN THE TOKENIZED OPERATOR URL TO REVIEW";
document.querySelector("#refresh-dashboard")?.addEventListener("click", () => loadConsoleData().catch((error) => window.alert(error.message)));
bindGraph();
loadConsoleData().catch((error) => {
  const mount = document.querySelector("#dashboard-metrics");
  if (mount) mount.innerHTML = `<div class="empty-state">${esc(error.message)}</div>`;
});
setInterval(() => loadConsoleData().catch(() => {}), 10000);
