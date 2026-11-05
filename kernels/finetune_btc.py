import sys
import os
import json
import subprocess

print("=== Doc Oc BTC Fine-Tune (Teacher Labels) ===")
print(f"Python: {sys.version}")

try:
    import torch
    print(f"PyTorch: {torch.__version__} CUDA: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"GPU: {torch.cuda.get_device_name(0)}")
except ImportError:
    print("PyTorch not installed")
    sys.exit(1)

subprocess.run([sys.executable, "-m", "pip", "install", "laya", "--quiet"], check=True)
print("Installed Laya")

result = subprocess.run(["laya-train", "--help"], capture_output=True, text=True)
if result.returncode != 0:
    print(f"laya-train not found: {result.stderr[:300]}")
    sys.exit(1)
print("laya-train verified")

DATA = "/kaggle/input/doc-oc-btc-dataset/train.jsonl"
if not os.path.exists(DATA):
    print(f"Dataset not found at {DATA}; inputs present:")
    for root, dirs, files in os.walk("/kaggle/input"):
        for f in files[:10]:
            print(os.path.join(root, f))
    sys.exit(2)

with open(DATA) as f:
    rows = [json.loads(line) for line in f]
print(f"Loaded {len(rows)} teacher-labeled examples")

print("\n--- Dry run ---")
result = subprocess.run(
    ["laya-train", "--data", DATA, "--dry-run"],
    capture_output=True, text=True)
print(result.stdout[-800:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Dry run failed (rc={result.returncode}): {result.stderr[-800:]}")
    sys.exit(1)
print("Dry run PASSED")

print("\n--- Training (4 epochs, freeze-encoder, teacher labels) ---")
result = subprocess.run(
    [
        "laya-train",
        "--data", DATA,
        "--out", "/kaggle/working/ckpt",
        "--base", "convaiinnovations/laya",
        "--epochs", "4",
        "--micro-batch", "8",
        "--grad-accum", "4",
        "--freeze-encoder",
        "--device", "cuda",
        "--max-len", "512",
    ],
    capture_output=True, text=True)
print(result.stdout[-2000:] if result.stdout else "no stdout")
if result.returncode != 0:
    print(f"Training failed (rc={result.returncode}): {result.stderr[-2000:]}")
    sys.exit(1)

os.makedirs("/kaggle/working/ckpt", exist_ok=True)
with open("/kaggle/working/ckpt/finetune_done.txt", "w") as f:
    f.write(f"fine-tune complete: {len(rows)} teacher-labeled examples, 4 epochs\n")
print("\n=== Fine-Tune Complete ===")
