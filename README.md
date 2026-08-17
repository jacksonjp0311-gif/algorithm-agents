<p align="center">
  <img src="docs/extracting-agents.jpg" alt="Specialist agents drawing procedures out of the human record and condensing them into archive cards" width="100%">
</p>

# Alchetron

**Alchemy × cybernetics for governed computational knowledge.** The binary remains `algo`; the system is Alchetron.

Version 2.0 turns every extraction into an evidence package: immutable source snapshot, claim-level spans, specialist verification, private candidate, human decision, versioned canon, and a typed knowledge lattice.

This is not a chatbot with extra steps.

It is a **shared instrument**. A human names an intent. An AI may enter this repository and run the crew as commands. Twenty-six specialist agents walk the human record — papers, notes, code, encyclopedias — and try to lift out a reconstructable procedure. What they return is a **candidate**, not a canon.

You decide what is kept.

```text
THE HUMAN SPEAKS THE INTENT
THE AI RUNS THE COMMANDS
THE AGENTS TOUCH ONLY THE SOURCE
THE ARCHIVE MOVES ONLY WHEN A HUMAN ACCEPTS
```

```text
SUPERVISOR AUTHORITY ≠ ARCHIVE AUTHORITY
```

That split is the whole ethic. Break it and this is just another model writing fiction into a folder. Keep it and you have something rarer: a way for people and machines to tend the same knowledge without lying about where it came from.

---

## Why this exists

Humanity already wrote the algorithms. They are buried. A single model asked to “find algorithms” will invent, flatten, or forget. This runtime does not ask one mind to be a library. It sends a **crew** with roles, budgets, and the right to fail.

- **Scout** finds sources.
- **Collector** retrieves them.
- **Detector** says whether a procedure is actually there.
- **Extractor** pulls math, assumptions, complexity, pseudocode.
- **Verifiers** challenge provenance and hallucination.
- **Nothing publishes.**

What comes back is a card you can open, reject, or accept. The locator stays attached. Uncertainty is kept, not polished away.

Give this to someone you trust. Give it to an AI you do not fully trust. The runtime, not the speaker, enforces the denies.

---

## How a human and an AI share the room

```text
HUMAN                         AI OPERATOR                      CREW
  │                               │                              │
  │  "get monte carlo from arxiv" │                              │
  │──────────────────────────────►│                              │
  │                               │  algo do "…"                 │
  │                               │─────────────────────────────►│
  │                               │                              │  scout → collect
  │                               │                              │  detect → extract
  │                               │  PENDING card + locator      │
  │  algo ui  /  accept or reject │◄─────────────────────────────│
  │◄──────────────────────────────│                              │
  │  the archive moves, or not    │                              │
```

The human is the conscience.  
The AI is the hands.  
The agents are the instruments.  
The archive is the ledger.

If you are human: speak plainly, then look at the cards.  
If you are an AI: read [`AGENTS.md`](AGENTS.md), then [`advanced/`](advanced/README.md). Do not improvise a canon.

---

## Enter in five minutes

Requires [Rust 1.88+](https://rustup.rs/). The toolchain is pinned to 1.88.0.

```bash
git clone https://github.com/jacksonjp0311-gif/algorithm-agents.git
cd algorithm-agents
cargo build --bin algo
```

On Windows, verify and enter the operator console:

```powershell
.\scripts\run-alchetron.ps1 -Verify
```

Or prove the crew without the network:

```bash
cargo run --bin algo -- smoke --process
cargo run --bin algo -- harvest "shortest path" --limit 1
cargo run --bin algo -- ui
```

Speak intent — human sentence or AI command, same door:

```bash
cargo run --bin algo -- do "scrape https://arxiv.org/abs/physics/0306182"
cargo run --bin algo -- do "find monte carlo algorithms on arxiv"
cargo run --bin algo -- cmds
```

Open [http://127.0.0.1:8791](http://127.0.0.1:8791). Accept or reject. Nothing enters the archive unless you say so.

Live retrieval is **off** until `--live` or an intent that clearly asks for the public record. Private hosts stay denied.

| Command | What it does |
| --- | --- |
| `algo do "<intent>"` | Front door. Turns a sentence into scrape, hunt, or harvest |
| `algo cmds` | The command card every AI should read |
| `algo --live scrape <url>` | One source, one extraction |
| `algo --live hunt "physics"` | arXiv + Wikipedia, then extract |
| `algo harvest "shortest path"` | Local fixtures and `sources/` |
| `algo find "kalman"` | Search fixtures, notes, archive |
| `algo archive accept CAND-0001 --reason "evidence reviewed"` | Human promotion into an immutable revision |
| `algo archive history` | Canonical revisions and rollback targets |
| `algo graph snapshot` | Canonical knowledge graph |
| `algo supervisor` | Provider-neutral model entry contract |
| `algo mcp` | MCP stdio server for capable models |
| `algo model capabilities` | Direct OpenAI, Grok, local, and Codex entry surfaces |
| `algo eval run` | Measured extraction regression corpus |
| `algo emergent scan` | Private missing-link hypotheses; never auto-canonical |
| `algo doctor` | Archive, graph, runtime, and registry integrity |
| `algo ui` | Secured operator dashboard and interactive Nexus |
| `algo smoke --process` | Compile every agent and the harvest path |

Drop notes in `sources/`. Labeled fields extract cleanly. Unstructured text is inferred and marked uncertain.

Locked runs: [`examples/collective-knowledge.md`](examples/collective-knowledge.md).

---

## The crew

| Family | Who walks |
| --- | --- |
| Acquisition | source scout, web collector, paper analyst, repository scout, citation walker |
| Analysis | detector, extractor, math / code / complexity / assumption / failure |
| Verification | provenance, cross-source, math checker, code verifier, hallucination challenger |
| Knowledge | deduplicator, relationship mapper, structural matcher, domain classifier, use-case mapper |
| Synthesis | normalizer, summarizer, code translator, experiment designer |

Names, duties, and how they speak to each other live in [`advanced/ROSTER.md`](advanced/ROSTER.md).

---

## If you were given this tool

Treat the archive as someone else's memory that you are allowed to tend.

Do not invent a paper to fill a hole.  
Do not accept a card to look productive.  
Do not strip a warning so the page looks finished.

The work is the extraction **and** the refusal.

Deeper room — covenant, circuit, commands, how to extend without breaking the split:

**[`advanced/`](advanced/README.md)**

Operational references:

- [`docs/architecture.md`](docs/architecture.md)
- [`docs/security.md`](docs/security.md)
- [`docs/operator.md`](docs/operator.md)
- [`docs/archive-recovery.md`](docs/archive-recovery.md)
- [`docs/supervisor-protocol.md`](docs/supervisor-protocol.md)
- [`docs/api.md`](docs/api.md)
- [`docs/mcp.md`](docs/mcp.md)
- [`docs/evaluation.md`](docs/evaluation.md)
- [`docs/emergent.md`](docs/emergent.md)
- [`docs/distribution.md`](docs/distribution.md)

---

## License

MIT. See `LICENSE`.
