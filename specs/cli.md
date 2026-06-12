# Switchyard CLI Specification

## Overview

The `switchyard` CLI is the primary user-facing interface for starting the routing server, querying statistics, inspecting recent routes, and viewing configuration. It provides colored terminal output for readability.

---

## 1. Global Options

All commands accept the following flags:

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--config` | `-c` | string | `switchyard.json` | Path to configuration file |

**Usage:**

```
switchyard [--config <path>] <command> [options]
```

---

## 2. Commands

### 2.1 `switchyard server`

Starts the routing server with all endpoints (proxy, stats, routes, health).

**Behavior:**
1. Load and validate configuration from `--config` path.
2. Initialize the fastembed embedding model and compute capability centroids.
3. Initialize the SQLite database (create `route_events` table if not exists).
4. Start the HTTP server on `server.host:server.port` with all endpoints.
5. Block until interrupted (Ctrl+C / SIGTERM), then gracefully shut down.

**Output:**

```
Switchyard starting...
  Routing server: 0.0.0.0:8420

Server ready.
```

**Endpoints served:**
- `POST /v1/chat/completions` — OpenAI-compatible routing proxy
- `POST /health` — Health check
- `GET /api/stats` — Routing statistics
- `GET /api/routes` — Recent route events

**Exit Codes:**

| Code | Condition |
|---|---|
| 0 | Graceful shutdown |
| 1 | Configuration error, missing config file, or startup failure |

---

### 2.2 `switchyard stats`

Displays routing statistics from the SQLite database.

**Behavior:**
1. Open the SQLite database at `dashboard.db_path` (from config).
2. Compute all statistics (total, category counts, fallback, latency, score, accuracy).
3. Print formatted, color-coded output to stdout.

**Output Example:**

```
Switchyard Routing Statistics

  Total Routes:       1234
  Tool Call:          395
  General:            780
  Fallback:           59

  Avg Latency:        145.3ms
  P50 Latency:        120.1ms
  P95 Latency:        310.8ms

  Avg Score:          0.6700
  Routing Accuracy:   95.2%
```

**Exit Codes:**

| Code | Condition |
|---|---|
| 0 | Success |
| 1 | Database not found or unreadable |

---

### 2.3 `switchyard routes`

Displays recent routing events from the SQLite database.

**Behavior:**
1. Open the SQLite database.
2. Query the most recent N route events.
3. Print a formatted table to stdout.

**Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `--limit` | integer | `20` | Number of recent routes to display |

**Exit Codes:**

| Code | Condition |
|---|---|
| 0 | Success |
| 1 | Database not found or unreadable |

---

### 2.4 `switchyard config show`

Displays the current configuration.

**Behavior:**
1. Load the config file from `--config` path.
2. Parse and validate the JSON.
3. Print a formatted view of all settings.

**Exit Codes:**

| Code | Condition |
|---|---|
| 0 | Config loaded successfully |
| 1 | Config file not found, parse error, or validation failure |

---

## 3. Acceptance Criteria

### AC-1: `switchyard server` Starts Successfully

- Running `switchyard server` with a valid config starts the server.
- Startup banner displays configuration details.
- Ctrl+C triggers graceful shutdown.

### AC-2: `switchyard stats` Matches Database

- After routing 100 requests, `switchyard stats` output matches the values in SQLite.

### AC-3: `switchyard routes` Shows Correct Data

- `switchyard routes --limit 5` outputs at most 5 rows.
- Rows are in reverse chronological order (newest first).

### AC-4: `--config` Flag Works

- `switchyard --config /path/to/custom.json stats` reads from the specified path.
- Missing config file produces a clear error.

---

## 4. Error Messages

All error messages follow a consistent format:

```
Error: <descriptive message>
```

| Scenario | Error Message |
|---|---|
| Config file not found | `Error: Config file not found: <path>` |
| Invalid JSON | `Error: Failed to parse config: <parse error>` |
| Missing required field | `Error: Config missing required field: <field>` |
| Database not found | `Error: Database not found: <path>` |
| Database locked | `Error: Database is locked, try again later` |
| Port in use | `Error: Port <port> is already in use` |
| Unknown command | `Error: Unknown command '<cmd>'. Run 'switchyard --help' for usage` |
