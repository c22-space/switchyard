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
Switchyard (port 8420)
    ├── Embedding model (all-MiniLM-L6-v2)
    ├── Capability centroids (tool_call, general, ...)
    └── Backend routing
         ├── tool_call → OpenRouter (Claude Sonnet)
         └── general → Ollama (Llama 3.1)
```

## Config

```json
{
  "server": { "host": "127.0.0.1", "port": 8420 },
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
cargo build --release
./target/release/switchyard switchyard.json
```

## Test

```bash
curl -X POST http://127.0.0.1:8420/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"test","messages":[{"role":"user","content":"What is the weather in Tokyo?"}]}'
```

## Status

MVP routing engine works. Embedding + KNN routing matches Python spike results exactly (centroid similarity: 0.4121).

### What works
- Config loading
- Embedding model loading (fastembed, local ONNX)
- Centroid computation
- Cosine similarity routing
- OpenAI-compatible API endpoint
- Backend forwarding (non-streaming)
- Streaming passthrough

### What's next
- Provider plugin for Hermes
- Better separation (fine-tune embedding model or reduce to 2 categories)
- Error handling for backend failures
- Retry logic
- Health checks for backends
