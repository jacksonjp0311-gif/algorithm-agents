# Operator guide

Windows entry point:

```powershell
.\scripts\run-alchetron.ps1 -Verify
```

The command verifies Rust and contracts, builds the runtime, starts the loopback server, prints a tokenized operator URL, and opens it. The token is moved into tab-scoped session storage and removed from the address bar.

Core commands:

```powershell
cargo run --bin algo -- harvest "shortest path" --limit 1
cargo run --bin algo -- --live scrape "https://example.org/paper"
cargo run --bin algo -- archive list
cargo run --bin algo -- archive history
cargo run --bin algo -- graph snapshot
cargo run --bin algo -- supervisor
```

Run the complete local verification:

```powershell
.\scripts\verify.ps1
```
