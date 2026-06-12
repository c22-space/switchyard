use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use switchyard_core::event::EventStore;
use tower_http::services::ServeDir;

pub struct DashboardState {
    pub event_store: EventStore,
    pub dist_dir: PathBuf,
}

pub fn routes(state: Arc<DashboardState>) -> Router {
    let dist_service = ServeDir::new(&state.dist_dir)
        .append_index_html_on_directories(true);

    Router::new()
        .route("/api/routes", get(recent_routes))
        .route("/api/stats", get(route_stats))
        .fallback_service(dist_service)
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
