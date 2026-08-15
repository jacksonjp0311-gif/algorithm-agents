# Algorithm Agents — instructions for the AI operator

You entered a standalone extraction runtime. You are here to run agents that pull algorithms out of sources. You are not here to invent a field or publish a canon.

## Mission

Extract reconstructable computational procedures from fixtures, files the human dropped in `sources/`, or live public pages they asked for. Queue them. Let the human accept.

## Hard rules

- Do not fabricate algorithms, papers, authors, or provenance.
- Do not accept/publish unless the human explicitly asks.
- Do not turn live fetch on unless they asked for the network.
- Do not add a public product site, shopping, payments, or React/Next/Vue/Svelte.
- Do not silently edit `data/archive/`.
- Keep SUPERVISOR AUTHORITY separate from ARCHIVE AUTHORITY.
- Preserve UNCERTAIN and NEEDS_HUMAN. Do not smooth them away.

## Commands you actually run

```bash
cargo run --bin algo -- smoke
cargo run --bin algo -- harvest "<objective>" --limit 2
cargo run --bin algo -- --live hunt "<topic>" --limit 4
cargo run --bin algo -- --live scrape "<url>"
cargo run --bin algo -- archive list
cargo run --bin algo -- ui --no-open
```

## Collective-knowledge path

When the human says extract from the public record:

1. Prefer `algo --live hunt "<topic>"` for arXiv + Wikipedia.
2. Prefer `algo --live scrape "<url>"` when they name a page.
3. Quote the real locator on every candidate.
4. Stop at PENDING. Show the queue.

## Local-only path

`algo harvest` and `algo find` use `fixtures/` and `sources/` with no network.

## Do not

- Dump extractions into some other website's Research or Algorithms page unless the human names that destination.
- Claim a survey, chrome-filled HTML page, or unlabeled note is a clean algorithm. Mark it uncertain.
- Expand scope into biography, payments, or a homepage.

See `README.md` (human + AI) and `docs/supervisor-attach.md` (tool attach).
