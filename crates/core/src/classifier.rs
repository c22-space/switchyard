use anyhow::{Context, Result};
use std::path::Path;

/// Neural network classification head for routing.
/// Loads pre-trained weights from a binary file and classifies embeddings.
pub struct Classifier {
    weights: Vec<f32>,
    bias1: Vec<f32>,
    weights2: Vec<f32>,
    bias2: Vec<f32>,
    labels: Vec<String>,
    embedding_dim: usize,
    hidden_dim: usize,
    num_labels: usize,
}

impl Classifier {
    /// Load classifier weights from a binary file.
    ///
    /// Binary format:
    /// - Header: 3 x i32 (num_labels, embedding_dim, hidden_dim)
    /// - weights1: embedding_dim x hidden_dim f32s (row-major)
    /// - bias1: hidden_dim f32s
    /// - weights2: hidden_dim x num_labels f32s (row-major)
    /// - bias2: num_labels f32s
    pub fn load(weights_path: &Path, labels_path: &Path) -> Result<Self> {
        let data = std::fs::read(weights_path)
            .with_context(|| format!("Failed to read weights from {}", weights_path.display()))?;

        if data.len() < 12 {
            anyhow::bail!("Weights file too small for header");
        }

        // Read header
        let num_labels = i32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let embedding_dim = i32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let hidden_dim = i32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

        let w1_size = embedding_dim * hidden_dim;
        let b1_size = hidden_dim;
        let w2_size = hidden_dim * num_labels;
        let b2_size = num_labels;
        let expected = 12 + (w1_size + b1_size + w2_size + b2_size) * 4;

        if data.len() < expected {
            anyhow::bail!(
                "Weights file truncated: expected {} bytes, got {}",
                expected,
                data.len()
            );
        }

        let offset = 12;
        let weights = read_f32s(&data, offset, w1_size);
        let bias1 = read_f32s(&data, offset + w1_size * 4, b1_size);
        let weights2 = read_f32s(&data, offset + (w1_size + b1_size) * 4, w2_size);
        let bias2 = read_f32s(&data, offset + (w1_size + b1_size + w2_size) * 4, b2_size);

        // Load labels
        let labels_data = std::fs::read_to_string(labels_path)
            .with_context(|| format!("Failed to read labels from {}", labels_path.display()))?;
        let labels_json: serde_json::Value = serde_json::from_str(&labels_data)?;
        let id2label = labels_json["id2label"]
            .as_object()
            .context("Missing id2label in labels.json")?;

        let mut labels = vec![String::new(); num_labels];
        for (k, v) in id2label {
            let idx: usize = k.parse().context("Invalid label index")?;
            if idx < num_labels {
                labels[idx] = v.as_str().unwrap_or("unknown").to_string();
            }
        }

        tracing::info!(
            "Classifier loaded: {} -> {} -> {} ({} labels)",
            embedding_dim,
            hidden_dim,
            num_labels,
            num_labels
        );

        Ok(Self {
            weights,
            bias1,
            weights2,
            bias2,
            labels,
            embedding_dim,
            hidden_dim,
            num_labels,
        })
    }

    /// Classify an embedding vector. Returns (category, confidence, all_scores).
    pub fn classify(&self, embedding: &[f32]) -> (String, f32, Vec<(String, f32)>) {
        assert_eq!(
            embedding.len(),
            self.embedding_dim,
            "Embedding dimension mismatch: expected {}, got {}",
            self.embedding_dim,
            embedding.len()
        );

        // Layer 1: embedding -> hidden (ReLU)
        let mut hidden = vec![0.0f32; self.hidden_dim];
        for j in 0..self.hidden_dim {
            let mut sum = self.bias1[j];
            for i in 0..self.embedding_dim {
                sum += embedding[i] * self.weights[j * self.embedding_dim + i];
            }
            hidden[j] = sum.max(0.0); // ReLU
        }

        // Layer 2: hidden -> logits
        let mut logits = vec![0.0f32; self.num_labels];
        for k in 0..self.num_labels {
            let mut sum = self.bias2[k];
            for j in 0..self.hidden_dim {
                sum += hidden[j] * self.weights2[k * self.hidden_dim + j];
            }
            logits[k] = sum;
        }

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = logits.iter().map(|&l| (l - max_logit).exp()).sum();
        let probs: Vec<f32> = logits
            .iter()
            .map(|&l| (l - max_logit).exp() / exp_sum)
            .collect();

        // Find best
        let best_idx = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let all_scores: Vec<(String, f32)> = self
            .labels
            .iter()
            .zip(probs.iter())
            .map(|(label, prob)| (label.clone(), *prob))
            .collect();

        (
            self.labels[best_idx].clone(),
            probs[best_idx],
            all_scores,
        )
    }
}

fn read_f32s(data: &[u8], offset: usize, count: usize) -> Vec<f32> {
    data[offset..offset + count * 4]
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
