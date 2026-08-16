# Circuit

This repository is a loop, not a pile of files. Human and AI stand at different points on the same wire.

```text
                    ┌──────────── intent ────────────┐
                    ▼                                │
              README / AGENTS.md                     │
                    │                                │
                    ▼                                │
              algo do "…"   /   algo scrape          │
                    │                                │
                    ▼                                │
         registry/agents.json   permissions.json     │
                    │                                │
                    ▼                                │
         session (RS-…)   runs (RUN-…)               │
                    │                                │
         scout → collect → detect → extract          │
              math / provenance / normalize          │
                    │                                │
                    ▼                                │
         artifacts/RS-…/ART-….json                   │
                    │                                │
                    ▼                                │
         data/archive/queue.json     PENDING         │
                    │                                │
                    ▼                                │
         algo ui   ←——  human looks  ——→  accept     │
                    │                                │
                    ▼                                │
         data/archive/algorithms.json                │
         data/archive/accepted/<id>.json             │
                    │                                │
                    └──────── find / harvest ────────┘
```

## Where each of you actually is

| Place in the repo | Human feels | AI feels |
| --- | --- | --- |
| `README.md` | Why the gift was given | Why you may not invent |
| `AGENTS.md` | How to hand the tool to a model | Your standing orders |
| `advanced/` | The standard | The standard |
| `registry/` | The crew list | The only agents that exist |
| `fixtures/` | Proof the crew is honest offline | The first place you extract |
| `sources/` | Notes the human dropped | More local record, not yours to invent |
| `data/archive/` | The ledger | Read and report. Do not rewrite. |
| `web/` | The card table | Serve it. Do not restyle it into a product. |
| `examples/` | What “works” already meant | The bar you must meet again |

## How a crossing feels

When the human says *get this from arXiv*, the AI does not write a summary. It runs:

```bash
cargo run --bin algo -- do "scrape https://arxiv.org/abs/…"
```

The collector rewrites the abs page to the arXiv API. The detector sees a procedure or it does not. A card appears with the paper's title. Both of you can look at the same `CAND-…` in `algo ui`. That is the interconnection: **one object, two kinds of attention.**

When the source is a fixture, there is no weather. When the source is the public record, there is weather — rate limits, messy pages, UNCERTAIN. The circuit does not hide the weather.

## What must not short

- AI → archive, skipping the queue
- Human → “just make something up so the page isn't empty”
- Agent → accept
- Live fetch → localhost, or any host the permissions file denies

The runtime will refuse some of these. The covenant refuses the rest.
