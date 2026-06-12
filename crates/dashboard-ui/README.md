# Switchyard Dashboard UI

Astro + React dashboard for the Switchyard capability router.

## Setup

```bash
npm install
```

## Development

```bash
npm run dev    # Starts at localhost:4321 with hot reload
```

The dev server proxies API calls to `http://127.0.0.1:4855`.

## Production Build

```bash
npm run build  # Outputs to dist/
```

After building, restart the Switchyard server to serve the new static files.

## Stack

- **Astro** — Static site generation, page routing
- **React** — Interactive UI components (client-side hydrated)
- **TypeScript** — Type safety

## Project Structure

```
src/
├── layouts/
│   └── Layout.astro       # Base HTML layout, global CSS variables
├── pages/
│   └── index.astro        # Entry point, mounts Dashboard
└── components/
    ├── Dashboard.tsx       # Sidebar nav + tab switching
    ├── Overview.tsx        # Stats cards + router config
    ├── Routes.tsx          # Route event table with limit selector
    └── Config.tsx          # Provider list + add form
```

## API Endpoints Used

| Endpoint | Method | Description |
|---|---|---|
| `/api/overview` | GET | Stats + config summary |
| `/api/stats` | GET | Aggregate routing statistics |
| `/api/routes` | GET | Recent route events (`?limit=N`) |
| `/api/providers` | GET | List all backends |
| `/api/providers` | POST | Add a new backend |

All API calls use relative paths (same-origin serving).

## Styling

Dark theme using inline React styles with CSS custom properties:
- Background: `#09090b`
- Surface: `#18181b`
- Border: `#27272a`
- Accent: `#3b82f6`
