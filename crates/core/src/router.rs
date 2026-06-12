use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tracing::info;

use crate::config::{Capability, Config};

pub struct Router {
    model: TextEmbedding,
    centroids: Vec<(String, Vec<f32>)>,
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
    pub fn new(config: &Config) -> Result<Self> {
        info!("Loading embedding model: {}", config.router.embedding_model);

        let model_name = match config.router.embedding_model.as_str() {
            "all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,
            "bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            "bge-large-en-v1.5" => EmbeddingModel::BGELargeENV15,
            "nomic-embed-text" => EmbeddingModel::NomicEmbedTextV1,
            _ => {
                info!(
                    "Unknown model '{}', falling back to all-MiniLM-L6-v2",
                    config.router.embedding_model
                );
                EmbeddingModel::AllMiniLML6V2
            }
        };

        let model = TextEmbedding::try_new(InitOptions::new(model_name))?;

        info!(
            "Computing centroids for {} capabilities",
            config.router.capabilities.len()
        );
        let centroids = Self::compute_centroids(&model, &config.router.capabilities)?;

        for (i, (name_a, _)) in centroids.iter().enumerate() {
            for (j, (name_b, _)) in centroids.iter().enumerate() {
                if i < j {
                    let sim = cosine_similarity(&centroids[i].1, &centroids[j].1);
                    info!(
                        "  centroid similarity: {} <-> {} = {:.4}",
                        name_a, name_b, sim
                    );
                }
            }
        }

        Ok(Self {
            model,
            centroids,
            threshold: config.router.threshold,
            fallback: config.router.fallback.clone(),
        })
    }

    fn compute_centroids(
        model: &TextEmbedding,
        capabilities: &[Capability],
    ) -> Result<Vec<(String, Vec<f32>)>> {
        let mut centroids = Vec::new();

        for cap in capabilities {
            let embeddings = model.embed(cap.examples.clone(), None)?;
            let dim = embeddings[0].len();
            let mut centroid = vec![0.0f32; dim];
            for emb in &embeddings {
                for (i, &val) in emb.iter().enumerate() {
                    centroid[i] += val;
                }
            }
            let n = embeddings.len() as f32;
            for val in &mut centroid {
                *val /= n;
            }
            let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut centroid {
                    *val /= norm;
                }
            }
            centroids.push((cap.name.clone(), centroid));
        }

        Ok(centroids)
    }

    pub fn route(&self, prompt: &str) -> Result<RouteResult> {
        let query_str: String = prompt.to_string();
        let embeddings = self.model.embed(vec![query_str], None)?;
        let query_emb = &embeddings[0];

        let mut scores: Vec<(String, f32)> = self
            .centroids
            .iter()
            .map(|(name, centroid)| {
                let sim = cosine_similarity(query_emb, centroid);
                (name.clone(), sim)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let (best_cat, best_score) = scores[0].clone();
        let is_fallback = best_score < self.threshold;

        let category = if is_fallback {
            self.fallback.clone()
        } else {
            best_cat
        };

        Ok(RouteResult {
            category,
            score: best_score,
            all_scores: scores,
            is_fallback,
        })
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
