use anyhow::Result;
use axum::response::IntoResponse;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use std::sync::Arc;
use switchyard_core::config::Config;
use switchyard_core::event::EventStore;
use switchyard_core::router::Router;
use tower_http::services::ServeDir;
use switchyard_core::fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

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

    // Initialize event store
    let db_path = std::path::Path::new(&config.dashboard.db_path);
    let event_store = EventStore::new(db_path)?;

    // Initialize router with fine-tuned classifier
    let router = Router::new(&config, std::path::Path::new("."))?;

    // Initialize embedder
    let embedder = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::AllMiniLML6V2)
    )?;

    // Resolve .env path (same directory as config file)
    let env_path = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(".env");

    println!("{}", "Switchyard starting...".green().bold());
    println!(
        "  Routing server: {}:{}",
        config.server.host, config.server.port
    );
    println!();

    let state = Arc::new(RouteState {
        router,
        embedder: std::sync::Mutex::new(embedder),
        config: config.clone(),
        config_path: config_path.clone(),
        env_path,
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
        .route("/api/overview", axum::routing::get(overview))
        .route("/api/providers", axum::routing::get(list_providers))
        .route("/api/providers", axum::routing::post(add_provider))
        .route("/api/providers", axum::routing::put(update_provider))
        .route("/api/providers", axum::routing::delete(delete_provider))
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("{}", "Server ready.".green().bold());
    println!();

    // Serve dashboard static files from dist/ directory
    let dist_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dashboard-ui")
        .join("dist");
    let app = app.fallback_service(ServeDir::new(&dist_path).append_index_html_on_directories(true));

    axum::serve(listener, app).await?;

    Ok(())
}

// ── Route state and handlers ───────────────────────────────────────────

struct RouteState {
    router: Router,
    embedder: std::sync::Mutex<TextEmbedding>,
    config: Config,
    config_path: PathBuf,
    env_path: PathBuf,
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

async fn overview(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let stats = state.event_store.stats().map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let backends: Vec<serde_json::Value> = state.config.backends.iter().map(|b| {
        serde_json::json!({
            "name": b.name,
            "provider": b.provider,
            "model": b.model,
            "cost_per_1m_input_tokens": b.cost_per_1m_input_tokens,
            "cost_per_1m_output_tokens": b.cost_per_1m_output_tokens,
        })
    }).collect();

    Ok(axum::Json(serde_json::json!({
        "stats": stats,
        "backends": backends,
        "capabilities": state.config.router.capabilities.len(),
        "embedding_model": state.config.router.embedding_model,
        "threshold": state.config.router.threshold,
        "fallback": state.config.router.fallback,
    })))
}

// ── .env key management ────────────────────────────────────────────────

/// Read all keys from .env file as key-value pairs.
fn read_env_keys(path: &std::path::Path) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if !path.exists() {
        return map;
    }
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    map
}

/// Write all key-value pairs back to .env file.
fn write_env_keys(path: &std::path::Path, keys: &std::collections::HashMap<String, String>) -> Result<()> {
    let mut lines: Vec<String> = Vec::new();
    for (k, v) in keys {
        lines.push(format!("{}={}", k, v));
    }
    lines.sort();
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Get the env var name for a backend's API key.
fn env_key_name(backend_name: &str) -> String {
    format!("{}_KEY", backend_name.to_uppercase().replace('-', "_"))
}

/// Mask a key for display: show first 3 and last 4 chars.
fn mask_key(key: &str) -> String {
    if key.len() <= 7 {
        return "*".repeat(key.len());
    }
    format!("{}...{}", &key[..3], &key[key.len() - 4..])
}

/// Load API key for a backend from .env.
fn load_backend_key(env_path: &std::path::Path, backend_name: &str) -> Option<String> {
    let keys = read_env_keys(env_path);
    keys.get(&env_key_name(backend_name)).cloned()
}

/// Save API key for a backend to .env.
fn save_backend_key(env_path: &std::path::Path, backend_name: &str, api_key: &str) -> Result<()> {
    let mut keys = read_env_keys(env_path);
    keys.insert(env_key_name(backend_name), api_key.to_string());
    write_env_keys(env_path, &keys)
}

/// Remove API key for a backend from .env.
fn remove_backend_key(env_path: &std::path::Path, backend_name: &str) -> Result<()> {
    let mut keys = read_env_keys(env_path);
    keys.remove(&env_key_name(backend_name));
    write_env_keys(env_path, &keys)
}

// ── Provider API handlers ──────────────────────────────────────────────

async fn list_providers(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
) -> axum::Json<Vec<serde_json::Value>> {
    let providers: Vec<serde_json::Value> = state.config.backends.iter().map(|b| {
        let masked = load_backend_key(&state.env_path, &b.name)
            .map(|k| mask_key(&k))
            .unwrap_or_else(|| "no key".to_string());
        serde_json::json!({
            "name": b.name,
            "provider": b.provider,
            "model": b.model,
            "base_url": b.base_url,
            "api_key_masked": masked,
        })
    }).collect();
    axum::Json(providers)
}

async fn add_provider(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
    axum::extract::Json(backend): axum::extract::Json<switchyard_core::config::Backend>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    // Save API key to .env if provided
    if let Some(ref key) = backend.api_key {
        if !key.is_empty() {
            save_backend_key(&state.env_path, &backend.name, key)
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    let mut config = state.config.clone();
    // Save backend without the key (key lives in .env)
    let mut backend_clean = backend.clone();
    backend_clean.api_key = None;
    config.backends.push(backend_clean);
    Config::save(&state.config_path, &config)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::Json(serde_json::json!({
        "ok": true,
        "provider": {
            "name": backend.name,
            "provider": backend.provider,
            "model": backend.model,
            "base_url": backend.base_url,
        }
    })))
}

#[derive(serde::Deserialize)]
struct ProviderIndex {
    index: usize,
}

#[derive(serde::Deserialize)]
struct UpdateProvider {
    index: usize,
    name: String,
    provider: String,
    base_url: String,
    model: String,
    /// If Some and non-empty, update the key. If None or empty, keep existing.
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    cost_per_1m_input_tokens: f64,
    #[serde(default)]
    cost_per_1m_output_tokens: f64,
}

async fn update_provider(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
    axum::extract::Json(payload): axum::extract::Json<UpdateProvider>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let mut config = state.config.clone();
    if payload.index >= config.backends.len() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let old_name = config.backends[payload.index].name.clone();

    // Update the key in .env if a new one was provided
    if let Some(ref key) = payload.api_key {
        if !key.is_empty() {
            // If name changed, remove old key and save under new name
            if old_name != payload.name {
                let _ = remove_backend_key(&state.env_path, &old_name);
            }
            save_backend_key(&state.env_path, &payload.name, key)
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    config.backends[payload.index] = switchyard_core::config::Backend {
        name: payload.name.clone(),
        provider: payload.provider.clone(),
        base_url: payload.base_url.clone(),
        api_key: None, // Key lives in .env
        model: payload.model.clone(),
        cost_per_1m_input_tokens: payload.cost_per_1m_input_tokens,
        cost_per_1m_output_tokens: payload.cost_per_1m_output_tokens,
    };
    Config::save(&state.config_path, &config)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::Json(serde_json::json!({
        "ok": true,
        "provider": {
            "name": payload.name,
            "provider": payload.provider,
            "model": payload.model,
            "base_url": payload.base_url,
        }
    })))
}

async fn delete_provider(
    axum::extract::State(state): axum::extract::State<Arc<RouteState>>,
    axum::extract::Json(payload): axum::extract::Json<ProviderIndex>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let mut config = state.config.clone();
    if payload.index >= config.backends.len() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    let removed = config.backends.remove(payload.index);
    // Remove key from .env
    let _ = remove_backend_key(&state.env_path, &removed.name);
    Config::save(&state.config_path, &config)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::Json(serde_json::json!({
        "ok": true,
        "removed": removed.name,
    })))
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
    let embedding = {
        let embedder = state.embedder.lock().unwrap();
        let embeddings = embedder.embed(vec![prompt], None).map_err(|e| {
            tracing::error!("Embedding failed: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
        embeddings.into_iter().next().unwrap_or_default()
    };
    let route = state.router.route(prompt, &embedding).map_err(|e| {
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

    // Load API key from .env
    let api_key = load_backend_key(&state.env_path, &backend.name);

    // Forward to backend
    let url = backend.base_url.clone();
    let mut body = serde_json::to_value(&request)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    body["model"] = serde_json::Value::String(backend.model.clone());

    let mut req_builder = state.http_client.post(&url).json(&body);
    if let Some(ref key) = api_key {
        req_builder = req_builder.bearer_auth(key);
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
            None,
            None,
            None,
        );
        axum::http::StatusCode::BAD_GATEWAY
    })?;

    let status = resp.status();

    if !status.is_success() {
        let body = resp
            .text()
            .await
            .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;
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
            None,
            None,
            None,
        );
        return Err(
            axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        );
    }

    // Streaming response - forward raw bytes from upstream (already SSE-formatted)
    if request.stream {
        use futures_util::StreamExt;
        let stream = resp.bytes_stream();
        let body = axum::body::Body::from_stream(stream.map(|result| {
            result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }));
        // Log streaming route (no usage data available in SSE stream)
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
            None,
            None,
            None,
        );

        let response = axum::response::Response::builder()
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .body(body)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(response)
    } else {
        // Non-streaming response
        let body = resp
            .text()
            .await
            .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

        let mut response: serde_json::Value =
            serde_json::from_str(&body).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        // Parse usage from response
        let input_tokens = response
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_i64());
        let output_tokens = response
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_i64());

        // Compute estimated cost
        let estimated_cost = match (input_tokens, output_tokens) {
            (Some(inp), Some(out)) => {
                let cost = (inp as f64 * backend.cost_per_1m_input_tokens
                    + out as f64 * backend.cost_per_1m_output_tokens)
                    / 1_000_000.0;
                Some(cost)
            }
            _ => None,
        };

        // Log with usage data
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
            input_tokens,
            output_tokens,
            estimated_cost,
        );

        response["switchyard_route"] = serde_json::json!({
            "category": route.category,
            "score": route.score,
            "is_fallback": route.is_fallback,
            "backend": backend.name,
            "estimated_cost": estimated_cost,
        });

        Ok(axum::Json(response).into_response())
    }
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

    if stats.total_input_tokens > 0 || stats.total_output_tokens > 0 {
        println!();
        println!(
            "  {:<20} {}",
            "Input Tokens:".dimmed(),
            stats.total_input_tokens.to_string().bold()
        );
        println!(
            "  {:<20} {}",
            "Output Tokens:".dimmed(),
            stats.total_output_tokens.to_string().bold()
        );
        println!(
            "  {:<20} {}",
            "Estimated Cost:".dimmed(),
            format!("${:.6}", stats.total_cost_usd).green().bold()
        );
    }

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
            "    - {}",
            cap.name.bold(),
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
