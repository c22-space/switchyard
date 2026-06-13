#!/usr/bin/env python3
"""
Train multi-task classifier: category (13) + complexity (3)
Frozen encoder approach - only trains classification heads.
Uses proper train/val split and early stopping to prevent overfitting.
"""
import json
import os
import random
import struct
import time
from pathlib import Path

import numpy as np

# ── Config ──────────────────────────────────────────────────────────────
EMBEDDING_MODEL = "sentence-transformers/all-MiniLM-L6-v2"
EMBEDDING_DIM = 384
HIDDEN_DIM = 256
NUM_EPOCHS = 100
BATCH_SIZE = 32
LEARNING_RATE = 0.001
VAL_SPLIT = 0.15
DROPOUT = 0.3  # increased from 0.1 for regularization
WEIGHT_DECAY = 1e-4  # L2 regularization
EARLY_STOP_PATIENCE = 15
SEED = 42

DATA_DIR = Path(__file__).parent.parent / "data"
TRAIN_FILE = DATA_DIR / "routing-train-merged.jsonl"
OUTPUT_DIR = Path(__file__).parent.parent / "models" / "switchyard-router"

LABEL2ID = {
    "classification": 0, "code": 1, "creative": 2, "extraction": 3,
    "general": 4, "memory": 5, "reasoning": 6, "search": 7,
    "structured_output": 8, "summarization": 9, "tool_call": 10,
    "translation": 11, "vision": 12,
}
ID2LABEL = {v: k for k, v in LABEL2ID.items()}
COMPLEXITY2ID = {"low": 0, "medium": 1, "high": 2}
ID2COMPLEXITY = {v: k for k, v in COMPLEXITY2ID.items()}


def load_data():
    """Load training data from JSONL."""
    data = []
    with open(TRAIN_FILE) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            text = d.get("text") or d.get("query", "")
            label = d.get("label") or d.get("category", "")
            complexity = d.get("complexity", "medium")
            if label in LABEL2ID and complexity in COMPLEXITY2ID:
                data.append({
                    "text": text,
                    "label_id": LABEL2ID[label],
                    "complexity_id": COMPLEXITY2ID[complexity],
                })
    return data


def get_embeddings(texts):
    """Compute embeddings using fastembed (matches runtime)."""
    from fastembed import TextEmbedding
    model = TextEmbedding(model_name=EMBEDDING_MODEL)
    embeddings = list(model.embed(texts))
    return np.array(embeddings, dtype=np.float32)


class MultiTaskClassifier:
    """Multi-task classifier with shared hidden layer."""
    
    def __init__(self, embedding_dim=EMBEDDING_DIM, hidden_dim=HIDDEN_DIM,
                 num_labels=13, num_complexities=3, dropout=DROPOUT):
        self.embedding_dim = embedding_dim
        self.hidden_dim = hidden_dim
        self.num_labels = num_labels
        self.num_complexities = num_complexities
        self.dropout = dropout
        
        # Xavier initialization
        scale_shared = np.sqrt(2.0 / (embedding_dim + hidden_dim))
        scale_cat = np.sqrt(2.0 / (hidden_dim + num_labels))
        scale_cmp = np.sqrt(2.0 / (hidden_dim + num_complexities))
        
        self.shared_w = np.random.randn(hidden_dim, embedding_dim).astype(np.float32) * scale_shared
        self.shared_b = np.zeros(hidden_dim, dtype=np.float32)
        
        self.cat_w = np.random.randn(num_labels, hidden_dim).astype(np.float32) * scale_cat
        self.cat_b = np.zeros(num_labels, dtype=np.float32)
        
        self.cmp_w = np.random.randn(num_complexities, hidden_dim).astype(np.float32) * scale_cmp
        self.cmp_b = np.zeros(num_complexities, dtype=np.float32)
        
        # Adam state
        self._init_adam()
    
    def _init_adam(self):
        self.adam_t = 0
        self.adam_m = {k: np.zeros_like(v) for k, v in self.params().items()}
        self.adam_v = {k: np.zeros_like(v) for k, v in self.params().items()}
    
    def params(self):
        return {
            "shared_w": self.shared_w, "shared_b": self.shared_b,
            "cat_w": self.cat_w, "cat_b": self.cat_b,
            "cmp_w": self.cmp_w, "cmp_b": self.cmp_b,
        }
    
    def forward(self, embeddings, training=True):
        """Forward pass. Returns (cat_logits, cmp_logits, hidden)."""
        # Shared layer: ReLU
        hidden = embeddings @ self.shared_w.T + self.shared_b
        hidden = np.maximum(hidden, 0)  # ReLU
        
        # Dropout during training
        if training and self.dropout > 0:
            mask = (np.random.rand(*hidden.shape) > self.dropout).astype(np.float32)
            hidden = hidden * mask / (1 - self.dropout)
        
        # Category head
        cat_logits = hidden @ self.cat_w.T + self.cat_b
        
        # Complexity head
        cmp_logits = hidden @ self.cmp_w.T + self.cmp_b
        
        return cat_logits, cmp_logits, hidden
    
    def loss(self, cat_logits, cmp_logits, cat_labels, cmp_labels):
        """Cross-entropy loss for both heads."""
        # Category loss
        cat_probs = self._softmax(cat_logits)
        cat_loss = -np.mean(np.log(cat_probs[np.arange(len(cat_labels)), cat_labels] + 1e-8))
        
        # Complexity loss
        cmp_probs = self._softmax(cmp_logits)
        cmp_loss = -np.mean(np.log(cmp_probs[np.arange(len(cmp_labels)), cmp_labels] + 1e-8))
        
        # L2 regularization
        l2 = 0
        for p in self.params().values():
            l2 += np.sum(p ** 2)
        l2 *= WEIGHT_DECAY / 2
        
        return cat_loss + 0.5 * cmp_loss + l2, cat_loss, cmp_loss
    
    def backward(self, cat_logits, cmp_logits, hidden, embeddings,
                 cat_labels, cmp_labels, lr=LEARNING_RATE):
        """Backward pass with Adam optimizer."""
        batch_size = len(cat_labels)
        self.adam_t += 1
        
        cat_probs = self._softmax(cat_logits)
        cmp_probs = self._softmax(cmp_logits)
        
        # Gradients for category head
        cat_onehot = np.zeros_like(cat_probs)
        cat_onehot[np.arange(batch_size), cat_labels] = 1
        d_cat_logits = (cat_probs - cat_onehot) / batch_size
        
        grads = {}
        grads["cat_w"] = d_cat_logits.T @ hidden
        grads["cat_b"] = d_cat_logits.sum(axis=0)
        
        # Gradients for complexity head
        cmp_onehot = np.zeros_like(cmp_probs)
        cmp_onehot[np.arange(batch_size), cmp_labels] = 1
        d_cmp_logits = (cmp_probs - cmp_onehot) / batch_size * 0.5
        
        grads["cmp_w"] = d_cmp_logits.T @ hidden
        grads["cmp_b"] = d_cmp_logits.sum(axis=0)
        
        # Gradient through shared layer
        d_hidden = d_cat_logits @ self.cat_w + d_cmp_logits @ self.cmp_w
        
        # Back through dropout
        # (simplified - just pass through for ReLU)
        d_hidden[hidden <= 0] = 0
        
        grads["shared_w"] = d_hidden.T @ embeddings
        grads["shared_b"] = d_hidden.sum(axis=0)
        
        # L2 regularization
        for k in grads:
            grads[k] += WEIGHT_DECAY * self.params()[k]
        
        # Adam update
        for name in grads:
            self.adam_m[name] = 0.9 * self.adam_m[name] + 0.1 * grads[name]
            self.adam_v[name] = 0.999 * self.adam_v[name] + 0.001 * grads[name] ** 2
            m_hat = self.adam_m[name] / (1 - 0.9 ** self.adam_t)
            v_hat = self.adam_v[name] / (1 - 0.999 ** self.adam_t)
            self.params()[name] -= lr * m_hat / (np.sqrt(v_hat) + 1e-8)
    
    def predict(self, embeddings):
        """Predict categories and complexities."""
        cat_logits, cmp_logits, _ = self.forward(embeddings, training=False)
        cat_probs = self._softmax(cat_logits)
        cmp_probs = self._softmax(cmp_logits)
        return cat_probs, cmp_probs
    
    def accuracy(self, cat_probs, cmp_probs, cat_labels, cmp_labels):
        """Compute accuracy."""
        cat_preds = np.argmax(cat_probs, axis=1)
        cmp_preds = np.argmax(cmp_probs, axis=1)
        cat_acc = np.mean(cat_preds == cat_labels)
        cmp_acc = np.mean(cmp_preds == cmp_labels)
        return cat_acc, cmp_acc
    
    def _softmax(self, logits):
        x = logits - np.max(logits, axis=1, keepdims=True)
        e = np.exp(x)
        return e / np.sum(e, axis=1, keepdims=True)
    
    def export_weights(self, path):
        """Export weights to binary format matching Rust classifier."""
        data = struct.pack(
            "<4i",
            self.num_labels, self.embedding_dim,
            self.hidden_dim, self.num_complexities,
        )
        expected_floats = (
            self.embedding_dim * self.hidden_dim  # shared_w
            + self.hidden_dim                     # shared_b
            + self.hidden_dim * self.num_labels   # cat_w
            + self.num_labels                     # cat_b
            + self.hidden_dim * self.num_complexities  # cmp_w
            + self.num_complexities               # cmp_b
        )
        for arr in [self.shared_w, self.shared_b, self.cat_w, self.cat_b,
                     self.cmp_w, self.cmp_b]:
            # Explicitly enforce float32
            arr_f32 = arr.astype(np.float32)
            print(f"  {arr.shape} dtype={arr.dtype} -> float32, {arr_f32.nbytes} bytes")
            data += arr_f32.tobytes()
        
        expected_bytes = 16 + expected_floats * 4
        print(f"Exported weights: {len(data)} bytes (expected {expected_bytes}) to {path}")
        assert len(data) == expected_bytes, f"Size mismatch: {len(data)} != {expected_bytes}"
        path.write_bytes(data)


def train():
    random.seed(SEED)
    np.random.seed(SEED)
    
    print("=" * 60)
    print("MULTI-TASK CLASSIFIER TRAINING")
    print("=" * 60)
    
    # Load data
    data = load_data()
    print(f"Loaded {len(data)} training examples")
    
    # Split into train/val
    random.shuffle(data)
    val_size = int(len(data) * VAL_SPLIT)
    val_data = data[:val_size]
    train_data = data[val_size:]
    print(f"Train: {len(train_data)}, Val: {len(val_data)}")
    
    # Compute embeddings (frozen - using fastembed)
    print("Computing training embeddings...")
    train_texts = [d["text"] for d in train_data]
    train_embs = get_embeddings(train_texts)
    
    print("Computing validation embeddings...")
    val_texts = [d["text"] for d in val_data]
    val_embs = get_embeddings(val_texts)
    
    train_cat_labels = np.array([d["label_id"] for d in train_data])
    train_cmp_labels = np.array([d["complexity_id"] for d in train_data])
    val_cat_labels = np.array([d["label_id"] for d in val_data])
    val_cmp_labels = np.array([d["complexity_id"] for d in val_data])
    
    print(f"Embedding shape: {train_embs.shape}")
    
    # Initialize model
    model = MultiTaskClassifier()
    
    # Training loop
    print(f"\nTraining for {NUM_EPOCHS} epochs (batch_size={BATCH_SIZE}, lr={LEARNING_RATE})")
    print(f"Dropout={DROPOUT}, Weight Decay={WEIGHT_DECAY}, Early Stop Patience={EARLY_STOP_PATIENCE}")
    print("-" * 60)
    
    best_val_acc = 0
    patience_counter = 0
    
    for epoch in range(NUM_EPOCHS):
        # Shuffle training data
        perm = np.random.permutation(len(train_data))
        train_embs_shuffled = train_embs[perm]
        train_cat_shuffled = train_cat_labels[perm]
        train_cmp_shuffled = train_cmp_labels[perm]
        
        # Mini-batch training
        epoch_loss = 0
        n_batches = 0
        for i in range(0, len(train_data), BATCH_SIZE):
            batch_embs = train_embs_shuffled[i:i+BATCH_SIZE]
            batch_cat = train_cat_shuffled[i:i+BATCH_SIZE]
            batch_cmp = train_cmp_shuffled[i:i+BATCH_SIZE]
            
            cat_logits, cmp_logits, hidden = model.forward(batch_embs, training=True)
            loss, cat_loss, cmp_loss = model.loss(cat_logits, cmp_logits, batch_cat, batch_cmp)
            model.backward(cat_logits, cmp_logits, hidden, batch_embs, batch_cat, batch_cmp)
            
            epoch_loss += loss
            n_batches += 1
        
        # Validation
        val_cat_probs, val_cmp_probs = model.predict(val_embs)
        val_cat_acc, val_cmp_acc = model.accuracy(val_cat_probs, val_cmp_probs, val_cat_labels, val_cmp_labels)
        
        # Combined metric: weighted average of category and complexity accuracy
        combined_val = 0.8 * val_cat_acc + 0.2 * val_cmp_acc
        
        if (epoch + 1) % 5 == 0 or epoch == 0:
            print(f"Epoch {epoch+1:3d} | loss={epoch_loss/n_batches:.4f} | "
                  f"val_cat={val_cat_acc:.1%} val_cmp={val_cmp_acc:.1%} combined={combined_val:.1%}")
        
        # Early stopping
        if combined_val > best_val_acc:
            best_val_acc = combined_val
            patience_counter = 0
            # Save best model temporarily
            best_params = {k: v.copy() for k, v in model.params().items()}
            best_adam = {k: v.copy() for k, v in model.adam_m.items()}
        else:
            patience_counter += 1
            if patience_counter >= EARLY_STOP_PATIENCE:
                print(f"\nEarly stopping at epoch {epoch+1} (best combined val: {best_val_acc:.1%})")
                break
    
    # Restore best model
    for k, v in best_params.items():
        setattr(model, k, v)
    
    print(f"\n{'='*60}")
    print(f"BEST VALIDATION: category={val_cat_acc:.1%}, complexity={val_cmp_acc:.1%}, combined={best_val_acc:.1%}")
    print(f"{'='*60}")
    
    # Per-category validation accuracy
    val_cat_preds = np.argmax(val_cat_probs, axis=1)
    print("\nPer-category validation accuracy:")
    for cat_id in sorted(set(val_cat_labels)):
        mask = val_cat_labels == cat_id
        if mask.sum() > 0:
            acc = np.mean(val_cat_preds[mask] == cat_id)
            print(f"  {ID2LABEL[cat_id]:20s}: {acc:.1%} ({mask.sum()} samples)")
    
    # Export weights
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    weights_path = OUTPUT_DIR / "weights.bin"
    model.export_weights(weights_path)
    
    # Save labels
    labels_path = OUTPUT_DIR / "labels.json"
    labels_json = {
        "label2id": LABEL2ID,
        "id2label": ID2LABEL,
        "complexity2id": COMPLEXITY2ID,
        "id2complexity": ID2COMPLEXITY,
    }
    labels_path.write_text(json.dumps(labels_json, indent=2))
    print(f"Saved labels to {labels_path}")
    
    print(f"\nModel saved to {OUTPUT_DIR}")
    print(f"Weight file size: {weights_path.stat().st_size} bytes")


if __name__ == "__main__":
    train()
