# Model supervisor protocol

Inspect the contract:

```powershell
cargo run --bin algo -- supervisor
```

Invoke a tool:

```powershell
cargo run --bin algo -- supervisor --request '{"protocol_version":"1.0","provider":"openai","model":"codex","tool":"list_agents","args":{}}'
```

Every invocation records provider, model, request, result, state, and timestamps in SQLite. Publication tools are rejected before dispatch. Supervisors may submit private relationship proposals, but only the human graph-review command can make an edge canonical.
