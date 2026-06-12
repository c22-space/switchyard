# Switchyard Dashboard UI — Specification

## Overview

The dashboard UI is a standalone Leptos WASM application that displays real-time routing statistics and recent route events. It fetches data from the routing server's JSON API. There is no separate dashboard server.

---

## 1. Architecture

```
trunk serve / trunk build
        |
        v
   Leptos WASM app
        |
        | fetch()
        v
   Routing server (port 8420)
   GET /api/stats
   GET /api/routes?limit=50
```

- **Development:** `trunk serve` in `crates/dashboard-ui/` starts a dev server (default port 8080). The WASM app fetches from `http://127.0.0.1:8420`.
- **Production:** `trunk build` produces static files in `dist/`. Serve with any static file server.

---

## 2. API Endpoints (consumed, not provided)

| Endpoint | Method | Query Params | Description |
|---|---|---|---|
| `/api/stats` | GET | — | Aggregate routing statistics |
| `/api/routes` | GET | `limit` (default 50) | Recent route events, newest first |

Both endpoints are served by the routing server on `server.port`.

---

## 3. UI Components

### 3.1 Stat Cards

Eight stat cards displayed in a responsive grid:
- Total Routes
- Tool Call count
- General count
- Fallback count
- Avg Latency
- P50 Latency
- P95 Latency
- Avg Score

### 3.2 Route Table

Columns: Time, Prompt, Category, Score, Backend, Latency, Status

- Category displayed as colored badge (blue for tool_call, green for general, yellow for fallback)
- Status badge (green for ok, red for error)
- Prompt truncated to 50 chars with ellipsis, full text in title attribute
- Time extracted from ISO timestamp, shown as HH:MM:SS

### 3.3 Auto-Refresh

Polls both endpoints every 5 seconds using `gloo-timers`.

---

## 4. Styling

Dark theme with GitHub-inspired color palette:
- Background: `#0f1117`
- Card background: `#161b22`
- Border: `#30363d`
- Text: `#e1e4e8`
- Accent: `#58a6ff`

---

## 5. Build

```bash
cd crates/dashboard-ui

# Development (with hot reload)
trunk serve

# Production build
trunk build --release
```

Output goes to `dist/` directory.

---

## 6. Running

The dashboard UI is independent of the routing server process. Start them separately:

1. `switchyard server` (starts routing engine on port 8420)
2. `trunk serve` or serve the built `dist/` directory (dashboard UI)

The dashboard UI expects the routing server at `http://127.0.0.1:8420` by default.
