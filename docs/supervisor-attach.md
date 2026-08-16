# Supervisor attach contract

Grok, Codex, and any later provider use the same tool surface.

```text
list_agents → start_session → run_agent / alias tools → inspect artifacts/events → submit_review_candidate
```

Everyday operators can skip the raw tools and use:

```text
algo do "…"
algo find "…"
algo harvest "…"
algo scrape <locator>
algo archive accept CAND-0001
```

The shared standard for human and AI is `advanced/`.

Those commands still go through `dispatch`. They do not bypass budgets or denies.

Surfaces:

- Rust: `agent_system::dispatch(runtime, tool, args)`
- CLI: `algo tool <name> --args '{...}'`
- Future HTTP / MCP: wrap `dispatch`

`registry/tools.json` is the schema source. The runtime has no Grok-only or Codex-only branches.

Publication never happens on this path. `submit_review_candidate` creates a PENDING extraction only. `algo archive accept` is an operator action, not an agent tool.
