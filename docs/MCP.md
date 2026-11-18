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
   - Clients such as Claude Desktop communicate via standard input/output (`stdio`).
   - Notare includes `notare-mcp` bundled directly inside the application package as a Tauri sidecar:
     - **macOS:** `/Applications/Notare.app/Contents/MacOS/notare-mcp`
     - **Windows:** `C:\Program Files\Notare\notare-mcp.exe`
     - **Linux:** `/usr/bin/notare-mcp` (or next to `notare` inside the AppImage)
   - Reads JSON-RPC requests from standard input, proxies them to the desktop HTTP server with the configured bearer token, and prints responses to standard output.

## Available Tools (Read-Only)

All tools are read-only and idempotent. No writes to the SQLite database occur via MCP.

| Tool Name | Parameters | Description |
|---|---|---|
| `list_meetings` | `query` (optional string), `limit` (optional int, default 20), `offset` (optional int) | List recent Notare meetings with pagination metadata. Filter by title or ID. |
| `get_meeting` | `id` (string) | Retrieve one meeting with its canonical note, summaries, participants, and action items. |
| `get_meeting_transcript` | `id` (string), `offset` (optional int), `limit` (optional int, default 200) | Bounded page of transcript words and readable text for a meeting. |
| `search_meetings` | `query` (string), `limit` (optional int), `offset` (optional int) | Full-text search across meetings using the on-device Tantivy search index. |
| `search_meeting_content` | `query` (string), `meeting_ids` (optional array of strings), `limit` (optional int, default 20) | Hybrid content search across meeting notes and transcripts: fuses lexical (SQL `LIKE`) and on-device semantic (embedding KNN) matches with Reciprocal Rank Fusion (k=60), grouped by meeting and ordered by fused score. Falls back to lexical-only — reported in the result's `mode` field — when the semantic model isn't downloaded or loaded yet; never errors solely because semantic search is unavailable. Each match carries a `[[src:<sessionId>@<ms>]]` citation id, matching the format used in chat. |
| `list_recipes` | *(none)* | List all available AI prompt recipes in Notare. |
| `get_daily_note` | `date` (string, `YYYY-MM-DD`) | Retrieve the daily note for a specific calendar date. |

## Client Configuration

### HTTP Clients (omp, Cursor, Claude Code) — Default

Clients that speak streamable HTTP connect directly to Notare's loopback server without requiring a bridge binary.

**Standard HTTP configuration snippet** (used by omp, Claude Code, and HTTP MCP runners):
```json
{
  "notare": {
    "url": "http://127.0.0.1:8787",
    "headers": {
      "Authorization": "Bearer YOUR_BEARER_TOKEN"
    }
  }
}
```

**Cursor (`~/.cursor/mcp.json` or workspace `.cursor/mcp.json`)**:
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

### Claude Desktop

Claude Desktop requires a stdio-based transport. Configure it with the bundled `notare-mcp` binary shipped inside Notare.

Edit your Claude Desktop configuration file:
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

Add the `notare` server under `mcpServers`:

```json
{
  "mcpServers": {
    "notare": {
      "command": "/Applications/Notare.app/Contents/MacOS/notare-mcp",
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

*(Replace `/Applications/Notare.app/Contents/MacOS/notare-mcp` with `C:\Program Files\Notare\notare-mcp.exe` on Windows or `/usr/bin/notare-mcp` on Linux. In Notare, **Settings → MCP Server** provides a pre-filled snippet that automatically inserts the absolute path for your platform).*

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
## Security & Privacy

- **Loopback only**: The HTTP server only accepts connections on `127.0.0.1`.
- **Bearer Token**: Every HTTP request must supply a valid `Authorization: Bearer <token>` header. Missing or invalid tokens immediately receive `401 Unauthorized`.
- **Read-Only**: None of the tools modify, delete, or create records in SQLite.
- **Port In Use Detection**: If the configured port is already occupied by another application, Notare surfaces an error in Settings without crashing.
