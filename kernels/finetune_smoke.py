import sys
import os
import json
import math
import random
import subprocess

print("=== Doc Oc BTC Fine-Tune (Synthetic Data) ===")
print(f"Python: {sys.version}")

# Check GPU
try:
    import torch
    print(f"PyTorch: {torch.__version__}")
    print(f"CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"GPU count: {torch.cuda.device_count()}")
        for i in range(torch.cuda.device_count()):
            print(f"  GPU {i}: {torch.cuda.get_device_name(i)}")
except ImportError:
    print("PyTorch not installed")
    sys.exit(1)

# Install Laya from PyPI
print("\n--- Installing Laya from PyPI ---")
subprocess.run([sys.executable, "-m", "pip", "install", "laya", "--quiet"], check=True)
print("Installed Laya")

result = subprocess.run(["laya-train", "--help"], capture_output=True, text=True)
if result.returncode != 0:
    print(f"laya-train not found: {result.stderr[:300]}")
    sys.exit(1)
TRAIN_CMD = ["laya-train"]
print("laya-train verified")

# Install training dependencies
print("\n--- Installing training deps ---")
subprocess.run([sys.executable, "-m", "pip", "install", "transformers", "accelerate", "datasets", "numpy", "--quiet"], check=True)
print("Installed training deps")

# Generate synthetic training data (mimicking real BTC features)
print("\n--- Generating training dataset ---")
os.makedirs("/kaggle/working/data", exist_ok=True)

random.seed(42)
np_random = random

# Simulate realistic BTC price series
import numpy as np
n_bars = 2000
prices = [65000.0]
for i in range(1, n_bars):
    ret = np.random.normal(0.0001, 0.015)
    prices.append(prices[-1] * (1 + ret))
prices = np.array(prices)

# Compute features
log_returns = np.diff(np.log(prices))
realized_vol = np.full(n_bars, np.nan)
for i in range(24, n_bars):
    realized_vol[i] = np.std(log_returns[i-24:i]) * np.sqrt(24)

mom_24 = np.full(n_bars, np.nan)
for i in range(24, n_bars):
    mom_24[i] = prices[i] / prices[i-24] - 1.0

mom_72 = np.full(n_bars, np.nan)
for i in range(72, n_bars):
    mom_72[i] = prices[i] / prices[i-72] - 1.0

trends = ["up", "down", "sideways"]
regimes = ["trending", "ranging", "volatile"]
news_options = ["ETF inflows positive", "funding neutral", "regulatory concerns", "macro risk-off", "halving narrative"]

rows = []
valid = 0
for i in range(200, n_bars - 24):
    vol = realized_vol[i]
    m24 = mom_24[i]
    m72 = mom_72[i] if np.isfinite(mom_72[i]) else m24
    if not (np.isfinite(vol) and np.isfinite(m24) and vol > 0):
        continue

    # Teacher signal: v1 style (GARCH + HMM approximated)
    trend_z = 0.6 * (m24 / (vol + 1e-9)) + 0.4 * (m72 / (vol * np.sqrt(3) + 1e-9))
    regime_z = 1.0 if m24 > 0 else -1.0  # simplified regime
    edge = 0.5 * trend_z + 0.5 * regime_z
    cost_z = 0.0012 / (vol + 1e-9)
    net_edge = edge - cost_z

    logits = [2.0 * net_edge, -2.0 * net_edge, 0.0]
    logits = [l - max(logits) for l in logits]
    probs = [np.exp(l) for l in logits]
    total = sum(probs)
    p_long, p_short, p_no = probs[0]/total, probs[1]/total, probs[2]/total

    if max(p_long, p_short) < 0.4:
        p_long, p_short, p_no = 0.2, 0.2, 0.6

    edge_prob = min(0.95, max(0.05, 1.0 / (1.0 + np.exp(-net_edge * 2))))
    conviction = min(2.0, max(0.0, abs(net_edge)))
    trend = "up" if m24 > 0.005 else ("down" if m24 < -0.005 else "sideways")
    regime = "trending" if abs(m24) > 0.01 else "ranging"
    news = np_random.choice(news_options)
    vol_str = f"{vol*100:.2f}%"

    recent_bars = []
    for j in range(max(0, i-5), i+1):
        recent_bars.append(f"O:{prices[j]:.0f} H:{prices[j]*1.002:.0f} L:{prices[j]*0.998:.0f} C:{prices[j]:.0f}")
    bars_text = " | ".join(recent_bars)

    state = (
        f"Account: paper. Equity 100000 USD. Position: flat. "
        f"Market: BTC/USDT perpetual. Last price: {prices[i]:.0f}. "
        f"24h momentum: {m24*100:.2f}%. "
        f"GARCH vol (24h): {vol_str}. "
        f"Recent bars: {bars_text}. "
        f"Regime: {regime}. News: {news}."
    )

    row = {
        "state": state,
        "questions": {
            "action": {
                "type": "choice",
                "instructions": "What should the desk do?",
                "criteria": ["LONG", "SHORT", "HOLD", "NO_ACTION", "CLOSE"],
            },
            "edge": {
                "type": "noul",
                "instructions": "Does this setup have positive expected value after costs?",
            },
            "conviction": {
                "type": "score",
                "instructions": "How strong is the conviction?",
                "criteria": ["low", "medium", "high"],
            },
        },
        "gold": {
            "action": {
                "probabilities": {
                    "LONG": round(float(p_long), 4),
                    "SHORT": round(float(p_short), 4),
                    "HOLD": 0.05,
                    "NO_ACTION": round(max(0.0, float(p_no) - 0.05), 4),
                    "CLOSE": 0.05,
                }
            },
            "edge": {
                "probabilities": {
                    "false": round(1.0 - float(edge_prob), 4),
                    "true": round(float(edge_prob), 4),
                }
            },
            "conviction": {
                "probabilities": {"0": 0.2, "1": 0.3, "2": 0.5},
                "score": round(float(conviction), 4),
            },
        },
    }
    rows.append(row)
    valid += 1

os.makedirs("/kaggle/working/data", exist_ok=True)
with open("/kaggle/working/data/train.jsonl", "w") as f:
    for row in rows:
        f.write(json.dumps(row) + "\n")
print(f"Wrote {len(rows)} training examples ({valid} valid)")

# Dry run first
print("\n--- Dry run ---")
result = subprocess.run(
    ["laya-train", "--data", "/kaggle/working/data/train.jsonl", "--dry-run"],
    capture_output=True, text=True,
)
print(result.stdout[-500:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Dry run failed (rc={result.returncode}): {result.stderr[-500:]}")
    sys.exit(1)
print("Dry run PASSED")

# Real training run (4 epochs, freeze-encoder)
print("\n--- Training (4 epochs, freeze-encoder) ---")
result = subprocess.run(
    [
        "laya-train",
        "--data", "/kaggle/working/data/train.jsonl",
        "--out", "/kaggle/working/ckpt",
        "--base", "convaiinnovations/laya",
        "--epochs", "4",
        "--micro-batch", "8",
        "--grad-accum", "4",
        "--freeze-encoder",
        "--device", "cuda",
        "--max-len", "512",
    ],
    capture_output=True, text=True,
)
print(result.stdout[-1500:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Training failed (rc={result.returncode}): {result.stderr[-1500:]}")
    sys.exit(1)

# Save status
os.makedirs("/kaggle/working/ckpt", exist_ok=True)
with open("/kaggle/working/ckpt/smoke_done.txt", "w") as f:
    f.write(f"fine-tune complete: {len(rows)} examples, 4 epochs\n")

print("\n=== Fine-Tune Complete ===")
PYEOF