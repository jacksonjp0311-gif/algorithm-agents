# Commands

The AI does not “use the agents.” The AI **runs commands**. The human may run the same ones. That is the interconnection: one language.

Print the card any time:

```bash
cargo run --bin algo -- cmds
```

## Preferred door

```bash
cargo run --bin algo -- do "<the human's sentence>"
```

| The human says | What `do` becomes |
| --- | --- |
| a URL, or “scrape this https://…” | live `scrape` |
| “on arxiv”, “wikipedia”, “search the web”, “papers” | live `hunt` |
| “from fixtures”, “extract shortest path” | offline `harvest` |
| “find kalman in the archive” | `find` |
| “smoke” / “check agents” | `smoke` |

Direct CMDs when you already know the verb:

```bash
cargo run --bin algo -- smoke
cargo run --bin algo -- smoke --process
cargo run --bin algo -- harvest "shortest path" --limit 2
cargo run --bin algo -- --live hunt "monte carlo" --limit 4
cargo run --bin algo -- --live scrape "https://arxiv.org/abs/physics/0306182"
cargo run --bin algo -- --live scrape "https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm"
cargo run --bin algo -- find "kalman"
cargo run --bin algo -- archive list
cargo run --bin algo -- archive show CAND-0001
cargo run --bin algo -- ui --no-open
```

Accept is not in the AI default mouth.

```bash
cargo run --bin algo -- archive accept CAND-0001
```

Run that only when the human has looked, or has said you may accept that id.

## What you report back

Not a lecture. A table:

- action (`scrape` / `hunt` / `harvest`)
- locator
- title from the source
- detect verdict
- candidate id
- PENDING

Then stop. The human opens `algo ui` or reads `archive list`.

## Raw tool surface

If you are attaching as a supervisor instead of a shell:

```text
list_agents → start_session → run_agent / alias tools → submit_review_candidate
```

`agent_system::dispatch(runtime, tool, args)`  
`algo tool <name> --args '{...}'`  
Schemas: `registry/tools.json`

There is no Grok branch. There is no Codex branch. Same wire.
