use anyhow::Result;
use axum::response::IntoResponse;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use std::sync::Arc;
use switchyard_core::config::Config;
use switchyard_core::event::EventStore;
use switchyard_core::router::Router;

#[derive(Parser)]
#[command(name = "switchyard", about = "Capability router for agentic workflows")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "switchyard.json")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the routing server
    Server,

    /// Show routing statistics
    Stats,

    /// Show recent route events
    Routes {
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },

    /// Show current configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Display current config
    Show,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "switchyard=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Server => cmd_server(&cli.config).await,
        Commands::Stats => cmd_stats(&cli.config),
        Commands::Routes { limit } => cmd_routes(&cli.config, limit),
        Commands::Config {
            action: ConfigAction::Show,
        } => cmd_config_show(&cli.config),
    }
}

async fn cmd_server(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    let router = Router::new(&config)?;

    // Initialize event store
    let db_path = std::path::Path::new(&config.dashboard.db_path);
    let event_store = EventStore::new(db_path)?;

    println!("{}", "Switchyard starting...".green().bold());
    println!(
        "  Routing server: {}:{}",
        config.server.host, config.server.port
    );
    println!();

    let state = Arc::new(RouteState {
        router,
        config: config.clone(),
        http_client: reqwest::Client::new(),
        event_store,
    });

    let app = axum::Router::new()
        .route(
            "/v1/chat/completions",
            axum::routing::post(chat_completions),
        )
        .route("/health", axum::routing::post(health))
        .route("/api/stats", axum::routing::get(route_stats))
        .route("/api/routes", axum::routing::get(recent_routes))
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("{}", "Server ready.".green().bold());
    println!();

    axum::serve(listener, app).await?;

    Ok(())
}

// ── Route state and handlers ───────────────────────────────────────────

struct RouteState {
    router: Router,
    config: Config,
    http_client: reqwest::Client,
    event_store: EventStore,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: bool,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

async fn health() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok" }))
}

#[derive(serde::Deserialize)]
struct RouteQuery {
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    50
}

async fn recent_routes(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
    axum::extract::Query(query): axum::extract::Query<RouteQuery>,
) -> Result<axum::Json<Vec<switchyard_core::event::RouteEvent>>, axum::http::StatusCode> {
    state
        .event_store
        .recent_routes(query.limit)
        .map(axum::Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

async fn route_stats(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
) -> Result<axum::Json<switchyard_core::event::RouteStats>, axum::http::StatusCode> {
    state
        .event_store
        .stats()
        .map(axum::Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

async fn chat_completions(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
    axum::extract::Json(request): axum::extract::Json<ChatCompletionRequest>,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let prompt = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.as_deref())
        .unwrap_or("");

    let start = std::time::Instant::now();
    let route = state.router.route(prompt).map_err(|e| {
        tracing::error!("Routing failed: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    let backend = state
        .config
        .find_backend(&route.category)
        .or_else(|| state.config.fallback_backend().ok())
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(
        "Routed '{}' -> {} (score: {:.4}, backend: {}, latency: {:.1}ms)",
        &prompt[..prompt.len().min(60)],
        route.category,
        route.score,
        backend.name,
        latency_ms
    );

    // Log event to SQLite
    let _ = state.event_store.log_route(
        prompt,
        &route.category,
        route.score,
        route.is_fallback,
        &backend.name,
        &backend.model,
        Some(latency_ms),
        "ok",
        None,
    );

    // Forward to backend
    let url = format!("{}/v1/chat/completions", backend.base_url);
    let mut body = serde_json::to_value(&request)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    body["model"] = serde_json::Value::String(backend.model.clone());

    let mut req_builder = state.http_client.post(&url).json(&body);
    if let Some(ref api_key) = backend.api_key {
        req_builder = req_builder.bearer_auth(api_key);
    }

    let resp = req_builder.send().await.map_err(|e| {
        tracing::error!("Backend request failed: {}", e);
        let _ = state.event_store.log_route(
            prompt,
            &route.category,
            route.score,
            route.is_fallback,
            &backend.name,
            &backend.model,
            Some(latency_ms),
            "error",
            Some(&e.to_string()),
        );
        axum::http::StatusCode::BAD_GATEWAY
    })?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

    if !status.is_success() {
        let _ = state.event_store.log_route(
            prompt,
            &route.category,
            route.score,
            route.is_fallback,
            &backend.name,
            &backend.model,
            Some(latency_ms),
            "error",
            Some(&body),
        );
        return Err(
            axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        );
    }

    let mut response: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    response["switchyard_route"] = serde_json::json!({
        "category": route.category,
        "score": route.score,
        "is_fallback": route.is_fallback,
        "backend": backend.name,
    });

    Ok(axum::Json(response).into_response())
}

// ── CLI commands ───────────────────────────────────────────────────────

fn cmd_stats(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    let store = EventStore::new(std::path::Path::new(&config.dashboard.db_path))?;
    let stats = store.stats()?;

    println!("{}", "Switchyard Routing Statistics".bold().underline());
    println!();
    println!(
        "  {:<20} {}",
        "Total Routes:".dimmed(),
        stats.total_routes.to_string().bold()
    );
    println!(
        "  {:<20} {}",
        "Tool Call:".dimmed(),
        stats.tool_call_count.to_string().green()
    );
    println!(
        "  {:<20} {}",
        "General:".dimmed(),
        stats.general_count.to_string().green()
    );
    println!(
        "  {:<20} {}",
        "Fallback:".dimmed(),
        stats.fallback_count.to_string().yellow()
    );
    println!();
    println!(
        "  {:<20} {}",
        "Avg Latency:".dimmed(),
        format!("{:.1}ms", stats.avg_latency_ms).bold()
    );
    println!(
        "  {:<20} {}",
        "P50 Latency:".dimmed(),
        format!("{:.1}ms", stats.p50_latency_ms).bold()
    );
    println!(
        "  {:<20} {}",
        "P95 Latency:".dimmed(),
        format!("{:.1}ms", stats.p95_latency_ms).yellow()
    );
    println!();
    println!(
        "  {:<20} {}",
        "Avg Score:".dimmed(),
        format!("{:.4}", stats.avg_score).bold()
    );
    println!(
        "  {:<20} {}",
        "Routing Accuracy:".dimmed(),
        format!("{:.1}%", stats.accuracy_pct).bold()
    );

    Ok(())
}

fn cmd_routes(config_path: &PathBuf, limit: u32) -> Result<()> {
    let config = Config::load(config_path)?;
    let store = EventStore::new(std::path::Path::new(&config.dashboard.db_path))?;
    let routes = store.recent_routes(limit)?;

    if routes.is_empty() {
        println!("{}", "No route events recorded yet.".dimmed());
        return Ok(());
    }

    println!(
        "{}",
        format!("Recent Routes (last {})", routes.len())
            .bold()
            .underline()
    );
    println!();

    for r in &routes {
        let time = r.timestamp.format("%H:%M:%S").to_string();
        let category = if r.is_fallback {
            format!("{} (fb)", r.category).yellow()
        } else {
            match r.category.as_str() {
                "tool_call" => r.category.green(),
                _ => r.category.blue(),
            }
        };
        let score = format!("{:.4}", r.score).dimmed();
        let latency = r
            .latency_ms
            .map(|l| format!("{:.0}ms", l))
            .unwrap_or_else(|| "-".to_string());
        let status = if r.status == "ok" {
            "ok".green()
        } else {
            r.status.red()
        };

        let prompt = if r.prompt.len() > 50 {
            format!("{}...", &r.prompt[..47])
        } else {
            r.prompt.clone()
        };

        println!(
            "  {}  {:<20}  {:<20}  {:<10}  {:<8}  {:<15}  {}",
            time.dimmed(),
            prompt,
            category,
            score,
            r.backend,
            latency,
            status
        );
    }

    Ok(())
}

fn cmd_config_show(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;

    println!("{}", "Switchyard Configuration".bold().underline());
    println!();
    println!(
        "  {} {}:{}",
        "Server:".dimmed(),
        config.server.host,
        config.server.port
    );
    println!(
        "  {} {}",
        "Embedding Model:".dimmed(),
        config.router.embedding_model
    );
    println!(
        "  {} {}",
        "Threshold:".dimmed(),
        config.router.threshold.to_string()
    );
    println!("  {} {}", "Fallback:".dimmed(), config.router.fallback);
    println!();
    println!(
        "  {} {} capabilities",
        "Capabilities:".dimmed(),
        config.router.capabilities.len()
    );
    for cap in &config.router.capabilities {
        println!(
            "    - {} ({} examples)",
            cap.name.bold(),
            cap.examples.len()
        );
    }
    println!();
    println!(
        "  {} {} backends",
        "Backends:".dimmed(),
        config.backends.len()
    );
    for backend in &config.backends {
        println!(
            "    - {} -> {}:{} ({})",
            backend.name.bold(),
            backend.provider,
            backend.model,
            backend.base_url
        );
    }
    println!();
    println!("  {} {}", "DB Path:".dimmed(), config.dashboard.db_path);

    Ok(())
}
