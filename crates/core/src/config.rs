use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub router: RouterConfig,
    pub backends: Vec<Backend>,
    #[serde(default)]
    pub dashboard: DashboardConfig,
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
    pub embedding_model: String,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    pub fallback: String,
    pub capabilities: Vec<Capability>,
}

fn default_threshold() -> f32 {
    0.25
}

#[derive(Debug, Deserialize, Clone)]
pub struct Capability {
    pub name: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Backend {
    pub name: String,
    pub provider: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DashboardConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_dashboard_port")]
    pub port: u16,
}

fn default_enabled() -> bool {
    true
}

fn default_db_path() -> String {
    "switchyard.db".to_string()
}

fn default_dashboard_port() -> u16 {
    8421
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            db_path: default_db_path(),
            port: default_dashboard_port(),
        }
    }
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
