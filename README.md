# GleanMark

Your personal knowledge base with semantic search and AI integration.

GleanMark lets you save web articles as bookmarks with their content, then find them later by *meaning* — not just keywords. Ask "what did I save about leadership challenges" and get the right articles, even if those exact words never appeared. Everything runs locally on your machine, your data stays yours, and AI agents can tap into your knowledge base directly.

## Why GleanMark

**The problem**: You read dozens of articles every week. Some are worth revisiting — for research, writing, or reference. But when you need them months later, keyword search fails because you don't remember the exact words. And your bookmarks are just a graveyard of URLs with no context.

**What GleanMark does differently**:

- **Saves content, not just URLs** — When you bookmark an article, GleanMark captures the text so you can search against it later, even if the original page goes offline
- **Finds by meaning** — Hybrid search combines semantic understanding (vector embeddings) with keyword matching (BM25), so searching "team motivation strategies" finds your Harvard Business Review article about employee morale
- **Works with AI agents** — Expose your knowledge base to Claude via MCP. Ask "what articles did I save about X?" and your AI assistant searches your personal library directly
- **Runs 100% locally** — No cloud accounts, no API keys, no subscription fees. Embeddings generated on-device, data stored on your machine
- **Grows more valuable over time** — As you accumulate hundreds of bookmarks, GleanMark becomes a personal corpus that reflects your interests and research, ready to be queried, referenced, and cited

## Features

- **Hybrid Search** — Semantic (vector) + keyword (BM25) combined via Reciprocal Rank Fusion
- **Multilingual** — MultilingualE5 embeddings + jieba Chinese word segmentation
- **Desktop App** — Tauri-based app with system tray, global hotkey (Cmd+Shift+G), and auto-updater
- **Web UI** — Search, browse, and manage bookmarks at `http://localhost:21580`
- **Browser Extension** — Chrome extension (Manifest V3) for one-click bookmark saving
- **CLI** — Terminal interface for power users
- **MCP Server** — AI agent integration for Claude Code / Claude Desktop
- **Dark Mode** — Light / dark / system theme support
- **Local Embeddings** — fastembed-rs runs on-device, no API keys needed
- **Import / Export** — JSON-based data portability

## Quick Start

### Desktop App (Recommended)

Download the latest release from [GitHub Releases](https://github.com/jiayun/gleanmark/releases):

- **macOS**: `.dmg` (Apple Silicon)
- **Windows**: `.msi` installer
- **Linux**: `.deb` or `.AppImage`

> **macOS**: If you see a security warning, run `xattr -cr /Applications/GleanMark.app` (the app is not yet notarized by Apple).

The app auto-manages Qdrant and embedding models. First launch may take a moment to download models (~100MB, one-time only).

### CLI

```bash
cargo install --path crates/gleanmark-cli

# Add a bookmark
gleanmark add https://example.com/article --title "Great Article" --tags "rust,programming"

# Search
gleanmark search "async programming patterns"

# List all bookmarks
gleanmark list

# Start the HTTP server + Web UI
gleanmark serve
```

### Browser Extension

1. Open Chrome → `chrome://extensions/` → Enable Developer Mode
2. Click "Load unpacked" → select the `extension/` folder
3. Start the server: `gleanmark serve` (or use the Desktop App)
4. Click the GleanMark icon on any page to save it

### MCP Server (AI Agents)

```bash
cargo build -p gleanmark-mcp --release
```

**Claude Code** (`.claude/settings.json`):
```json
{
  "mcpServers": {
    "gleanmark": {
      "command": "/path/to/gleanmark-mcp"
    }
  }
}
```

**Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "gleanmark": {
      "command": "/path/to/gleanmark-mcp"
    }
  }
}
```

## Architecture

```
gleanmark/
├── gleanmark-core       Core library: embeddings, storage, search
├── gleanmark-server     HTTP API + Web UI (Axum, port 21580)
├── gleanmark-cli        CLI tool (clap)
├── gleanmark-tauri      Desktop app (Tauri v2)
├── gleanmark-mcp        MCP server for AI agents (rmcp)
└── extension/           Chrome extension (Manifest V3)
```

All components share `gleanmark-core`. Data stored in `~/Library/Application Support/gleanmark/` (macOS) or `~/.local/share/gleanmark/` (Linux).

## API Reference

The HTTP server runs on `http://127.0.0.1:21580`.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/bookmarks` | Create bookmark |
| GET | `/api/bookmarks?limit=50&offset=<id>` | List bookmarks (paginated) |
| DELETE | `/api/bookmarks/{id}` | Delete bookmark |
| POST | `/api/search` | Search bookmarks |
| POST | `/api/export` | Export to JSON file |
| POST | `/api/import` | Import from JSON file |
| POST | `/api/open` | Open URL in system browser |

### Search Request

```json
{
  "query": "async programming",
  "limit": 10,
  "tags": ["rust"]
}
```

### Search Response

```json
[
  {
    "bookmark": {
      "id": "uuid",
      "url": "https://...",
      "title": "...",
      "content": "...",
      "tags": ["rust"],
      "created_at": "2026-04-02T...",
      "updated_at": "2026-04-02T..."
    },
    "score": 0.85
  }
]
```

## MCP Tools

| Tool | Description | Key Parameters |
|------|-------------|----------------|
| `search_bookmarks` | Semantic + keyword search | `query`, `limit?` (default 10), `tags?` |
| `list_bookmarks` | List all bookmarks | `limit?` (default 20), `offset?` |
| `add_bookmark` | Save a new bookmark | `url`, `title`, `content?`, `tags?` |
| `delete_bookmark` | Delete by ID | `id` |

## Development

### Prerequisites

- Rust 1.85+ (edition 2024)
- [Qdrant](https://github.com/qdrant/qdrant/releases) binary in PATH or `~/.local/share/gleanmark/bin/`
- For desktop app: [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)

### Build

```bash
git clone https://github.com/jiayun/gleanmark.git
cd gleanmark

# Build all
cargo build

# Build specific crate
cargo build -p gleanmark-server

# Desktop app (dev mode)
cd crates/gleanmark-tauri
cargo tauri dev

# Run tests
cargo test

# Re-index bookmarks (after model changes)
cargo run -p gleanmark-cli -- reindex
```

### Project Structure

```
crates/
  gleanmark-core/
    src/
      embedding.rs      MultilingualE5 + SPLADE via fastembed
      storage.rs        Qdrant operations (CRUD, scroll pagination)
      search.rs         Hybrid search (RRF fusion)
      qdrant_manager.rs Qdrant process lifecycle
      models.rs         Data types and config
  gleanmark-server/
    src/main.rs         Axum routes + rust-embed static serving
    static/             Web UI (HTML/CSS/JS)
  gleanmark-cli/
    src/main.rs         CLI commands via clap
  gleanmark-tauri/
    src/
      main.rs           Tauri setup, Axum background task, splash screen
      tray.rs           System tray + global hotkey
  gleanmark-mcp/
    src/
      main.rs           MCP server entry (stdio transport)
      server.rs         Tool handlers (lazy-init GleanMark)
      types.rs          MCP tool parameter schemas
extension/
  manifest.json         Chrome Manifest V3
  popup/                Bookmark save UI
  background/           Service worker
```

### Tech Stack

| Component | Technology |
|-----------|-----------|
| Vector DB | Qdrant (embedded) |
| Dense Embeddings | MultilingualE5-Small (384d, fastembed) |
| Sparse Embeddings | SPLADE PP v1 + jieba segmentation |
| Web Framework | Axum |
| CLI | clap |
| Desktop | Tauri v2 |
| MCP | rmcp |
| Browser Extension | Chrome Manifest V3 |

## License

MIT OR Apache-2.0
