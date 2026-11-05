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
EXPORTER = os.path.join(ROOT, "third_party", "laya", "export_onnx.py")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--model", required=True, help="Fine-tuned checkpoint directory")
    p.add_argument("--out", required=True, help="ONNX output directory")
    p.add_argument("--verify", action="store_true", default=True)
    args = p.parse_args()

    if not os.path.exists(EXPORTER):
        print(f"Exporter not found: {EXPORTER}")
        sys.exit(1)

    cmd = [sys.executable, EXPORTER, "--model", args.model, "--out", args.out, "--verify"]
    print("Running:", " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(r.stdout[-3000:] if r.stdout else "no stdout")
    print(r.stderr[-3000:] if r.stderr else "no stderr", file=sys.stderr)
    if r.returncode != 0:
        sys.exit(r.returncode)
    print("ONNX export complete. Run the Rust parity test with DOCOC_LAYA_MODEL set.")


if __name__ == "__main__":
    main()
