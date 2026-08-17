# Alchetron architecture

Alchetron turns source material into private, evidence-bearing algorithm candidates. Its trust boundary is structural:

```text
SOURCE
  → SESSION-SCOPED AGENTS
  → HASHED ARTIFACTS + EVENTS + MESSAGES
  → VERIFICATION GATES
  → PRIVATE CANDIDATE
  → HUMAN REVIEW
  → IMMUTABLE CANONICAL REVISION
  → KNOWLEDGE GRAPH
```

SQLite is the operational ledger. JSON artifacts preserve agent outputs. The private queue is working state. `data/archive/CURRENT.json` points to the authoritative immutable canonical revision. Compatibility files in `data/archive/algorithms.json` and `accepted/` are exports.

Model supervisors enter through the versioned supervisor envelope. They can inspect, run agents, and propose private candidates or relationships. They cannot call acceptance, rollback, or canonical mutation tools.

Canonical relationships require two already-canonical endpoints and a separate human decision.
