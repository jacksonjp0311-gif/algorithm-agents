<p align="center">
  <img src="docs/extracting-agents.jpg" alt="Specialist agents drawing procedures out of the human record and condensing them into archive cards" width="100%">
</p>

# Algorithm Agents

Humanity already wrote the algorithms. They are scattered across papers, notes, code, and encyclopedias. This runtime sends a crew of specialist agents into that record, pulls a reconstructable procedure out of a source, and saves it as a reviewable card.

You stay in charge. Agents find, extract, and propose. They do not publish.

```text
SUPERVISOR AUTHORITY ≠ ARCHIVE AUTHORITY
```

---

## Why this is powerful

Most tools summarize. This one **extracts**.

Give it a question like “shortest path” or a URL from arXiv or Wikipedia. Twenty-six bounded agents hunt sources, detect whether a real procedure is present, pull the math, assumptions, complexity, and pseudocode, then check provenance. What comes back is not a chat paragraph. It is a candidate you can keep.

That matters because the human collective of knowledge is huge and messy. Algorithms hide inside papers, side remarks, and unlabeled pages. A single model asked to “find algorithms” will invent, flatten, or forget. A crew with roles, budgets, and denies will stall, mark uncertainty, or refuse — and you still have the source.

Use it to:

- Build a private algorithm archive from public knowledge
- Turn a paper or wiki page into a structured card in one command
- Let another AI operate the same tool surface without giving it publication power

---

## How it works

```text
YOU  (or an AI operator)
        │
        ▼
   algo hunt / scrape / harvest
        │
        ▼
   26 specialist agents
   scout → collect → detect → extract
   math / provenance / normalize
        │
        ▼
   PENDING card in data/archive/queue.json
        │
        ▼
   you accept   →   local archive + card UI
```

Live web access is **off** until you pass `--live`. Private hosts are denied even then. Fixtures in `fixtures/` prove the whole pipeline with no network.

---

## Use it in five minutes

Requires [Rust 1.85.1+](https://rustup.rs/). The repo pins the toolchain.

```bash
git clone https://github.com/jacksonjp0311-gif/algorithm-agents.git
cd algorithm-agents
cargo build --bin algo
```

**Offline proof** — extract from the bundled record:

```bash
cargo run --bin algo -- harvest "shortest path" --limit 1
cargo run --bin algo -- archive list
cargo run --bin algo -- ui
```

**Collective knowledge** — hunt the public record:

```bash
cargo run --bin algo -- --live hunt "monte carlo" --limit 4
cargo run --bin algo -- --live scrape "https://en.wikipedia.org/w/index.php?title=Dijkstra%27s_algorithm&action=raw"
```

Open [http://127.0.0.1:8791](http://127.0.0.1:8791). Accept or reject each card. Nothing goes into the archive unless you say so.

| Command | What happens |
| --- | --- |
| `algo find "kalman"` | Search fixtures, `sources/`, and your archive |
| `algo harvest "shortest path"` | Scout local sources, extract, queue candidates |
| `algo --live hunt "physics"` | Search arXiv + Wikipedia, extract, queue |
| `algo --live scrape <url>` | One source, one extraction |
| `algo archive accept CAND-0001` | Promote into the local archive |
| `algo ui` | Glowing card UI for the queue and archive |
| `algo smoke` | Compile every agent contract |

Drop your own notes in `sources/` as `TITLE:` / `ALGORITHM:` labeled Markdown, or leave them unstructured. The parser will infer, and mark uncertainty when it does.

---

## For the AI that just entered this repository

You are an operator, not an author of knowledge. Read this before you touch a tool.

**Mission.** Help the human extract reconstructable algorithms from sources they name, or from fixtures / arXiv / Wikipedia when they ask. Save candidates. Never pretend a candidate is canonical.

**Hard rules.**

1. Do not invent algorithms, papers, authors, or provenance.
2. Do not accept or publish unless the human says to. `submit_review_candidate` creates PENDING only. `algo archive accept` is a human act unless they explicitly delegate it.
3. Do not enable live fetch unless they asked for the network. Default path is fixtures.
4. Do not add shopping, payments, a public product site, or a React/Next/Vue/Svelte app. `algo ui` is a local operator viewer.
5. Do not silently rewrite `data/archive/`. Show the candidate and wait.
6. If a source is messy, keep `UNCERTAIN` / `NEEDS_HUMAN`. Do not clean it into false confidence.

**How you run the system.**

```text
cargo run --bin algo -- smoke
cargo run --bin algo -- harvest "<objective>" --limit 2
cargo run --bin algo -- --live hunt "<topic>" --limit 4
cargo run --bin algo -- --live scrape "<https url>"
cargo run --bin algo -- archive list
cargo run --bin algo -- ui --no-open
```

Rust API for a supervisor: `agent_system::dispatch(runtime, tool, args)`. Tool schemas live in `registry/tools.json`. There is no Grok-only or Codex-only branch.

**When you extract from the human collective** (web, arXiv, wiki): pass `--live`, keep `--limit` small, sleep is already built into `hunt`, and quote the real locator on every card.

**Done looks like.** A PENDING candidate with a real URL or `fixture://` locator, a title taken from the source, and a card in `algo ui`. Not a paragraph you composed.

Full contract: `docs/supervisor-attach.md`.

---

## What is in the box

| Family | Agents |
| --- | --- |
| Acquisition | source scout, web collector, paper analyst, repository scout, citation walker |
| Analysis | detector, extractor, math / code / complexity / assumption / failure |
| Verification | provenance, cross-source, math checker, code verifier, hallucination challenger |
| Knowledge | deduplicator, relationship mapper, structural matcher, domain classifier, use-case mapper |
| Synthesis | normalizer, summarizer, code translator, experiment designer |

Verified walkthrough: [`examples/collective-knowledge.md`](examples/collective-knowledge.md).

---

## License

MIT. See `LICENSE`.
