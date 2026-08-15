# Locked example — extract from the human record

Run on 2026-08-15 against this repository. Two paths. Both returned PENDING candidates. Nothing was auto-published.

## 1. Offline — bundled collective memory

```bash
cargo run --bin algo -- harvest "shortest path" --limit 1
```

Result:

| Field | Value |
| --- | --- |
| Locator | `fixture://shortest-path/bellman-ford.md` |
| Detect | **YES** — explicit computational procedure |
| Title | Bellman-Ford shortest path |
| Candidate | `CAND-0011` PENDING |
| Publication count | 0 |

This proves the crew works with no network.

## 2. Live — Wikipedia, public knowledge

```bash
cargo run --bin algo -- --live scrape "https://en.wikipedia.org/w/index.php?title=Dijkstra%27s_algorithm&action=raw"
```

Result:

| Field | Value |
| --- | --- |
| Locator | Wikipedia raw for Dijkstra's algorithm |
| Detect | **UNCERTAIN** — unstructured source, heuristic fields |
| Title | Dijkstra's algorithm |
| Candidate | `CAND-0012` PENDING |
| Publication count | 0 |

Uncertainty is correct. The page is not a labeled fixture. The operator still gets a card and a source URL.

## What “works” means

- A real locator is attached
- A detector verdict is recorded
- A candidate is queued, not published
- `algo archive list` and `algo ui` can show it

Repeat with `algo --live hunt "monte carlo"` to pull from arXiv and Wikipedia together.
