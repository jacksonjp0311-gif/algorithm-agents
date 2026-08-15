# Algorithm Agents

A **session-scoped, tool-driven multi-agent runtime** for discovering, extracting, reviewing, and saving algorithms.

Anyone can run it locally. Point it at bundled fixtures, files you drop in `sources/`, or a public URL. The agents scout, collect, detect, extract, check provenance, and write a **review candidate**. You decide what enters the archive.

```text
SUPERVISOR AUTHORITY ≠ ARCHIVE AUTHORITY
```

Agents may discover, collect, analyze, extract, compare, verify, and propose.
They may not silently promote a candidate. `algo archive accept` is the only path from queue → archive.

---

## What this is

Twenty-six specialist agents behind one CLI, `algo`.

| Family | Agents | Job |
| --- | --- | --- |
| Acquisition | source scout, web collector, paper analyst, repository scout, citation walker | Find and retrieve sources |
| Analysis | detector, extractor, math / code / complexity / assumption / failure analysts | Read a procedure out of a source |
| Verification | provenance, cross-source, math checker, code verifier, hallucination challenger | Challenge the extraction |
| Knowledge | deduplicator, relationship mapper, structural matcher, domain classifier, use-case mapper | Place it against what you already have |
| Synthesis | normalizer, summarizer, code translator, experiment designer | Shape a candidate you can keep |

The runtime — not the caller — enforces tools, budgets, host allow/deny lists, and denies. Failed, blocked, low-confidence, and needs-human states are first-class. Generated code is labeled as generated unless it came from the source.

Live public retrieval is **off by default**. Fixtures are the v1 source of truth and the offline path.

---

## Quick start

Requires [Rust 1.85.1+](https://rustup.rs/) (the repo pins the toolchain).

```powershell
cd $HOME\Desktop\algorithm-agents
cargo build --bin algo
```

Prove the contracts:

```powershell
cargo run --bin algo -- smoke
```

Find, extract, and save without the network:

```powershell
cargo run --bin algo -- find "shortest path"
cargo run --bin algo -- harvest "weighted shortest path"
cargo run --bin algo -- archive list
cargo run --bin algo -- archive accept CAND-0001
```

Scrape a public page (explicit opt-in):

```powershell
cargo run --bin algo -- --live scrape "https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm"
cargo run --bin algo -- archive show CAND-0002
```

Open the card UI (same archive cards as the Jackson site):

```powershell
cargo run --bin algo -- ui
```

That starts a local page at http://127.0.0.1:8791 — glowing cards, domain chips, search, and Accept / Reject on the queue. Nothing is public. Close the terminal to stop it.

A passing smoke run means all 26 agents compiled against their schemas, executed, and the failure path still fails.

---

## How a harvest works

```text
HUMAN OBJECTIVE
      │
      ▼
algo harvest "…"
      │
      ▼
source_scout        → ranked locators (fixtures + sources/ + optional URLs)
web_collector       → source text (fixture / file / live URL)
algorithm_detector  → YES / NO / UNCERTAIN
algorithm_extractor → normalized candidate
math + provenance   → independent checks
submit              → CAND-000n in data/archive/queue.json   (PENDING)
      │
      ▼
algo archive accept CAND-000n
      │
      ▼
data/archive/algorithms.json     local archive
data/archive/accepted/<id>.json  full extraction
```

Nothing in that pipeline publishes to a website or a remote API. The archive is files on disk.

---

## Commands

### Everyday

| Command | What it does |
| --- | --- |
| `algo find "<query>"` | Search fixtures, `sources/`, and accepted archive entries |
| `algo harvest "<objective>"` | Scout → extract → save up to `--limit` candidates (default 4) |
| `algo scrape <locator>` | Collect one fixture, file, or URL and save a candidate |
| `algo archive list` | Queue + accepted algorithms |
| `algo archive show <id>` | Full queued candidate |
| `algo archive accept <id>` | Promote into the local archive |
| `algo archive reject <id>` | Mark rejected |
| `algo ui` | Local card UI at http://127.0.0.1:8791 |

### Runtime

| Command | What it does |
| --- | --- |
| `algo smoke` | Compile every agent contract |
| `algo smoke --agent algorithm_extractor` | Smoke one agent |
| `algo demo shortest-path` | Scripted end-to-end extract (Dijkstra fixture) |
| `algo demo messages` | Inter-agent typed message bus |
| `algo demo escalation` | Ambiguity is preserved, not smoothed over |
| `algo demo harvest` | Multi-source harvest of fixtures |
| `algo agent list` / `algo agent describe <id>` | Registry |
| `algo session create --objective "…"` | Manual session |
| `algo run <agent> --session RS-0001 --artifact ART-0001` | Run one agent |
| `algo tool <name> --args '{…}'` | Call any registered tool |

Global flags:

```text
--root <dir>         Agent system root (registry, fixtures, sources)
--data-dir <dir>     SQLite + artifacts + archive   (default ./data)
--database <url>     sqlite://…                     (default <data-dir>/agent.db)
--live               Allow http(s) retrieval
```

Environment:

```text
ALG_AGENT_ROOT=…           override root discovery
ALGO_LIVE_FETCH=1          same as --live
ALGO_USER_AGENT=…          override the fetch User-Agent
```

---

## Locators

| Prefix | Meaning |
| --- | --- |
| `fixture://shortest-path/dijkstra.md` | Bundled labeled fixture |
| `sources://my-note.md` | File you dropped in `sources/` |
| `file://C:/papers/kalman.md` | Arbitrary local file |
| `https://…` | Public page, only with `--live` |

Private / loopback / link-local hosts are denied even when live fetch is on. An optional host allowlist lives in `registry/permissions.json`.

---

## Adding your own sources

Drop Markdown (or `.txt`) into `sources/`. Labeled fields extract cleanly. Unstructured pages still run — the parser infers title, abstract, code fences, and a conservative algorithm verdict.

A high-signal fixture looks like this:

```text
TITLE: Dijkstra's shortest path
AUTHORS:
- Edsger W. Dijkstra
YEAR: 1959
TYPE: paper
DOMAIN: Graph Algorithms
ABSTRACT: Single-source shortest paths on non-negative weighted graphs.
EQUATION:
d[v] = min(d[v], d[u] + w(u,v))
PSEUDOCODE:
u = extract_min(Q)
ALGORITHM: yes
```

See `fixtures/` for the full field set. `sources/README.md` is the operator cheat sheet.

---

## Where things are stored

```text
data/
  agent.db                 sessions, runs, messages, events
  artifacts/RS-…/ART-….json
  archive/
    queue.json             PENDING / REJECTED candidates
    algorithms.json        accepted catalog
    accepted/<id>.json     full extraction body
```

SQLite holds metadata. Artifact bodies are files. The archive is JSON you can diff, back up, or import.

---

## Trust model

- Agents are static contracts (`agents/<id>/agent.json` + input/output schemas), not free-form personalities.
- Role, capability, and authority are separate from any model provider.
- The runtime denies `publish_algorithm`, `approve_extraction`, unrestricted shell, and arbitrary filesystem writes.
- Code verification runs `python -I` on a submitted snippet with a timeout, or returns `NOT_TESTED`.
- Duplicate titles / core ideas against the accepted archive are blocked at submit time.
- Live fetch is opt-in, size-capped, time-capped, and SSRF-hardened.

This crate does **not** include a homepage, biography, payments, or a public site. It is the research runtime only.

---

## Architecture

```text
HUMAN DIRECTIVE
      │
      ▼
SupervisorAdapter     (CLI / scripted / later Grok, Codex, MCP)
      │  dispatch(tool, args)
      ▼
AgentRuntime
      ├── registry + permissions
      ├── sessions / runs / budgets
      ├── typed message bus
      ├── artifacts (SQLite metadata + files)
      ├── events + receipts
      └── family executors
              │
              ▼
        ArchiveHost.submit_extraction_candidate()
              │
              ▼
        LOCAL QUEUE   data/archive/queue.json
              │
              ▼
        HUMAN ACCEPT  algo archive accept
```

`SupervisorAdapter` is the only orchestration interface: `receive_directive`, `inspect_agents` / `inspect_tools`, `request_tool`, `stop`.

Grok, Codex, or an MCP host attach by calling the same tools in `registry/tools.json`. The runtime has no provider-specific branches. See `docs/supervisor-attach.md`.

---

## Adding an agent

1. Create `agents/<id>/` with `agent.json`, input/output schemas, smoke fixtures
2. Register the id in `registry/agents.json`
3. Implement one match arm in `src/agents/`
4. Run `algo smoke --agent <id>`
5. The agent is enabled only if smoke and schema validation pass

Do not edit the orchestrator to add an agent.

---

## Tests

```powershell
cargo test
cargo run --bin algo -- smoke
cargo run --bin algo -- demo shortest-path
```

`demo shortest-path` must report `"result": "PASS"` and `"publication_count": 0`.

---

## License

MIT. See `LICENSE`.
