# Browser Harness MCP Server

`browser-harness-mcp` exposes the existing Rust CLI commands as MCP tools over
stdio. It reuses `browser-harness`, so the daemon, typed requests, browser state,
and security boundaries stay in the current Rust architecture.

## Start

```bash
browser-harness-mcp
```

Set `BROWSER_HARNESS_BIN` when the facade binary has a custom path. The server
supports MCP `initialize`, `ping`, `tools/list`, and `tools/call`. Tool failures
return MCP tool errors while the stdio server remains available.

Example client configuration:

```json
{
  "mcpServers": {
    "browser-harness": {
      "command": "browser-harness-mcp"
    }
  }
}
```
