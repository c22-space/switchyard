#!/usr/bin/env python3
"""
Evaluate the fine-tuned classifier against a benchmark.
Loads the binary weights, computes fastembed embeddings, runs inference.
"""
import json
import struct
import time
import sys
from pathlib import Path
from collections import defaultdict

import numpy as np

BENCHMARK_FILE = Path(__file__).parent.parent / "data" / "benchmark_v3.jsonl"
WEIGHTS_FILE = Path(__file__).parent.parent / "models" / "switchyard-router" / "weights.bin"
LABELS_FILE = Path(__file__).parent.parent / "models" / "switchyard-router" / "labels.json"
EMBEDDING_MODEL = "sentence-transformers/all-MiniLM-L6-v2"
EMBEDDING_DIM = 384


def load_labels():
    with open(LABELS_FILE) as f:
        data = json.load(f)
    return data["id2label"], data.get("id2complexity", {"0": "low", "1": "medium", "2": "high"})


def load_weights():
    data = WEIGHTS_FILE.read_bytes()
    num_labels, embedding_dim, hidden_dim, num_complexities = struct.unpack_from("<4i", data, 0)
    
    offset = 16
    def read_f32s(count):
        nonlocal offset
        arr = np.frombuffer(data[offset:offset+count*4], dtype=np.float32).copy()
        offset += count * 4
        return arr
    
    shared_w = read_f32s(embedding_dim * hidden_dim).reshape(hidden_dim, embedding_dim)
    shared_b = read_f32s(hidden_dim)
    cat_w = read_f32s(hidden_dim * num_labels).reshape(num_labels, hidden_dim)
    cat_b = read_f32s(num_labels)
    cmp_w = read_f32s(hidden_dim * num_complexities).reshape(num_complexities, hidden_dim)
    cmp_b = read_f32s(num_complexities)
    
    return {
        "shared_w": shared_w, "shared_b": shared_b,
        "cat_w": cat_w, "cat_b": cat_b,
        "cmp_w": cmp_w, "cmp_b": cmp_b,
        "num_labels": num_labels, "embedding_dim": embedding_dim,
        "hidden_dim": hidden_dim, "num_complexities": num_complexities,
    }


def softmax(x):
    e = np.exp(x - np.max(x, axis=1, keepdims=True))
    return e / np.sum(e, axis=1, keepdims=True)


def predict(embeddings, weights):
    # Shared layer
    hidden = embeddings @ weights["shared_w"].T + weights["shared_b"]
    hidden = np.maximum(hidden, 0)  # ReLU
    
    # Category head
    cat_logits = hidden @ weights["cat_w"].T + weights["cat_b"]
    cat_probs = softmax(cat_logits)
    
    # Complexity head
    cmp_logits = hidden @ weights["cmp_w"].T + weights["cmp_b"]
    cmp_probs = softmax(cmp_logits)
    
    return cat_probs, cmp_probs


def main():
    id2label, id2complexity = load_labels()
    weights = load_weights()
    
    print(f"Loaded weights: {weights['num_labels']} labels, {weights['embedding_dim']}d -> {weights['hidden_dim']}h")
    
    # Load benchmark
    queries = []
    expected_cats = []
    with open(BENCHMARK_FILE) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            queries.append(d.get("query") or d.get("text", ""))
            expected_cats.append(d.get("category") or d.get("label", ""))
    
    print(f"Benchmark: {len(queries)} queries, {len(set(expected_cats))} categories")
    
    # Compute embeddings
    print("Computing embeddings...")
    from fastembed import TextEmbedding
    model = TextEmbedding(model_name=EMBEDDING_MODEL)
    embeddings = np.array(list(model.embed(queries)), dtype=np.float32)
    print(f"Embeddings shape: {embeddings.shape}")
    
    # Predict
    t0 = time.time()
    cat_probs, cmp_probs = predict(embeddings, weights)
    latency_ms = (time.time() - t0) / len(queries) * 1000
    
    cat_preds = np.argmax(cat_probs, axis=1)
    cat_conf = np.max(cat_probs, axis=1)
    
    # Overall accuracy
    correct = sum(1 for p, e in zip(cat_preds, expected_cats) if id2label[str(p)] == e)
    accuracy = correct / len(queries)
    
    print(f"\n{'='*60}")
    print(f"OVERALL: {correct}/{len(queries)} = {accuracy:.1%}")
    print(f"Average latency: {latency_ms:.1f}ms per query")
    print(f"{'='*60}")
    
    # Per-category breakdown
    cat_groups = defaultdict(lambda: {"correct": 0, "total": 0, "errors": []})
    for i, (pred_id, expected) in enumerate(zip(cat_preds, expected_cats)):
        pred_label = id2label[str(pred_id)]
        cat_groups[expected]["total"] += 1
        if pred_label == expected:
            cat_groups[expected]["correct"] += 1
        else:
            cat_groups[expected]["errors"].append((queries[i], pred_label, cat_conf[i]))
    
    print(f"\n{'Category':<20} {'Correct':>8} {'Total':>6} {'Accuracy':>10}")
    print("-" * 50)
    for cat in sorted(cat_groups.keys()):
        g = cat_groups[cat]
        acc = g["correct"] / g["total"] if g["total"] > 0 else 0
        print(f"{cat:<20} {g['correct']:>8} {g['total']:>6} {acc:>10.1%}")
    
    # Show errors
    total_errors = sum(len(g["errors"]) for g in cat_groups.values())
    print(f"\n{'='*60}")
    print(f"ERRORS ({total_errors}):")
    print(f"{'='*60}")
    for cat in sorted(cat_groups.keys()):
        errors = cat_groups[cat]["errors"]
        if errors:
            print(f"\n  [{cat}] {len(errors)} errors:")
            for query, pred, conf in errors[:5]:
                print(f"    \"{query[:70]}\"")
                print(f"      -> predicted: {pred} (conf: {conf:.1%})")
    
    return accuracy


if __name__ == "__main__":
    accuracy = main()
    # Exit with non-zero if below target
    sys.exit(0 if accuracy >= 0.96 else 1)
