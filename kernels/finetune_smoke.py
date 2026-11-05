import sys
import os
import json
import subprocess

print("=== Doc Oc Fine-Tune Smoke Test ===")
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

# Clone Laya repo
print("\n--- Cloning Laya repo ---")
if not os.path.exists("/kaggle/working/laya"):
    subprocess.run(["git", "clone", "--depth", "1", "https://github.com/NandhaKishorM/laya.git", "/kaggle/working/laya"], check=True)
    print("Cloned successfully")
else:
    print("Already cloned")

os.chdir("/kaggle/working/laya")
print(f"Working dir: {os.getcwd()}")

# Install Laya
print("\n--- Installing Laya ---")
subprocess.run([sys.executable, "-m", "pip", "install", "-e", ".", "--quiet"], check=True)
print("Installed Laya")

# Verify laya-train is available
print("\n--- Verifying laya-train ---")
result = subprocess.run(["laya-train", "--help"], capture_output=True, text=True)
print(f"laya-train exit code: {result.returncode}")
if result.returncode != 0:
    print(f"laya-train not found: {result.stderr[:500]}")
    # Try python -m
    result2 = subprocess.run([sys.executable, "-m", "laya.train_cli", "--help"], capture_output=True, text=True)
    print(f"python -m laya.train_cli exit code: {result2.returncode}")
    if result2.returncode != 0:
        print(f"Also failed: {result2.stderr[:500]}")
        sys.exit(1)
    TRAIN_CMD = [sys.executable, "-m", "laya.train_cli"]
else:
    TRAIN_CMD = ["laya-train"]
print(f"Using: {' '.join(TRAIN_CMD)}")

# Install training dependencies
print("\n--- Installing training deps ---")
subprocess.run([sys.executable, "-m", "pip", "install", "transformers", "accelerate", "datasets", "--quiet"], check=True)
print("Installed training deps")

# Create a small training dataset
print("\n--- Creating training dataset ---")
os.makedirs("/kaggle/working/data", exist_ok=True)

import random
random.seed(42)

states = [
    "Account: paper. Equity 100000 USD. Position: flat. Market: BTC/USDT perpetual. Last price {price}. 4h trend: {trend}. GARCH vol: {vol}. Regime: {regime}. News: {news}.",
    "Account: paper. Equity 100000 USD. Position: long 0.1 BTC. Market: BTC/USDT perpetual. Last price {price}. 4h trend: {trend}. GARCH vol: {vol}. Regime: {regime}. News: {news}.",
]

trends = ["up", "down", "sideways"]
regimes = ["trending", "ranging", "volatile"]
news_options = ["ETF inflows positive", "funding neutral", "regulatory concerns", "macro risk-off"]

rows = []
for i in range(50):
    price = random.randint(60000, 70000)
    trend = random.choice(trends)
    vol = round(random.uniform(0.003, 0.015), 4)
    regime = random.choice(regimes)
    news = random.choice(news_options)
    state = random.choice(states).format(price=price, trend=trend, vol=vol, regime=regime, news=news)

    if trend == "up" and regime == "trending":
        p_long, p_short, p_no = 0.7, 0.1, 0.2
    elif trend == "down" and regime == "trending":
        p_long, p_short, p_no = 0.1, 0.7, 0.2
    else:
        p_long, p_short, p_no = 0.2, 0.2, 0.6

    edge_prob = round(random.uniform(0.3, 0.9), 4)
    conviction = round(random.uniform(0.5, 1.5), 4)

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
                    "LONG": p_long,
                    "SHORT": p_short,
                    "HOLD": 0.05,
                    "NO_ACTION": max(0.0, p_no - 0.05),
                    "CLOSE": 0.05,
                }
            },
            "edge": {
                "probabilities": {
                    "false": 1.0 - edge_prob,
                    "true": edge_prob,
                }
            },
            "conviction": {
                "probabilities": [0.2, 0.3, 0.5],
                "score": conviction,
            },
        },
    }
    rows.append(row)

with open("/kaggle/working/data/train.jsonl", "w") as f:
    for row in rows:
        f.write(json.dumps(row) + "\n")
print(f"Wrote {len(rows)} training examples")

# Dry run first
print("\n--- Dry run ---")
result = subprocess.run(
    TRAIN_CMD + ["--data", "/kaggle/working/data/train.jsonl", "--dry-run"],
    capture_output=True, text=True,
)
print(result.stdout[-500:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Dry run failed (rc={result.returncode}): {result.stderr[-500:]}")
    sys.exit(1)
print("Dry run PASSED")

# Real training run (1 epoch, small batch)
print("\n--- Training (1 epoch) ---")
result = subprocess.run(
    TRAIN_CMD + [
        "--data", "/kaggle/working/data/train.jsonl",
        "--out", "/kaggle/working/ckpt",
        "--base", "convaiinnovations/laya",
        "--epochs", "1",
        "--micro-batch", "4",
        "--grad-accum", "2",
        "--freeze-encoder",
        "--device", "cuda",
    ],
    capture_output=True, text=True,
)
print(result.stdout[-1000:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Training failed (rc={result.returncode}): {result.stderr[-1000:]}")
    sys.exit(1)

# Save status
os.makedirs("/kaggle/working/ckpt", exist_ok=True)
with open("/kaggle/working/ckpt/smoke_done.txt", "w") as f:
    f.write("fine-tune smoke test complete\n")

print("\n=== Fine-Tune Smoke Test PASSED ===")
PYEOF