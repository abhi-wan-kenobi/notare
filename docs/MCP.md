# Model Context Protocol (MCP) Server

Notare includes an embedded, read-only Model Context Protocol (MCP) server that exposes your local meeting notes, summaries, transcripts, recipes, and daily notes to external AI assistants such as Claude Desktop, Cursor, and CLI agent tools.

## Architecture

1. **In-App Streamable-HTTP Server**:
   - Runs directly inside the Notare desktop application when enabled.
   - Listens on `127.0.0.1:<port>` (default: `8787`, configurable in Settings).
   - Enforces Bearer token authentication on all endpoints:
     `Authorization: Bearer <token>`
   - Binds strictly to loopback (`127.0.0.1`); never accessible from external networks.
   - The per-install bearer token is generated once and stored securely in your OS keychain via `plugins/store2` secrets. It can be viewed, copied, and regenerated from the settings tab.

2. **Stdio Bridge (`notare-mcp`)**:
   - Clients such as Claude Desktop only support stdio-based MCP servers (`command` + `args`).
   - Notare provides a dedicated lightweight sidecar binary, `notare-mcp` (`apps/mcp-stdio/`), which reads JSON-RPC requests from standard input, proxies them to the desktop HTTP server with the configured bearer token, and prints responses to standard output.
   - In addition, the desktop application supports `--mcp-stdio --port <port> --token <token>` as a zero-packaging fallback. The standalone `notare-mcp` console binary is preferred for Tauri packaging and stdio stability across Linux, macOS, and Windows.

## Available Tools (Read-Only)

All tools are read-only and idempotent. No writes to the SQLite database occur via MCP.

| Tool Name | Parameters | Description |
|---|---|---|
| `list_meetings` | `query` (optional string), `limit` (optional int, default 20), `offset` (optional int) | List recent Notare meetings with pagination metadata. Filter by title or ID. |
| `get_meeting` | `id` (string) | Retrieve one meeting with its canonical note, summaries, participants, and action items. |
| `get_meeting_transcript` | `id` (string), `offset` (optional int), `limit` (optional int, default 200) | Bounded page of transcript words and readable text for a meeting. |
| `search_meetings` | `query` (string), `limit` (optional int), `offset` (optional int) | Full-text search across meetings using the on-device Tantivy search index. |
| `search_meeting_content` | `query` (string), `meeting_ids` (optional array of strings), `limit` (optional int) | Lexical content search across meeting notes and transcripts for exact phrases and keywords. *(Note: plugins/embedding-search only exposes KNN semantic search without a hybrid reciprocal-rank fusion Rust API, so content search runs lexically).* |
| `list_recipes` | *(none)* | List all available AI prompt recipes in Notare. |
| `get_daily_note` | `date` (string, `YYYY-MM-DD`) | Retrieve the daily note for a specific calendar date. |

## Client Configuration

### Claude Desktop

Edit your Claude Desktop configuration file:
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

Add the `notare` server under `mcpServers`:

```json
{
  "mcpServers": {
    "notare": {
      "command": "notare-mcp",
      "args": [
        "--port",
        "8787",
        "--token",
        "YOUR_BEARER_TOKEN"
      ]
    }
  }
}
```

*Alternatively, using the desktop binary with `--mcp-stdio`:*
```json
{
  "mcpServers": {
    "notare": {
      "command": "/Applications/Notare.app/Contents/MacOS/notare",
      "args": [
        "--mcp-stdio",
        "--port",
        "8787",
        "--token",
        "YOUR_BEARER_TOKEN"
      ]
    }
  }
}
```

### Cursor

Edit `~/.cursor/mcp.json` or your workspace `.cursor/mcp.json`.

**Option A: HTTP Streamable Transport (Recommended for Cursor)**:
```json
{
  "mcpServers": {
    "notare": {
      "url": "http://127.0.0.1:8787",
      "headers": {
        "Authorization": "Bearer YOUR_BEARER_TOKEN"
      }
    }
  }
}
```

**Option B: Stdio Transport**:
```json
{
  "mcpServers": {
    "notare": {
      "command": "notare-mcp",
      "args": [
        "--port",
        "8787",
        "--token",
        "YOUR_BEARER_TOKEN"
      ]
    }
  }
}
```

## Security & Privacy

- **Loopback only**: The HTTP server only accepts connections on `127.0.0.1`.
- **Bearer Token**: Every HTTP request must supply a valid `Authorization: Bearer <token>` header. Missing or invalid tokens immediately receive `401 Unauthorized`.
- **Read-Only**: None of the tools modify, delete, or create records in SQLite.
- **Port In Use Detection**: If the configured port is already occupied by another application, Notare surfaces an error in Settings without crashing.
