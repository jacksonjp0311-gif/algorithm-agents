# MCP server

Run `algo mcp` to expose the registered Alchetron tools over MCP stdio.

```json
{
  "mcpServers": {
    "alchetron": {
      "command": "algo",
      "args": ["--root", "/path/to/alchetron", "mcp"]
    }
  }
}
```

The server supports `initialize`, `ping`, `tools/list`, and `tools/call`. Its tools can extract, inspect, propose relationships, write private hypotheses, and create private research logs. Canonical acceptance, rollback, and human review tools are deliberately absent.
