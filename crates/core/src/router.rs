use anyhow::Result;
use tracing::info;

use crate::config::Config;
use crate::vector_store::VectorStore;

pub struct Router {
    vector_store: VectorStore,
    threshold: f32,
    fallback: String,
    k: usize,
}

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub category: String,
    pub score: f32,
    pub all_scores: Vec<(String, f32)>,
    pub is_fallback: bool,
}

impl Router {
    pub fn new(config: &Config, db_path: &std::path::Path) -> Result<Self> {
        info!(
            "Initializing k-NN router (model: {}, k: 5, threshold: {})",
            config.router.embedding_model, config.router.threshold
        );

        let vector_store =
            VectorStore::new(db_path, &config.router.embedding_model)?;

        let count = vector_store.count()?;
        info!("Router ready with {} examples in vector store", count);

        Ok(Self {
            vector_store,
            threshold: config.router.threshold,
            fallback: config.router.fallback.clone(),
            k: 5,
        })
    }

    pub fn route(&self, prompt: &str) -> Result<RouteResult> {
        let embedding = self.vector_store.embed(prompt)?;

        let neighbors = self.vector_store.search(&embedding, self.k)?;

        let (category, score, is_fallback) = self
            .vector_store
            .classify(&neighbors, self.threshold, &self.fallback);

        let all_scores = neighbors
            .iter()
            .map(|(cat, score)| (cat.clone(), *score))
            .collect();

        Ok(RouteResult {
            category,
            score,
            all_scores,
            is_fallback,
        })
    }

    /// Store a routing example for future k-NN classification.
    pub fn store_example(
        &mut self,
        prompt: &str,
        category: &str,
        confidence: f32,
    ) -> Result<()> {
        let embedding = self.vector_store.embed(prompt)?;
        self.vector_store
            .store(prompt, &embedding, category, confidence)?;
        Ok(())
    }

    pub fn example_count(&self) -> Result<usize> {
        self.vector_store.count()
    }
}
