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

Starts the core routing engine and (optionally) the dashboard.

**Behavior:**
1. Load and validate configuration from `--config` path.
2. Initialize the fastembed embedding model and compute capability centroids.
3. Initialize the SQLite database (create `route_events` table if not exists).
4. Start the routing proxy server on `server.host:server.port`.
5. If `dashboard.enabled` is true, start the dashboard HTTP server on `dashboard.port`.
6. Block until interrupted (Ctrl+C / SIGTERM), then gracefully shut down.

**Output:**

```
🚀 Switchyard v0.1.0
   Config:     switchyard.json
   Model:      all-MiniLM-L6-v2
   Capabilities: tool_call, general
   Threshold:  0.25
   Proxy:      http://0.0.0.0:8321
   Dashboard:  http://0.0.0.0:8421
   Database:   switchyard.db
   ────────────────────────────────────
   Ready. Listening for requests...
```

**Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `--host` | string | from config | Override server bind host |
| `--port` | integer | from config | Override server listening port |
| `--no-dashboard` | flag | false | Disable dashboard even if enabled in config |

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
📊 Routing Statistics
─────────────────────────────────
  Total Routes:    1,234
  Tool Calls:        395 (32.0%)
  General:           780 (63.2%)
  Fallbacks:           59 ( 4.8%)
─────────────────────────────────
  Avg Latency:    145.3ms
  P50 Latency:    120.1ms
  P95 Latency:    310.8ms
─────────────────────────────────
  Avg Score:        0.67
  Accuracy:        95.2%
─────────────────────────────────
```

**Color Coding:**

| Element | Color | Condition |
|---|---|---|
| Total Routes | White/bold | Always |
| Tool Calls | Blue | Always |
| General | Green | Always |
| Fallbacks | Orange | If > 5% of total |
| Fallbacks | Red | If > 15% of total |
| Accuracy ≥ 90% | Green | Always |
| Accuracy 70–89% | Yellow | Always |
| Accuracy < 70% | Red | Always |

**Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `--json` | flag | false | Output as JSON instead of formatted text |

When `--json` is used, output is a single JSON object matching the `GET /api/stats` response schema.

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

**Output Example:**

```
🔀 Recent Routes (last 10)
──────────────────────────────────────────────────────────────────────────────
  Time                  Category     Score  Backend  Latency  Status
──────────────────────────────────────────────────────────────────────────────
  2025-01-15 12:34:56   tool_call    0.82   openai   234ms    ✅ success
  2025-01-15 12:34:52   general      0.71   openai   189ms    ✅ success
  2025-01-15 12:34:48   general      0.45   ollama   567ms    ✅ success
  2025-01-15 12:34:41   fallback     0.18   ollama   123ms    ❌ error
──────────────────────────────────────────────────────────────────────────────
```

**Color Coding:**

| Element | Color |
|---|---|
| `tool_call` category | Blue |
| `general` category | Green |
| `fallback` category | Orange |
| Score ≥ 0.5 | Green |
| Score 0.25–0.49 | Yellow |
| Score < 0.25 | Red |
| `success` status | Green + ✅ |
| `error` status | Red + ❌ |

**Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `--limit` | integer | `10` | Number of recent routes to display (max 100) |
| `--json` | flag | false | Output as JSON array instead of formatted table |

When `--json` is used, output is a JSON array matching the `GET /api/routes` response schema.

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
3. Print a formatted view of all settings, with sensitive values masked.

**Output Example:**

```
⚙️  Configuration (switchyard.json)
─────────────────────────────────────────────
  Server
    Host:            0.0.0.0
    Port:            8321

  Router
    Model:           all-MiniLM-L6-v2
    Threshold:       0.25
    Fallback:        general
    Capabilities:    tool_call, general

  Backends
    1. openai (openai)
       URL:    https://api.openai.com/v1
       Key:    sk-****abcd
       Model:  gpt-4
    2. ollama (ollama)
       URL:    http://localhost:11434/v1
       Key:    (none)
       Model:  llama3

  Dashboard
    Enabled:         true
    Port:            8421
    DB Path:         switchyard.db
─────────────────────────────────────────────
```

**Sensitive Value Masking:**
- API keys are masked to show only the last 4 characters: `sk-****abcd`.
- If the key is empty, show `(none)`.
- Environment variable references like `${OPENAI_API_KEY}` are shown as-is (not resolved).

**Output Format Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `--json` | flag | false | Output as raw JSON (unmasked keys) |
| `--validate` | flag | false | Validate config and print errors only |

When `--validate` is used:
- If valid: print `✅ Configuration is valid` and exit 0.
- If invalid: print each validation error and exit 1.

**Exit Codes:**

| Code | Condition |
|---|---|
| 0 | Config loaded successfully |
| 1 | Config file not found, parse error, or validation failure |

---

## 3. Version and Help

### `switchyard --version`

```
switchyard 0.1.0
```

### `switchyard --help`

```
Switchyard — Semantic routing for LLM requests

USAGE:
    switchyard [OPTIONS] <COMMAND>

COMMANDS:
    server      Start the routing server and dashboard
    stats       Show routing statistics
    routes      Show recent routing decisions
    config      Manage configuration

OPTIONS:
    -c, --config <PATH>    Path to config file [default: switchyard.json]
    -V, --version          Print version
    -h, --help             Print help
```

---

## 4. Color Output

All commands use ANSI color codes for terminal output:

| Color | ANSI Code | Usage |
|---|---|---|
| Bold White | `\x1b[1;37m` | Headers, labels |
| Blue | `\x1b[34m` | `tool_call` category |
| Green | `\x1b[32m` | `general` category, success, high accuracy |
| Orange | `\x1b[33m` | Fallback category, warnings |
| Red | `\x1b[31m` | Errors, low accuracy, error status |
| Yellow | `\x1b[33m` | Medium accuracy, medium scores |
| Reset | `\x1b[0m` | End of colored span |

**Color Detection:**
- Colors are enabled by default when stdout is a TTY.
- Colors are disabled when stdout is piped or redirected (check `isatty`).
- Force colors with `--color=always` (global flag).
- Disable colors with `--color=never` (global flag).

---

## 5. Acceptance Criteria

### AC-1: `switchyard server` Starts Successfully

- Running `switchyard server` with a valid config starts the proxy and dashboard.
- Startup banner displays all configuration details.
- Ctrl+C triggers graceful shutdown with a "Shutting down..." message.

### AC-2: `switchyard stats` Matches Database

- After routing 100 requests, `switchyard stats` output matches the values in SQLite.
- `total_routes`, category counts, and fallback count are exact integers.
- Latency and score values are rounded to 1 decimal / 2 decimals respectively.

### AC-3: `switchyard routes` Shows Correct Data

- `switchyard routes --limit 5` outputs at most 5 rows.
- Rows are in reverse chronological order (newest first).
- Color-coded badges match category and status.

### AC-4: `switchyard config show` Masks Sensitive Data

- API keys are displayed as `sk-****abcd` (last 4 chars visible).
- Full keys are only shown with `--json` flag.

### AC-5: `--config` Flag Works

- `switchyard --config /path/to/custom.json stats` reads from the specified path.
- Missing config file produces a clear error: `Error: Config file not found: /path/to/custom.json`.

### AC-6: Color Output

- On a TTY, output includes ANSI color codes.
- When piped (`switchyard stats | cat`), output has no color codes.
- `--color=always` forces colors regardless of TTY detection.

### AC-7: JSON Output Mode

- `switchyard stats --json` produces valid JSON matching the stats API schema.
- `switchyard routes --json --limit 5` produces a valid JSON array of ≤ 5 objects.
- `switchyard config --json` produces the raw config as valid JSON.

### AC-8: Exit Codes

- All commands exit with code 0 on success.
- All commands exit with code 1 on configuration errors, missing files, or database errors.
- Error messages are written to stderr with a clear prefix: `Error: <message>`.

---

## 6. Error Messages

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
