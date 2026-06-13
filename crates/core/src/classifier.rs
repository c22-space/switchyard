use anyhow::{Context, Result};
use std::path::Path;

/// Multi-task classification head: shared encoder + category + complexity heads.
pub struct Classifier {
    // Shared layer: embedding_dim -> hidden_dim
    shared_w: Vec<f32>,
    shared_b: Vec<f32>,
    // Category head: hidden_dim -> num_labels
    cat_w: Vec<f32>,
    cat_b: Vec<f32>,
    // Complexity head: hidden_dim -> num_complexities
    cmp_w: Vec<f32>,
    cmp_b: Vec<f32>,
    labels: Vec<String>,
    complexities: Vec<String>,
    embedding_dim: usize,
    hidden_dim: usize,
    num_labels: usize,
    num_complexities: usize,
}

#[derive(Debug, Clone)]
pub struct ClassifyResult {
    pub category: String,
    pub category_confidence: f32,
    pub complexity: String,
    pub complexity_confidence: f32,
    pub all_category_scores: Vec<(String, f32)>,
}

impl Classifier {
    /// Load multi-task weights from binary file.
    ///
    /// Format:
    /// - Header: 4 x i32 (num_labels, embedding_dim, hidden_dim, num_complexities)
    /// - shared_w: embedding_dim x hidden_dim f32s
    /// - shared_b: hidden_dim f32s
    /// - cat_w: hidden_dim x num_labels f32s
    /// - cat_b: num_labels f32s
    /// - cmp_w: hidden_dim x num_complexities f32s
    /// - cmp_b: num_complexities f32s
    pub fn load(weights_path: &Path, labels_path: &Path) -> Result<Self> {
        let data = std::fs::read(weights_path)
            .with_context(|| format!("Failed to read weights from {}", weights_path.display()))?;

        if data.len() < 16 {
            anyhow::bail!("Weights file too small for header");
        }

        let num_labels = i32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let embedding_dim = i32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let hidden_dim = i32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let num_complexities = i32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;

        let sw_size = embedding_dim * hidden_dim;
        let sb_size = hidden_dim;
        let cw_size = hidden_dim * num_labels;
        let cb_size = num_labels;
        let xw_size = hidden_dim * num_complexities;
        let xb_size = num_complexities;
        let expected = 16 + (sw_size + sb_size + cw_size + cb_size + xw_size + xb_size) * 4;

        if data.len() < expected {
            anyhow::bail!(
                "Weights file truncated: expected {} bytes, got {}",
                expected,
                data.len()
            );
        }

        let mut offset = 16;
        let shared_w = read_f32s(&data, offset, sw_size); offset += sw_size * 4;
        let shared_b = read_f32s(&data, offset, sb_size); offset += sb_size * 4;
        let cat_w = read_f32s(&data, offset, cw_size); offset += cw_size * 4;
        let cat_b = read_f32s(&data, offset, cb_size); offset += cb_size * 4;
        let cmp_w = read_f32s(&data, offset, xw_size); offset += xw_size * 4;
        let cmp_b = read_f32s(&data, offset, xb_size); offset += xb_size * 4;

        // Load labels
        let labels_data = std::fs::read_to_string(labels_path)
            .with_context(|| format!("Failed to read labels from {}", labels_path.display()))?;
        let labels_json: serde_json::Value = serde_json::from_str(&labels_data)?;

        let id2label = labels_json["id2label"]
            .as_object()
            .context("Missing id2label")?;
        let mut labels = vec![String::new(); num_labels];
        for (k, v) in id2label {
            let idx: usize = k.parse().context("Invalid label index")?;
            if idx < num_labels {
                labels[idx] = v.as_str().unwrap_or("unknown").to_string();
            }
        }

        let complexities = if let Some(id2cmp) = labels_json["id2complexity"].as_object() {
            let mut c = vec![String::new(); num_complexities];
            for (k, v) in id2cmp {
                let idx: usize = k.parse().unwrap_or(0);
                if idx < num_complexities {
                    c[idx] = v.as_str().unwrap_or("unknown").to_string();
                }
            }
            c
        } else {
            vec!["low".into(), "medium".into(), "high".into()]
        };

        tracing::info!(
            "Classifier loaded: {} -> {} -> ({} categories, {} complexities)",
            embedding_dim, hidden_dim, num_labels, num_complexities
        );

        Ok(Self {
            shared_w, shared_b,
            cat_w, cat_b,
            cmp_w, cmp_b,
            labels, complexities,
            embedding_dim, hidden_dim,
            num_labels, num_complexities,
        })
    }

    /// Classify an embedding. Returns category, complexity, and scores.
    pub fn classify(&self, embedding: &[f32]) -> ClassifyResult {
        assert_eq!(embedding.len(), self.embedding_dim);

        // Shared layer: embedding -> hidden (ReLU)
        let mut hidden = vec![0.0f32; self.hidden_dim];
        for j in 0..self.hidden_dim {
            let mut sum = self.shared_b[j];
            for i in 0..self.embedding_dim {
                sum += embedding[i] * self.shared_w[j * self.embedding_dim + i];
            }
            hidden[j] = sum.max(0.0);
        }

        // Category head
        let cat_logits = linear(&hidden, &self.cat_w, &self.cat_b, self.num_labels);
        let cat_probs = softmax(&cat_logits);
        let (cat_idx, cat_conf) = argmax(&cat_probs);

        // Complexity head
        let cmp_logits = linear(&hidden, &self.cmp_w, &self.cmp_b, self.num_complexities);
        let cmp_probs = softmax(&cmp_logits);
        let (cmp_idx, cmp_conf) = argmax(&cmp_probs);

        let all_category_scores: Vec<(String, f32)> = self
            .labels
            .iter()
            .zip(cat_probs.iter())
            .map(|(label, prob)| (label.clone(), *prob))
            .collect();

        ClassifyResult {
            category: self.labels[cat_idx].clone(),
            category_confidence: cat_conf,
            complexity: self.complexities[cmp_idx].clone(),
            complexity_confidence: cmp_conf,
            all_category_scores,
        }
    }
}

fn linear(input: &[f32], weights: &[f32], bias: &[f32], out_dim: usize) -> Vec<f32> {
    let in_dim = input.len();
    let mut output = vec![0.0f32; out_dim];
    for j in 0..out_dim {
        let mut sum = bias[j];
        for i in 0..in_dim {
            sum += input[i] * weights[j * in_dim + i];
        }
        output[j] = sum;
    }
    output
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f32 = logits.iter().map(|&l| (l - max_logit).exp()).sum();
    logits.iter().map(|&l| (l - max_logit).exp() / exp_sum).collect()
}

fn argmax(probs: &[f32]) -> (usize, f32) {
    probs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, &p)| (i, p))
        .unwrap_or((0, 0.0))
}

fn read_f32s(data: &[u8], offset: usize, count: usize) -> Vec<f32> {
    data[offset..offset + count * 4]
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
