# Switchyard

Capability router for agentic workflows. A tiny Rust engine that runs locally and routes prompts to the best available model based on capability needs.

## How it works

1. Accepts an OpenAI-compatible API request
2. Embeds the user prompt using a local embedding model
3. Computes cosine similarity to pre-defined capability centroids
4. Routes to the best-matching backend
5. Falls back to the configured fallback model if confidence is low

## Architecture

```
Client (OpenAI-compatible)
    ↓
Switchyard (port 4855)
    ├── API routes (/v1/chat/completions, /health, /api/*)
    ├── Static dashboard (Astro + React, served from dist/)
    ├── Embedding model (all-MiniLM-L6-v2)
    ├── Capability centroids (tool_call, general, ...)
    └── Backend routing
         ├── tool_call → OpenRouter (Claude Sonnet)
         └── general → Ollama (Llama 3.1)
```

## Project Structure

```
switchyard/
├── crates/
│   ├── cli/           # Axum server + CLI binary
│   ├── core/          # Routing engine, config, event store
│   └── dashboard-ui/  # Astro + React dashboard (static build)
├── specs/             # Design specifications
├── switchyard.json    # Runtime config
└── AGENTS.md          # Agent instructions
```

## Config

```json
{
  "server": { "host": "127.0.0.1", "port": 4855 },
  "router": {
    "embedding_model": "all-MiniLM-L6-v2",
    "threshold": 0.25,
    "fallback": "general",
    "capabilities": [
      { "name": "tool_call", "examples": ["..."] },
      { "name": "general", "examples": ["..."] }
    ]
  },
  "backends": [
    { "name": "tool_call", "provider": "openrouter", "base_url": "...", "model": "..." },
    { "name": "general", "provider": "ollama", "base_url": "...", "model": "..." }
  ]
}
```

## Build & Run

```bash
# Build the Rust binary
cargo build

# Start the server (serves API + dashboard)
cd /home/charlie/switchyard
RUST_LOG=info ./target/debug/switchyard server
```

The server serves both the API and the dashboard static files on a single port (4855).

### Dashboard (development)

```bash
cd crates/dashboard-ui
npm install
npm run dev    # Dev server at localhost:4321 (with hot reload)
npm run build  # Static build to dist/
```

After `npm run build`, restart the Switchyard server to pick up new static files.

## API Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/v1/chat/completions` | POST | OpenAI-compatible routing proxy |
| `/health` | POST | Health check |
| `/api/stats` | GET | Aggregate routing statistics |
| `/api/routes` | GET | Recent route events (`?limit=50`) |
| `/api/overview` | GET | Stats + config summary (for dashboard) |
| `/api/providers` | GET | List all backends |
| `/api/providers` | POST | Add a new backend (persists to config) |

## Test

```bash
curl -X POST http://127.0.0.1:4855/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"test","messages":[{"role":"user","content":"What is the weather in Tokyo?"}]}'
```

### What works
- Config loading
- Embedding model loading (fastembed, local ONNX)
- Centroid computation
- Cosine similarity routing
- OpenAI-compatible API endpoint
- Backend forwarding (non-streaming)
- Streaming passthrough
- Dashboard with Overview, Routes, Config tabs
- Provider management (add via dashboard)
- Single-port serving (API + static files)

### What's next
- Provider plugin for Hermes
- Better separation (fine-tune embedding model or reduce to 2 categories)
- Error handling for backend failures
- Retry logic
- Health checks for backends
