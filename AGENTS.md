# AGENTS.md — Switchyard

Instructions for AI agents working on this repository.

## Project Overview

Switchyard is a capability router for agentic workflows. It embeds user prompts, computes cosine similarity to configured capability centroids, and routes to the best-matching LLM backend. Built in Rust (Axum) with an Astro + React dashboard.

## Quick Reference

| What | Where |
|---|---|
| Server binary | `crates/cli/src/main.rs` |
| Routing engine | `crates/core/src/router.rs` |
| Config structs | `crates/core/src/config.rs` |
| Event store (SQLite) | `crates/core/src/event.rs` |
| Dashboard UI | `crates/dashboard-ui/` (Astro + React) |
| Runtime config | `switchyard.json` |
| Design specs | `specs/` |

## Build Commands

```bash
# Rust binary
cargo build

# Dashboard (from crates/dashboard-ui/)
npm install
npm run build    # Static output to dist/
```

## Running

```bash
cd /home/charlie/switchyard
RUST_LOG=info ./target/debug/switchyard server
```

Server runs on port 4855. Serves both API and dashboard static files from a single port.

## API Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/v1/chat/completions` | POST | OpenAI-compatible routing proxy |
| `/health` | POST | Health check |
| `/api/stats` | GET | Aggregate routing statistics |
| `/api/routes` | GET | Recent route events (`?limit=N`) |
| `/api/overview` | GET | Stats + config summary |
| `/api/providers` | GET | List all backends |
| `/api/providers` | POST | Add a new backend (persists to config) |
| `/*` | GET | Static dashboard files (fallback) |

## Code Conventions

- **Rust edition:** 2021
- **Async runtime:** Tokio
- **HTTP framework:** Axum
- **Static file serving:** `tower-http` `ServeDir`
- **Dashboard:** Astro + React (static build, client-side hydration)
- **Styling:** Inline React styles, Zinc dark theme
- **Config format:** JSON (`switchyard.json`)

## Key Architecture Decisions

1. **Single port (4855):** API and dashboard share one port. Axum routes handle API paths; `ServeDir` fallback serves static files from `crates/dashboard-ui/dist/`.

2. **Static dashboard:** Built with `npm run build` in `crates/dashboard-ui/`. Output goes to `dist/`. Server must be restarted after rebuild to pick up new files.

3. **Relative API paths:** Dashboard fetches use relative URLs (`/api/overview`), not hardcoded localhost. This works through the Cloudflare tunnel.

4. **Provider persistence:** `POST /api/providers` writes to `switchyard.json`. Server restart needed for routing to use new backends.

5. **Event logging:** Every routing decision logged to SQLite (`switchyard.db`). Dashboard reads from the same DB.

## Common Tasks

### Add a new API endpoint

1. Add route in `crates/cli/src/main.rs` (`axum::Router`)
2. Add handler function in same file
3. If it needs new data structures, add to `crates/core/`
4. Update `specs/core.md`

### Modify the dashboard

1. Edit React components in `crates/dashboard-ui/src/components/`
2. Run `npm run build` from `crates/dashboard-ui/`
3. Restart Switchyard server
4. Hard-refresh browser (Ctrl+Shift+R)

### Change config schema

1. Update structs in `crates/core/src/config.rs`
2. Update `switchyard.json` with new fields
3. Update handler in `crates/cli/src/main.rs` if needed
4. Update `specs/core.md`

## Pitfalls

- **Dashboard cache:** Cloudflare tunnel caches aggressively. After rebuild, users may need to hard-refresh (Ctrl+Shift+R) or purge cache.
- **Server restart required:** Static files are read at startup. Rebuild dashboard, then restart server.
- **Config writes are not hot-reloaded:** Adding a provider via API persists to `switchyard.json` but the in-memory config doesn't update until restart.
- **Node modules not committed:** `node_modules/` and `dist/` are in `.gitignore`. Run `npm install` and `npm run build` after cloning.
