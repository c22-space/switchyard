mod config;
mod router;
mod server;

use anyhow::Result;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "switchyard=info,tower_http=info".into()),
        )
        .init();

    // Load config
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "switchyard.json".to_string());

    info!("Loading config from: {}", config_path);
    let config = config::Config::load(std::path::Path::new(&config_path))?;

    // Initialize router with embedding model
    let router = router::Router::new(&config)?;

    // Build shared state
    let state = Arc::new(server::AppState {
        router,
        config: config.clone(),
        http_client: reqwest::Client::new(),
    });

    // Build axum app
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(server::routes(state))
        .layer(cors);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Switchyard listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
