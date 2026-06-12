# Switchyard Dashboard UI — Specification

## Overview

The dashboard is a static Astro + React application built to `crates/dashboard-ui/dist/`. The Axum server serves these files directly on the same port as the API (4855). There is no separate dashboard server.

---

## 1. Architecture

```
npm run build (Astro)
        |
        v
   Static HTML/JS/CSS in dist/
        |
        | served by Axum ServeDir fallback
        v
   Switchyard server (port 4855)
   ├── /api/stats
   ├── /api/routes?limit=50
   ├── /api/overview
   ├── /api/providers (GET/POST)
   └── /* (static files from dist/)
```

- **Development:** `npm run dev` in `crates/dashboard-ui/` starts Astro dev server at `localhost:4321`. API calls proxy to port 4855.
- **Production:** `npm run build` produces static files in `dist/`. Restart the Switchyard server to serve them.

---

## 2. API Endpoints (consumed)

| Endpoint | Method | Query Params | Description |
|---|---|---|---|
| `/api/overview` | GET | — | Stats + config summary |
| `/api/stats` | GET | — | Aggregate routing statistics |
| `/api/routes` | GET | `limit` (default 50) | Recent route events, newest first |
| `/api/providers` | GET | — | List all backends |
| `/api/providers` | POST | — | Add a new backend |

All endpoints are served by the routing server on `server.port` (4855).

---

## 3. UI Components

### 3.1 Sidebar Navigation

Three tabs: Overview, Routes, Config. Implemented with React `useState` for tab switching.

### 3.2 Overview Tab

Eight stat cards in a responsive grid:
- Total Routes, Tool Call count, General count, Fallback count
- Avg Latency, P95 Latency, Accuracy, Avg Score

Below the cards: Router Config section showing backends count, capabilities count, embedding model, threshold, fallback.

### 3.3 Routes Tab

Table with columns: Time, Prompt, Category, Score, Backend, Latency, Status.

- Category: colored text (green for tool_call, blue for general, yellow for fallback)
- Status: green for ok, red for error
- Prompt truncated to 60 chars
- Limit selector (10, 20, 50, 100)

### 3.4 Config Tab

Provider management:
- List of current providers (name, provider, model, base_url)
- "+ Add Provider" button opens inline form
- Form fields: Name, Provider, Base URL, Model
- POST to `/api/providers` on submit (persists to `switchyard.json`)

---

## 4. Styling

Dark theme with Zinc color palette:
- Background: `#09090b`
- Surface: `#18181b`
- Border: `#27272a`
- Text: `#fafafa`
- Muted text: `#a1a1aa`
- Accent: `#3b82f6`

All styles are inline React styles (no external CSS framework).

---

## 5. Build

```bash
cd crates/dashboard-ui

# Install dependencies
npm install

# Development (hot reload)
npm run dev

# Production build
npm run build
```

Output goes to `dist/` directory. Restart Switchyard server to serve new files.

---

## 6. Running

The dashboard is served by the Switchyard server on the same port:

1. `cd crates/dashboard-ui && npm run build` (build dashboard)
2. `cd /home/charlie/switchyard && RUST_LOG=info ./target/debug/switchyard server` (starts on port 4855)

Access at `http://127.0.0.1:4855` or via Cloudflare tunnel.

---

## 7. Project Structure

```
crates/dashboard-ui/
├── astro.config.mjs      # Astro config with React integration
├── package.json           # Dependencies (astro, react, react-dom)
├── tsconfig.json          # TypeScript config
├── public/                # Static assets (favicon)
├── src/
│   ├── layouts/
│   │   └── Layout.astro   # Base HTML layout with global styles
│   ├── pages/
│   │   └── index.astro    # Entry point, mounts Dashboard component
│   └── components/
│       ├── Dashboard.tsx   # Main component with sidebar nav + tab routing
│       ├── Overview.tsx    # Stats cards + router config
│       ├── Routes.tsx      # Route event table
│       └── Config.tsx      # Provider list + add form
└── dist/                  # Build output (served by Axum)
```
