use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::config::Config;
use crate::router::{RouteResult, Router as SwitchyardRouter};

/// Shared application state.
pub struct AppState {
    pub router: SwitchyardRouter,
    pub config: Config,
    pub http_client: Client,
}

// ── OpenAI-compatible request/response types ───────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switchyard_route: Option<RouteMetadata>,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct RouteMetadata {
    pub category: String,
    pub backend: String,
    pub score: f32,
    pub is_fallback: bool,
}

// ── Handlers ──────────────────────────────────────────────────────────

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/health", post(health))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, StatusCode> {
    let prompt = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.as_deref())
        .unwrap_or("");

    let route = state.router.route(prompt).map_err(|e| {
        error!("Routing failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Routed '{}' -> {} (score: {:.4}, fallback: {})",
        &prompt[..prompt.len().min(60)],
        route.category,
        route.score,
        route.is_fallback
    );

    let backend = state
        .config
        .find_backend(&route.category)
        .or_else(|| state.config.fallback_backend().ok())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(
        "Using backend: {} (provider: {}, model: {})",
        backend.name, backend.provider, backend.model
    );

    if request.stream {
        stream_forward(&state, backend, &request, &route).await
    } else {
        non_stream_forward(&state, backend, &request, &route).await
    }
}

async fn non_stream_forward(
    state: &Arc<AppState>,
    backend: &crate::config::Backend,
    request: &ChatCompletionRequest,
    route: &RouteResult,
) -> Result<Response, StatusCode> {
    let url = format!("{}/v1/chat/completions", backend.base_url);

    let mut body = serde_json::to_value(request).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    body["model"] = serde_json::Value::String(backend.model.clone());

    let mut req_builder = state.http_client.post(&url).json(&body);

    if let Some(ref api_key) = backend.api_key {
        req_builder = req_builder.bearer_auth(api_key);
    }

    let resp = req_builder.send().await.map_err(|e| {
        error!("Backend request failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let status = resp.status();
    let body = resp.text().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !status.is_success() {
        error!("Backend returned {}: {}", status, body);
        return Err(
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        );
    }

    let mut response: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    response["switchyard_route"] = serde_json::json!({
        "category": route.category,
        "score": route.score,
        "is_fallback": route.is_fallback,
    });

    Ok(Json(response).into_response())
}

async fn stream_forward(
    state: &Arc<AppState>,
    backend: &crate::config::Backend,
    request: &ChatCompletionRequest,
    _route: &RouteResult,
) -> Result<Response, StatusCode> {
    let url = format!("{}/v1/chat/completions", backend.base_url);

    let mut body = serde_json::to_value(request).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    body["model"] = serde_json::Value::String(backend.model.clone());
    body["stream"] = serde_json::Value::Bool(true);

    let mut req_builder = state.http_client.post(&url).json(&body);

    if let Some(ref api_key) = backend.api_key {
        req_builder = req_builder.bearer_auth(api_key);
    }

    let resp = req_builder.send().await.map_err(|e| {
        error!("Backend stream request failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        error!("Backend returned {}: {}", status, body);
        return Err(
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        );
    }

    // For streaming, we pass through the raw SSE from the backend.
    // Convert to axum SSE stream.
    use axum::response::sse::Event;
    use futures::StreamExt;

    let byte_stream = resp.bytes_stream();
    let sse_stream = byte_stream.map(|chunk| {
        chunk.map(|bytes| {
            let text = String::from_utf8_lossy(&bytes).to_string();
            Event::default().data(text)
        })
    });

    use axum::response::sse::Sse;
    Ok(Sse::new(sse_stream).into_response())
}
