"""Export a fine-tuned Laya checkpoint to ONNX, then verify Rust parity.

Usage (PowerShell):
    $env:DOCOC_LAYA_CKPT = "models/btcusdt/ckpt"
    .venv-laya\Scripts\python.exe scripts\export_onnx.py --model <ckpt> --out models/btcusdt
    $env:DOCOC_LAYA_MODEL = "models/btcusdt"
    cargo test -p dococ-laya --test golden -- --ignored
"""

import argparse
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXPORTER = os.path.join(ROOT, "third_party", "laya", "scripts", "export_onnx.py")


def export_onnx(model: str, output: str, verify: bool = True):
    """Export --output must be the full file path (e.g. models/btcusdt/model.onnx)
    so the baked external-data filename is model.onnx.data (what Rust loads)."""
    if not os.path.exists(EXPORTER):
        print(f"Exporter not found: {EXPORTER}")
        sys.exit(1)

    cmd = [sys.executable, EXPORTER, "--model", model, "--output", output]
    print("Running:", " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(r.stdout[-3000:] if r.stdout else "no stdout")
    print(r.stderr[-3000:] if r.stderr else "no stderr", file=sys.stderr)
    if r.returncode != 0:
        sys.exit(r.returncode)
    print("ONNX export complete. Run the Rust parity test with DOCOC_LAYA_MODEL set.")


def assemble_model_dir(ckpt: str, out_dir: str) -> str | None:
    """Assemble the Rust-loadable model dir: model.onnx, model.onnx.data,
    tokenizer (from checkpoint), rl_agent_config.json (from checkpoint)."""
    import shutil
    os.makedirs(out_dir, exist_ok=True)
    for sub in os.listdir(out_dir):
        p = os.path.join(out_dir, sub)
        if os.path.isfile(p):
            os.remove(p)
    tok = os.path.join(ckpt, "tokenizer")
    if os.path.isdir(tok):
        dst = os.path.join(out_dir, "tokenizer")
        if os.path.isdir(dst):
            shutil.rmtree(dst)
        shutil.copytree(tok, dst)
    cfg = os.path.join(ckpt, "rl_agent_config.json")
    if os.path.exists(cfg):
        shutil.copy(cfg, out_dir)
    return out_dir


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--model", required=True, help="Fine-tuned checkpoint directory")
    p.add_argument("--out", required=True, help="Model dir (e.g. models/btcusdt)")
    p.add_argument("--verify", action="store_true", default=True)
    args = p.parse_args()

    export_onnx(args.model, os.path.join(args.out, "model.onnx"), args.verify)
    assemble_model_dir(args.model, args.out)
    print("Run the Rust parity test with DOCOC_LAYA_MODEL set.")


if __name__ == "__main__":
    main()
