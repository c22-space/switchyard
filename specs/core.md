# Switchyard Core — Routing Engine Specification

## Overview

The core routing engine classifies incoming LLM prompts by semantic similarity to configured capabilities, routes them to the most appropriate backend, and logs every decision to SQLite for analytics. The same server also exposes a JSON API for the dashboard UI.

---

## 1. Configuration Schema (`switchyard.json`)

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8420
  },
  "router": {
    "embedding_model": "all-MiniLM-L6-v2",
    "threshold": 0.25,
    "fallback": "general",
    "capabilities": [
      {
        "name": "tool_call",
        "examples": [
          "search for files matching a pattern",
          "read the contents of a configuration file",
          "list all running processes on the system",
          "fetch data from a remote API endpoint"
        ]
      },
      {
        "name": "general",
        "examples": [
          "explain how photosynthesis works",
          "write a poem about the ocean",
          "translate this sentence to French",
          "summarize the history of Rome"
        ]
      }
    ]
  },
  "backends": [
    {
      "name": "openai",
      "provider": "openai",
      "base_url": "https://api.openai.com/v1",
      "api_key": "${OPENAI_API_KEY}",
      "model": "gpt-4"
    },
    {
      "name": "ollama",
      "provider": "ollama",
      "base_url": "http://localhost:11434/v1",
      "api_key": "",
      "model": "llama3"
    }
  ],
  "dashboard": {
    "db_path": "switchyard.db"
  }
}
```

### Schema Rules

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `server.host` | string | no | `"0.0.0.0"` | Bind address for the routing proxy |
| `server.port` | integer | no | `8420` | Listening port for the proxy |
| `router.embedding_model` | string | yes | — | fastembed model identifier |
| `router.threshold` | float | yes | `0.25` | Minimum cosine similarity to avoid fallback |
| `router.fallback` | string | yes | — | Capability name used when no centroid exceeds threshold |
| `router.capabilities` | array | yes | — | Must contain >=1 entry; each needs `name` and `examples` (>=1 string) |
| `backends` | array | yes | — | At least one backend must be defined |
| `backends[].name` | string | yes | — | Unique identifier for routing decisions |
| `backends[].provider` | string | yes | — | `"openai"`, `"ollama"`, or custom |
| `backends[].base_url` | string | yes | — | Base URL of the provider API |
| `backends[].api_key` | string | no | `""` | May use `${ENV_VAR}` interpolation |
| `backends[].model` | string | yes | — | Model name sent to the provider |
| `dashboard.db_path` | string | no | `"switchyard.db"` | Path to SQLite database file |

---

## 2. Routing Algorithm

### 2.1 Initialization

1. Load `switchyard.json` from the current directory (or `--config` path).
2. Load the embedding model specified by `router.embedding_model` via the **fastembed** library.
3. For each capability in `router.capabilities`:
   a. Embed every example string using the loaded model.
   b. Compute the **centroid** as the element-wise mean of all example embeddings.
   c. **L2-normalize** the centroid so it lies on the unit hypersphere.
4. Store the normalized centroids in a lookup table keyed by capability name.

### 2.2 Route Request

```
POST /v1/chat/completions   (OpenAI-compatible proxy)
Body: { "messages": [...], "model": "...", ... }
```

1. Extract the most recent user message text from `messages`.
2. Embed the prompt text using the same fastembed model.
3. For each capability centroid, compute **cosine similarity** between the prompt embedding and the centroid.
4. Select the capability with the **highest cosine similarity score**.
5. If the highest score **< `router.threshold`**, use `router.fallback` instead and set `is_fallback = 1`.
6. Route the request to the backend associated with the selected capability (or the first backend if capability-to-backend mapping is not explicit).
7. Forward the response from the chosen backend back to the caller.
8. Log the routing decision (see section 3).

### 2.3 Performance Requirements

- Embedding computation must complete in < 50ms for a single prompt.
- Cosine similarity for N capabilities completes in < 1ms.
- Total routing overhead (excluding backend latency) must be < 100ms.

---

## 3. Event Logging (SQLite)

### 3.1 Table Schema: `route_events`

```sql
CREATE TABLE route_events (
    id          TEXT PRIMARY KEY,       -- UUID v4
    timestamp   TEXT NOT NULL,          -- ISO 8601 with timezone
    prompt      TEXT NOT NULL,          -- User message text
    category    TEXT NOT NULL,          -- Selected capability name
    score       REAL NOT NULL,          -- Cosine similarity score
    is_fallback INTEGER NOT NULL,       -- 1 if fallback was used, 0 otherwise
    backend     TEXT NOT NULL,          -- Name of the backend chosen
    model       TEXT NOT NULL,          -- Model identifier used
    latency_ms  REAL NOT NULL,          -- Total request latency in ms
    status      TEXT NOT NULL,          -- "success" or "error"
    error       TEXT                    -- Error message if status = "error", else NULL
);

CREATE INDEX idx_route_events_timestamp ON route_events(timestamp);
CREATE INDEX idx_route_events_category ON route_events(category);
```

### 3.2 Logging Behavior

- A new row is inserted for **every** routing decision, successful or not.
- `id` is a randomly generated UUID v4.
- `timestamp` is the current UTC time in ISO 8601 format (e.g. `2025-01-15T12:34:56.789Z`).
- `latency_ms` is measured from request receipt to response completion.
- `error` is populated only when `status = "error"`.

---

## 4. Acceptance Criteria

### AC-1: Centroid Similarity Verification

| Criterion | Expected Value | Tolerance |
|---|---|---|
| Cosine similarity between `tool_call` and `general` centroids (all-MiniLM-L6-v2) | ~0.41 | +/-0.03 |

**Test:** Load the two default capability categories, compute centroids, measure cosine similarity between them. The value should fall within 0.38-0.44.

### AC-2: Routing Accuracy at Threshold 0.25

| Category | Minimum Accuracy | Notes |
|---|---|---|
| `tool_call` | ~70% | Tool-oriented prompts should route correctly >=70% of the time |
| `general` | ~95% | General prompts should route correctly >=95% of the time |

**Test:** Send a curated set of 50 prompts (25 tool_call, 25 general) through the routing engine and verify classification accuracy meets the thresholds above.

### AC-3: Fallback Behavior

- Any prompt whose highest cosine similarity < 0.25 must be routed to `router.fallback`.
- The `is_fallback` field must be `1` in the event log for these cases.

### AC-4: Event Logging Completeness

- After N requests, `route_events` must contain exactly N rows.
- Every row must have a valid UUID, ISO 8601 timestamp, and non-null fields.

### AC-5: Configuration Validation

- Missing required fields (`embedding_model`, `threshold`, `fallback`, `capabilities`, `backends`) must produce a clear error on startup.
- Invalid JSON must fail with a parse error message.

---

## 5. HTTP Interface

The server exposes endpoints on a single port (`server.port`):

### 5.1 OpenAI-Compatible Proxy

```
POST /v1/chat/completions
Content-Type: application/json

{
  "model": "<any string, ignored, model is selected by routing>",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "Search for all Python files in /tmp" }
  ],
  "stream": false
}
```

**Response:** Proxied response from the selected backend, with `switchyard_route` JSON in the response body indicating the routing decision.

### 5.2 Health Check

```
POST /health
```

**Response:** `{ "status": "ok" }`

### 5.3 Route Statistics

```
GET /api/stats
```

**Response:**
```json
{
  "total_routes": 150,
  "tool_call_count": 100,
  "general_count": 30,
  "fallback_count": 20,
  "avg_latency_ms": 12.5,
  "p50_latency_ms": 10.2,
  "p95_latency_ms": 25.8,
  "avg_score": 0.45,
  "accuracy_pct": 86.7
}
```

### 5.4 Recent Route Events

```
GET /api/routes?limit=50
```

**Response:**
```json
[
  {
    "id": "uuid",
    "timestamp": "2025-01-15T12:34:56.789Z",
    "prompt": "Search for all Python files...",
    "category": "tool_call",
    "score": 0.52,
    "is_fallback": false,
    "backend": "ollama",
    "model": "llama3",
    "latency_ms": 12.5,
    "status": "ok",
    "error": null
  }
]
```

---

## 6. Error Handling

| Condition | Behavior |
|---|---|
| Missing/invalid config file | Return startup error, exit code 1 |
| Embedding model fails to load | Return startup error, exit code 1 |
| Backend unreachable | Log error event, return HTTP 502 to caller |
| Backend returns error | Log error event, forward error response to caller |
| Malformed request body | Return HTTP 400 with descriptive message |
| SQLite write failure | Log warning to stderr, continue routing (best-effort logging) |
