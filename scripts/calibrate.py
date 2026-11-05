"""Fit the calibration temperature for a fine-tuned Laya checkpoint.

Reads the model JSON sidecar (logits + gold distribution per question),
fits a single temperature T by minimizing negative log-likelihood
(golden-section search), then reports ECE/Brier before and after.

Usage:
    .venv-quant\Scripts\python.exe scripts\calibrate.py <checkpoint_dir>
    (writes <checkpoint_dir>/calibration.json)
"""

import json
import math
import os
import sys


def softmax(logits, temp):
    scaled = [l / temp for l in logits]
    m = max(scaled)
    exps = [math.exp(l - m) for l in scaled]
    total = sum(exps)
    return [e / total for e in exps]


def nll(data, temp):
    total = 0.0
    for row in data:
        probs = softmax(row["logits"], temp)
        gold = row["gold"]
        for i, g in enumerate(gold):
            total -= g * math.log(max(probs[i], 1e-12))
    return total / len(data)


def ece(data, temp, bins=10):
    # Expected calibration error on the argmax class.
    buckets = [[] for _ in range(bins)]
    for row in data:
        probs = softmax(row["logits"], temp)
        pred = max(range(len(probs)), key=lambda i: probs[i])
        gold = max(range(len(row["gold"])), key=lambda i: row["gold"][i])
        buckets[min(bins - 1, int(probs[pred] * bins))].append(
            1.0 if pred == gold else 0.0)
    e = 0.0
    for b in buckets:
        if b:
            e += abs(sum(b) / len(b) - sum(1 for _ in b) / len(b)) * len(b) / len(data)
    return e


def brier(data, temp):
    total = 0.0
    for row in data:
        probs = softmax(row["logits"], temp)
        total += sum((p - g) ** 2 for p, g in zip(probs, row["gold"]))
    return total / len(data)


def fit(data):
    lo, hi = 0.1, 10.0
    # Golden-section search.
    gr = (math.sqrt(5) - 1) / 2
    c, d = hi - gr * (hi - lo), lo + gr * (hi - lo)
    fc, fd = nll(data, c), nll(data, d)
    for _ in range(60):
        if fc < fd:
            hi, d, fd = d, c, fc
            c = hi - gr * (hi - lo)
            fc = nll(data, c)
        else:
            lo, c, fc = c, d, fd
            d = lo + gr * (hi - lo)
            fd = nll(data, d)
    return (lo + hi) / 2


def main():
    ckpt = sys.argv[1] if len(sys.argv) > 1 else "models/btcusdt/ckpt"
    sidecar = os.path.join(ckpt, "val_logits.json")
    if not os.path.exists(sidecar):
        print(f"No logits sidecar at {sidecar}; run the validation export first.")
        sys.exit(1)
    with open(sidecar) as f:
        data = json.load(f)

    temp = fit(data)
    out = {
        "temperature": round(temp, 6),
        "ece_before": round(ece(data, 1.0), 6),
        "ece_after": round(ece(data, temp), 6),
        "brier_before": round(brier(data, 1.0), 6),
        "brier_after": round(brier(data, temp), 6),
        "n": len(data),
    }
    with open(os.path.join(ckpt, "calibration.json"), "w") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
