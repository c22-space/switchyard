use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::classifier::Classifier;

pub struct Router {
    classifier: Classifier,
    threshold: f32,
    fallback: String,
}

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub category: String,
    pub score: f32,
    pub all_scores: Vec<(String, f32)>,
    pub is_fallback: bool,
}

impl Router {
    pub fn new(config: &crate::config::Config, _db_path: &Path) -> Result<Self> {
        let model_dir = Path::new("models/switchyard-router");
        let weights_path = model_dir.join("weights.bin");
        let labels_path = model_dir.join("labels.json");

        info!("Loading fine-tuned classifier from {}", model_dir.display());

        let classifier = Classifier::load(&weights_path, &labels_path)?;

        info!(
            "Router ready (threshold: {}, fallback: {})",
            config.router.threshold, config.router.fallback
        );

        Ok(Self {
            classifier,
            threshold: config.router.threshold,
            fallback: config.router.fallback.clone(),
        })
    }

    pub fn route(&self, _prompt: &str, embedding: &[f32]) -> Result<RouteResult> {
        let (category, confidence, all_scores) = self.classifier.classify(embedding);

        let is_fallback = confidence < self.threshold;

        let final_category = if is_fallback {
            self.fallback.clone()
        } else {
            category
        };

        Ok(RouteResult {
            category: final_category,
            score: confidence,
            all_scores,
            is_fallback,
        })
    }
}
