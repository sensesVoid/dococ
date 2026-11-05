import sys
import os
import json
import subprocess

print("=== Doc Oc BTC Fine-Tune (Multilingual Base) ===")
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

# Download the multilingual checkpoint (subfolder of convaiinnovations/laya).
from huggingface_hub import snapshot_download

token = os.environ.get("HF_TOKEN") or os.environ.get("HUGGING_FACE_HUB_TOKEN")
print("Downloading multilingual checkpoint (subfolder)...")
base_dir = snapshot_download(
    repo_id="convaiinnovations/laya",
    allow_patterns=["multilingual/**"],
    token=token,
    local_dir="/kaggle/working/base_multilingual",
)
if os.path.isdir(os.path.join(base_dir, "multilingual", "model.safetensors")) or os.path.isdir(os.path.join(base_dir, "multilingual")):
    pass  # download includes subfolder content at repo root or nested; resolve below
nested = os.path.join(base_dir, "multilingual")
if os.path.isdir(nested) and (
    os.path.exists(os.path.join(nested, "model.safetensors"))
    or os.path.exists(os.path.join(nested, "rl_agent_config.json"))
):
    base_dir = nested
print(f"Base checkpoint at {base_dir}")

DATA = "/kaggle/input/doc-oc-btc-dataset/train.jsonl"
if not os.path.exists(DATA):
    DATA = "/kaggle/input/datasets/sensesvoid/doc-oc-btc-dataset/train.jsonl"
if not os.path.exists(DATA):
    print("Dataset missing")
    sys.exit(2)

with open(DATA) as f:
    rows = [json.loads(line) for line in f]
print(f"Loaded {len(rows)} teacher-labeled examples")

print("\n--- Dry run ---")
try:
    result = subprocess.run(
        ["laya-train", "--data", DATA, "--base", base_dir, "--dry-run"],
        capture_output=True, text=True)
    print(result.stdout[-800:] if result.stdout else "no stdout")
    print("STDERR:", result.stderr[-800:] if result.stderr else "no stderr")
    if result.returncode != 0:
        print(f"Dry run failed (rc={result.returncode})")
        sys.exit(1)
except Exception as e:
    import traceback
    print("Dry run crashed:", e)
    print(traceback.format_exc())
    sys.exit(1)
print("Dry run PASSED")

print("\n--- Training (4 epochs, freeze-encoder, multilingual base) ---")
try:
    result = subprocess.run(
        [
            "laya-train",
            "--data", DATA,
            "--out", "/kaggle/working/ckpt",
            "--base", base_dir,
            "--epochs", "4",
            "--micro-batch", "4",
            "--grad-accum", "8",
            "--freeze-encoder",
            "--device", "cuda",
            "--max-len", "512",
        ],
        capture_output=True, text=True)
    print(result.stdout[-2000:] if result.stdout else "no stdout")
    print("STDERR:", result.stderr[-2000:] if result.stderr else "no stderr")
    if result.returncode != 0:
        os.makedirs("/kaggle/working/ckpt", exist_ok=True)
        with open("/kaggle/working/ckpt/train_failed.txt", "w") as f:
            f.write(f"rc={result.returncode}\nSTDOUT:\n{result.stdout[-4000:]}\nSTDERR:\n{result.stderr[-4000:]}")
        print(f"Training failed (rc={result.returncode})")
        sys.exit(1)
except Exception as e:
    import traceback
    os.makedirs("/kaggle/working/ckpt", exist_ok=True)
    with open("/kaggle/working/ckpt/train_failed.txt", "w") as f:
        f.write(traceback.format_exc())
    print("Training crashed:", e)
    sys.exit(1)

os.makedirs("/kaggle/working/ckpt", exist_ok=True)
with open("/kaggle/working/ckpt/finetune_done.txt", "w") as f:
    f.write(f"multilingual fine-tune complete: {len(rows)} teacher-labeled examples, 4 epochs\n")
print("\n=== Fine-Tune Complete ===")
