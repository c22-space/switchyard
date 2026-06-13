use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use qdrant_edge::{
    CountRequest, Distance, EdgeConfigBuilder, EdgeShard, EdgeVectorParamsBuilder, NamedQuery,
    Payload, PointId, PointInsertOperations, PointOperations, PointStructPersisted, QueryEnum,
    QueryRequest, ScoringQuery, ScrollRequest, UpdateOperation, VectorInternal,
    VectorPersisted, VectorStructPersisted, WithPayloadInterface,
};
use serde_json::{json, Map};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

const VECTOR_NAME: &str = "embedding";
const VECTOR_DIM: usize = 384; // all-MiniLM-L6-v2
const MAX_EXAMPLES: usize = 10_000;
const SEED_CONFIDENCE: f32 = 1.0;

/// Seed examples for cold start. Used only when the vector store is empty.
fn seed_examples() -> Vec<(&'static str, &'static str)> {
    vec![
        // ── tool_call: imperative, action-oriented queries ──
        ("search the web for latest news", "tool_call"),
        ("get the current weather in New York", "tool_call"),
        ("send an email to john@example.com", "tool_call"),
        ("check if the server is running on port 8080", "tool_call"),
        ("look up the stock price of Apple", "tool_call"),
        ("is the server up?", "tool_call"),
        ("what processes are using the most memory?", "tool_call"),
        ("how much diskspace is free?", "tool_call"),
        ("fetch data from the REST API endpoint", "tool_call"),
        ("query the database for all active users", "tool_call"),
        ("check the status of my order", "tool_call"),
        ("monitor the CPU usage of the server", "tool_call"),
        ("what's running on port 8080?", "tool_call"),
        // ── general: reasoning, analysis, meta-discussion ──
        ("here are my thoughts on the routing approach", "general"),
        ("how can we optimize this for budget?", "general"),
        ("I think the caching strategy should be different", "general"),
        ("what do you think about adding prompt compression?", "general"),
        ("that categorised as tool_call doesn't make sense", "general"),
        ("let me explain why I think this approach is wrong", "general"),
        ("can you list the pros and cons of using qdrant here?", "general"),
        ("what's the capital of France?", "general"),
        ("tell me a fun fact about space", "general"),
        ("write a Python function to sort a list", "general"),
        ("explain quantum physics simply", "general"),
        ("tell me a joke", "general"),
        ("compare the pros and cons of React vs Vue", "general"),
        ("how do I make good coffee?", "general"),
        ("why did my query go to fallback?", "general"),
    ]
}

pub struct VectorStore {
    shard: EdgeShard,
    model: TextEmbedding,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl VectorStore {
    pub fn new(db_path: &Path, embedding_model: &str) -> Result<Self> {
        let shard_dir = db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("vector_store");

        let model_name = match embedding_model {
            "all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,
            "bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            "bge-large-en-v1.5" => EmbeddingModel::BGELargeENV15,
            "nomic-embed-text" => EmbeddingModel::NomicEmbedTextV1,
            _ => {
                info!(
                    "Unknown model '{}', falling back to all-MiniLM-L6-v2",
                    embedding_model
                );
                EmbeddingModel::AllMiniLML6V2
            }
        };

        let model = TextEmbedding::try_new(InitOptions::new(model_name))
            .context("Failed to initialize embedding model")?;

        let config = EdgeConfigBuilder::new()
            .vector(
                VECTOR_NAME,
                EdgeVectorParamsBuilder::new(VECTOR_DIM, Distance::Cosine).build(),
            )
            .build();

        let shard = if shard_dir.exists() {
            info!("Loading existing vector store from {:?}", shard_dir);
            EdgeShard::load(&shard_dir, Some(config))
                .context("Failed to load vector store")?
        } else {
            info!("Creating new vector store at {:?}", shard_dir);
            std::fs::create_dir_all(&shard_dir)?;
            EdgeShard::new(&shard_dir, config).context("Failed to create vector store")?
        };

        let mut store = Self {
            shard,
            model,
            db_path: db_path.to_path_buf(),
        };

        // Seed if empty
        let count = store.count()?;
        if count == 0 {
            info!("Vector store is empty, inserting seed examples...");
            store.seed()?;
        } else {
            info!("Vector store has {} examples", count);
        }

        Ok(store)
    }

    pub fn count(&self) -> Result<usize> {
        let req = CountRequest {
            filter: None,
            exact: true,
        };
        let count = self.shard.count(req).context("Failed to count points")?;
        Ok(count)
    }

    /// Embed a text string into a vector.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self
            .model
            .embed(vec![text.to_string()], None)
            .context("Failed to embed text")?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    /// k-NN search: find the k nearest examples and return (category, score) pairs.
    pub fn search(&self, embedding: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let query_enum = QueryEnum::Nearest(NamedQuery {
            query: VectorInternal::Dense(embedding.to_vec()),
            using: Some(VECTOR_NAME.to_string().into()),
        });

        let req = QueryRequest {
            prefetches: vec![],
            query: Some(ScoringQuery::Vector(query_enum)),
            filter: None,
            score_threshold: None,
            limit: k,
            offset: 0,
            params: None,
            with_vector: false.into(),
            with_payload: WithPayloadInterface::Bool(true),
        };

        let results = self.shard.query(req).context("Failed to query vector store")?;

        let mut neighbors = Vec::new();
        for hit in results {
            let category = hit
                .payload
                .as_ref()
                .and_then(|p| p.0.get("category"))
                .and_then(|v| v.as_str())
                .unwrap_or("general")
                .to_string();
            let score = hit.score;
            neighbors.push((category, score));
        }

        Ok(neighbors)
    }

    /// Store a new example in the vector store.
    pub fn store(
        &mut self,
        text: &str,
        embedding: &[f32],
        category: &str,
        confidence: f32,
    ) -> Result<()> {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut payload_map = Map::new();
        payload_map.insert("prompt".to_string(), json!(text));
        payload_map.insert("category".to_string(), json!(category));
        payload_map.insert("confidence".to_string(), json!(confidence));

        let point = PointStructPersisted {
            id: PointId::NumId(id),
            vector: {
                let mut map = HashMap::new();
                map.insert(
                    VECTOR_NAME.to_string().into(),
                    VectorPersisted::Dense(embedding.to_vec()),
                );
                VectorStructPersisted::Named(map)
            },
            payload: Some(Payload(payload_map)),
        };

        let op = UpdateOperation::PointOperation(PointOperations::UpsertPoints(
            PointInsertOperations::PointsList(vec![point]),
        ));

        self.shard.update(op).context("Failed to upsert point")?;

        // Prune if over limit
        self.prune_if_needed()?;

        Ok(())
    }

    /// Majority vote from k-NN neighbors. Returns (category, score, is_fallback).
    pub fn classify(
        &self,
        neighbors: &[(String, f32)],
        threshold: f32,
        fallback: &str,
    ) -> (String, f32, bool) {
        if neighbors.is_empty() {
            return (fallback.to_string(), 0.0, true);
        }

        let best_score = neighbors[0].1;

        if best_score < threshold {
            return (fallback.to_string(), best_score, true);
        }

        // Count votes by category
        let mut votes: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        for (category, score) in neighbors {
            *votes.entry(category.clone()).or_insert(0.0) += score;
        }

        // Find category with highest total score
        let (best_cat, _) = votes
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        (best_cat.clone(), best_score, false)
    }

    /// Prune low-confidence examples when over MAX_EXAMPLES.
    fn prune_if_needed(&mut self) -> Result<()> {
        let count = self.count()?;
        if count <= MAX_EXAMPLES {
            return Ok(());
        }

        info!(
            "Vector store has {} examples (max {}), pruning low-confidence...",
            count, MAX_EXAMPLES
        );

        let req = ScrollRequest {
            offset: None,
            limit: Some(1000),
            filter: None,
            with_payload: Some(WithPayloadInterface::Bool(true)),
            with_vector: false.into(),
            order_by: None,
        };

        let (records, _) = self.shard.scroll(req).context("Failed to scroll points")?;

        // Collect low-confidence point IDs
        let mut low_confidence_ids: Vec<PointId> = Vec::new();
        for record in &records {
            let confidence = record
                .payload
                .as_ref()
                .and_then(|p| p.0.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32;

            // Only prune low confidence (< 0.6)
            if confidence < 0.6 {
                low_confidence_ids.push(record.id);
            }
        }

        // Delete only what we need to get back to MAX_EXAMPLES
        let excess = count - MAX_EXAMPLES;
        low_confidence_ids.truncate(excess);

        if !low_confidence_ids.is_empty() {
            let op = UpdateOperation::PointOperation(PointOperations::DeletePoints {
                ids: low_confidence_ids.clone(),
            });
            self.shard.update(op).context("Failed to delete points")?;
            info!(
                "Pruned {} low-confidence examples",
                low_confidence_ids.len()
            );
        }

        Ok(())
    }

    /// Insert seed examples for cold start.
    fn seed(&mut self) -> Result<()> {
        let seeds = seed_examples();
        let texts: Vec<String> = seeds.iter().map(|(t, _)| t.to_string()).collect();
        let embeddings = self
            .model
            .embed(texts, None)
            .context("Failed to embed seed examples")?;

        let mut points = Vec::new();
        for (i, ((text, category), embedding)) in seeds.iter().zip(embeddings.iter()).enumerate() {
            let mut payload_map = Map::new();
            payload_map.insert("prompt".to_string(), json!(text));
            payload_map.insert("category".to_string(), json!(category));
            payload_map.insert("confidence".to_string(), json!(SEED_CONFIDENCE));

            points.push(PointStructPersisted {
                id: PointId::NumId((i + 1) as u64),
                vector: {
                    let mut map = HashMap::new();
                    map.insert(
                        VECTOR_NAME.to_string().into(),
                        VectorPersisted::Dense(embedding.to_vec()),
                    );
                    VectorStructPersisted::Named(map)
                },
                payload: Some(Payload(payload_map)),
            });
        }

        let op = UpdateOperation::PointOperation(PointOperations::UpsertPoints(
            PointInsertOperations::PointsList(points),
        ));

        self.shard.update(op).context("Failed to upsert seed points")?;

        info!("Inserted {} seed examples", seeds.len());
        Ok(())
    }
}
