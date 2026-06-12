use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub router: RouterConfig,
    pub backends: Vec<Backend>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    8420
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
    /// Embedding model name (fastembed compatible)
    pub embedding_model: String,

    /// Routing threshold - below this, use fallback
    #[serde(default = "default_threshold")]
    pub threshold: f32,

    /// Fallback backend name
    pub fallback: String,

    /// Capability definitions with example prompts
    pub capabilities: Vec<Capability>,
}

fn default_threshold() -> f32 {
    0.25
}

#[derive(Debug, Deserialize, Clone)]
pub struct Capability {
    /// Category name (e.g., "tool_call", "general")
    pub name: String,

    /// Example prompts that define this capability's centroid
    pub examples: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Backend {
    /// Backend name (matches capability names or "fallback")
    pub name: String,

    /// Provider type: "openai", "ollama", "openrouter"
    pub provider: String,

    /// API base URL
    pub base_url: String,

    /// API key (optional, for cloud providers)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name to use
    pub model: String,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn find_backend(&self, name: &str) -> Option<&Backend> {
        self.backends.iter().find(|b| b.name == name)
    }

    pub fn fallback_backend(&self) -> anyhow::Result<&Backend> {
        self.find_backend(&self.router.fallback)
            .ok_or_else(|| anyhow::anyhow!("fallback backend '{}' not found", self.router.fallback))
    }
}
