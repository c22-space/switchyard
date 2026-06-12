# Switchyard Dashboard Specification

## Overview

The dashboard provides a real-time web UI and JSON API for monitoring routing activity, viewing statistics, and inspecting recent routing decisions. It reads directly from the same SQLite database used by the core routing engine.

---

## 1. Server Configuration

| Setting | Source | Default |
|---|---|---|
| Port | `dashboard.port` in `switchyard.json` | `8421` |
| Database | `dashboard.db_path` in `switchyard.json` | `switchyard.db` |
| Enabled | `dashboard.enabled` in `switchyard.json` | `true` |

The dashboard server starts as a child task of the main routing server. When `dashboard.enabled` is `false`, no HTTP server is started.

---

## 2. HTTP Endpoints

### 2.1 `GET /` — Dashboard UI

**Response:** `200 OK` with `Content-Type: text/html`

Returns a self-contained HTML page with the following characteristics:

#### Visual Design

- **Dark theme** background (`#1a1a2e` or similar dark palette).
- Clean, modern layout using system fonts (no external CDN dependencies).
- Responsive design that works on desktop and tablet viewports.

#### Auto-Refresh

- The page automatically refreshes its data every **5 seconds**.
- Implementation: JavaScript `setInterval` that fetches `/api/stats` and `/api/routes?limit=20` and updates the DOM without full page reload.

#### Layout Structure

```
┌──────────────────────────────────────────────┐
│  Switchyard Dashboard                        │
├──────────┬──────────┬──────────┬─────────────┤
│ Total    │ Tool     │ General  │ Fallback    │
│ Routes   │ Call %   │ %        │ %           │
│ 1,234    │ 32%      │ 63%      │ 5%          │
├──────────┴──────────┴──────────┴─────────────┤
│ Avg Latency │ P50     │ P95     │ Avg Score  │
│ 145ms       │ 120ms   │ 310ms   │ 0.67       │
├─────────────┴─────────┴─────────┴────────────┤
│ Recent Routes (table)                         │
│ Time | Category | Score | Backend | Status    │
│ ...                                          │
└──────────────────────────────────────────────┘
```

#### Stat Cards

- Display 4 primary metrics: `total_routes`, `tool_call_count` (with percentage), `general_count` (with percentage), `fallback_count` (with percentage).
- Display 4 secondary metrics: `avg_latency_ms`, `p50_latency_ms`, `p95_latency_ms`, `avg_score`.
- **Accuracy card** showing `accuracy_pct` with color coding:
  - 🟢 Green if ≥ 90%
  - 🟡 Yellow if ≥ 70% and < 90%
  - 🔴 Red if < 70%

#### Routes Table

- Shows the 20 most recent route events.
- Columns: Timestamp, Prompt (truncated to 60 chars), Category, Score, Backend, Latency, Status.
- **Color-coded badges:**
  - `tool_call` → blue badge
  - `general` → green badge
  - `fallback` → orange badge
  - `success` status → green dot
  - `error` status → red dot
- Prompts are displayed as-is but HTML-escaped to prevent XSS.

---

### 2.2 `GET /api/routes` — Recent Route Events

**Query Parameters:**

| Param | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | `20` | Max number of events to return (cap: 500) |

**Response:** `200 OK` with `Content-Type: application/json`

```json
[
  {
    "id": "a1b2c3d4-...",
    "timestamp": "2025-01-15T12:34:56.789Z",
    "prompt": "Search for all Python files in /tmp",
    "category": "tool_call",
    "score": 0.82,
    "is_fallback": 0,
    "backend": "openai",
    "model": "gpt-4",
    "latency_ms": 234.5,
    "status": "success",
    "error": null
  }
]
```

**Behavior:**
- Results are ordered by `timestamp DESC` (most recent first).
- If `limit` exceeds 500, cap at 500.
- If the database file does not exist or table is empty, return `[]`.
- The dashboard JS calls this endpoint as `GET /api/routes?limit=20`.

---

### 2.3 `GET /api/stats` — Route Statistics

**Response:** `200 OK` with `Content-Type: application/json`

```json
{
  "total_routes": 1234,
  "tool_call_count": 395,
  "general_count": 780,
  "fallback_count": 59,
  "avg_latency_ms": 145.3,
  "p50_latency_ms": 120.1,
  "p95_latency_ms": 310.8,
  "avg_score": 0.67,
  "accuracy_pct": 85.2
}
```

**Field Definitions:**

| Field | Type | Computation |
|---|---|---|
| `total_routes` | integer | `COUNT(*)` from `route_events` |
| `tool_call_count` | integer | `COUNT(*)` WHERE `category = 'tool_call'` AND `is_fallback = 0` |
| `general_count` | integer | `COUNT(*)` WHERE `category = 'general'` AND `is_fallback = 0` |
| `fallback_count` | integer | `COUNT(*)` WHERE `is_fallback = 1` |
| `avg_latency_ms` | float | `AVG(latency_ms)` rounded to 1 decimal |
| `p50_latency_ms` | float | 50th percentile of `latency_ms` (median) |
| `p95_latency_ms` | float | 95th percentile of `latency_ms` |
| `avg_score` | float | `AVG(score)` rounded to 2 decimals |
| `accuracy_pct` | float | `((total_routes - fallback_count) / total_routes) * 100`, or `0.0` if `total_routes = 0` |

**Behavior:**
- All numeric values are computed from the full `route_events` table (no time window).
- Percentile calculations use SQLite's `ORDER BY` with integer index approach.
- If the database is empty, return all zeros.

---

## 3. Acceptance Criteria

### AC-1: Dashboard Loads

- `GET /` returns HTTP 200 with `Content-Type: text/html`.
- The HTML contains stat cards and a routes table.
- No external CDN references (fully self-contained).

### AC-2: Stats API Returns Correct Data

- After inserting 10 known route events (7 general, 2 tool_call, 1 fallback), `GET /api/stats` returns:
  - `total_routes = 10`
  - `general_count = 7`
  - `tool_call_count = 2`
  - `fallback_count = 1`
  - `accuracy_pct = 90.0`

### AC-3: Routes API Returns Correct Limit

- `GET /api/routes?limit=5` returns at most 5 entries.
- Results are in reverse chronological order.
- Each entry contains all required fields.

### AC-4: Auto-Refresh Works

- The HTML dashboard JavaScript fetches `/api/stats` and `/api/routes?limit=20` every 5 seconds.
- Data updates without full page reload.
- No JavaScript errors in browser console.

### AC-5: Color-Coded Badges

- Category badges use distinct colors: `tool_call` = blue, `general` = green, fallback = orange.
- Status indicators use green for success, red for error.

### AC-6: Dark Theme

- Background color is a dark shade (#1a1a2e or similar).
- Text is light-colored for readability.
- Cards and table use appropriate contrast.

### AC-7: Error Handling

- If the database file doesn't exist, the dashboard still loads (empty stats, empty routes).
- If the SQLite query fails, return HTTP 500 with a JSON error body.
- If the dashboard port is already in use, log an error and continue without crashing the main routing server.

---

## 4. Database Access

The dashboard reads from the SQLite database specified by `dashboard.db_path`. It uses **read-only** connections and never writes to the database. The core engine is the sole writer.

### Read-Only Queries

```sql
-- Total routes
SELECT COUNT(*) FROM route_events;

-- Category counts (non-fallback)
SELECT category, COUNT(*) FROM route_events WHERE is_fallback = 0 GROUP BY category;

-- Fallback count
SELECT COUNT(*) FROM route_events WHERE is_fallback = 1;

-- Latency percentiles
SELECT latency_ms FROM route_events ORDER BY latency_ms;

-- Average score
SELECT AVG(score) FROM route_events;

-- Recent routes
SELECT * FROM route_events ORDER BY timestamp DESC LIMIT ?;
```

---

## 5. Deployment Notes

- The dashboard binds to `0.0.0.0:<port>` by default (configurable via `dashboard.port`).
- The dashboard is designed to run on the same machine as the core engine, sharing the SQLite file.
- For remote access, the user should configure a reverse proxy (nginx/caddy) in front of the dashboard port.
- The dashboard has no authentication by default. If exposed to a network, authentication should be added via reverse proxy.
