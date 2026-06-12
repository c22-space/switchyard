use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use switchyard_core::event::EventStore;

pub struct DashboardState {
    pub event_store: EventStore,
}

pub fn routes(state: Arc<DashboardState>) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/api/routes", get(recent_routes))
        .route("/api/stats", get(route_stats))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct RouteQuery {
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    50
}

async fn index_page() -> impl IntoResponse {
    Html(DASHBOARD_HTML.to_string())
}

async fn recent_routes(
    State(state): State<Arc<DashboardState>>,
    Query(query): Query<RouteQuery>,
) -> Result<Json<Vec<switchyard_core::event::RouteEvent>>, StatusCode> {
    state
        .event_store
        .recent_routes(query.limit)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn route_stats(
    State(state): State<Arc<DashboardState>>,
) -> Result<Json<switchyard_core::event::RouteStats>, StatusCode> {
    state
        .event_store
        .stats()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Switchyard Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f1117; color: #e1e4e8; padding: 24px; }
        h1 { font-size: 24px; margin-bottom: 24px; color: #58a6ff; }
        .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px; margin-bottom: 32px; }
        .stat-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; }
        .stat-card .label { font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }
        .stat-card .value { font-size: 28px; font-weight: 600; margin-top: 4px; }
        .stat-card .value.green { color: #3fb950; }
        .stat-card .value.blue { color: #58a6ff; }
        .stat-card .value.yellow { color: #d29922; }
        .stat-card .value.red { color: #f85149; }
        table { width: 100%; border-collapse: collapse; background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; }
        th { background: #1c2128; text-align: left; padding: 12px 16px; font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; border-bottom: 1px solid #30363d; }
        td { padding: 10px 16px; border-bottom: 1px solid #21262d; font-size: 14px; }
        tr:hover { background: #1c2128; }
        .badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 12px; font-weight: 500; }
        .badge.tool_call { background: #1f3a5f; color: #58a6ff; }
        .badge.general { background: #1a3a1a; color: #3fb950; }
        .badge.fallback { background: #3d2e00; color: #d29922; }
        .badge.error { background: #3d1414; color: #f85149; }
        .prompt { max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #8b949e; }
        .score { font-family: monospace; }
        .refresh { margin-bottom: 16px; }
        .refresh button { background: #21262d; border: 1px solid #30363d; color: #e1e4e8; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-size: 14px; }
        .refresh button:hover { background: #30363d; }
    </style>
</head>
<body>
    <h1>Switchyard Dashboard</h1>
    <div class="refresh"><button onclick="loadAll()">Refresh</button></div>
    <div class="stats" id="stats"></div>
    <table>
        <thead>
            <tr>
                <th>Time</th>
                <th>Prompt</th>
                <th>Category</th>
                <th>Score</th>
                <th>Backend</th>
                <th>Latency</th>
                <th>Status</th>
            </tr>
        </thead>
        <tbody id="routes"></tbody>
    </table>
    <script>
        async function loadStats() {
            const res = await fetch('/api/stats');
            const s = await res.json();
            document.getElementById('stats').innerHTML = `
                <div class="stat-card"><div class="label">Total Routes</div><div class="value blue">${s.total_routes}</div></div>
                <div class="stat-card"><div class="label">Tool Call</div><div class="value green">${s.tool_call_count}</div></div>
                <div class="stat-card"><div class="label">General</div><div class="value green">${s.general_count}</div></div>
                <div class="stat-card"><div class="label">Fallback</div><div class="value yellow">${s.fallback_count}</div></div>
                <div class="stat-card"><div class="label">Avg Latency</div><div class="value blue">${s.avg_latency_ms.toFixed(1)}ms</div></div>
                <div class="stat-card"><div class="label">P50 Latency</div><div class="value blue">${s.p50_latency_ms.toFixed(1)}ms</div></div>
                <div class="stat-card"><div class="label">P95 Latency</div><div class="value yellow">${s.p95_latency_ms.toFixed(1)}ms</div></div>
                <div class="stat-card"><div class="label">Avg Score</div><div class="value blue">${s.avg_score.toFixed(4)}</div></div>
            `;
        }
        async function loadRoutes() {
            const res = await fetch('/api/routes?limit=50');
            const routes = await res.json();
            const tbody = document.getElementById('routes');
            tbody.innerHTML = routes.map(r => `
                <tr>
                    <td>${new Date(r.timestamp).toLocaleTimeString()}</td>
                    <td class="prompt" title="${r.prompt.replace(/"/g, '&quot;')}">${r.prompt}</td>
                    <td><span class="badge ${r.is_fallback ? 'fallback' : r.category}">${r.category}${r.is_fallback ? ' (fb)' : ''}</span></td>
                    <td class="score">${r.score.toFixed(4)}</td>
                    <td>${r.backend}</td>
                    <td>${r.latency_ms ? r.latency_ms.toFixed(0) + 'ms' : '-'}</td>
                    <td><span class="badge ${r.status === 'ok' ? 'general' : 'error'}">${r.status}</span></td>
                </tr>
            `).join('');
        }
        function loadAll() { loadStats(); loadRoutes(); }
        loadAll();
        setInterval(loadAll, 5000);
    </script>
</body>
</html>"#;
