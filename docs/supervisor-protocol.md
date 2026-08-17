# Model supervisor protocol

Inspect the contract:

```powershell
cargo run --bin algo -- supervisor
```

Invoke a tool:

```powershell
cargo run --bin algo -- supervisor --request '{"protocol_version":"2.0","provider":"openai","model":"codex","tool":"get_capabilities","args":{}}'
```

Protocol `2.0` is the current contract; `1.0` envelopes remain accepted for compatibility.

Every invocation records provider, model, request, result, state, and timestamps in SQLite. Publication tools are rejected before dispatch. Codex and other supervisors enter through this audited tool boundary. Direct OpenAI, xAI/Grok, and loopback model adapters are separately capability-gated. Supervisors may create private hypotheses or relationship proposals, but only an explicit human review operation can change canonical state.
