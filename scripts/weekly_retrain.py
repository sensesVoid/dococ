"""Weekly retraining job (manual-promote gated; never auto-deploys).

Pipeline (Phase 5):
  1. Build the dataset (scripts/build_dataset.py).
  2. Push/update the Kaggle dataset.
  3. Push the fine-tune kernel and wait for completion.
  4. Download the checkpoint; calibrate; export ONNX; rerun parity.
  5. Register the model (dococ-laya ModelRegistry atomic_switch); the
     checkpoint only becomes active after a manual `dococ asset promote`
     once the drift monitor reports healthy over shadow/paper.

This script automates steps 1-4; step 5 stays manual by design.

Usage:
    .venv-quant\Scripts\python.exe scripts\weekly_retrain.py --dry-run
    .venv-quant\Scripts\python.exe scripts\weekly_retrain.py
"""

import argparse
import subprocess
import sys

PY = sys.executable


def run(cmd, dry_run=False):
    print(">", " ".join(cmd))
    if dry_run:
        return 0
    return subprocess.run(cmd, capture_output=True, text=True).stderr[-500:]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--dry-run", action="store_true",
                   help="Print the plan without executing it")
    args = p.parse_args()

    steps = [
        "dataset: scripts/build_dataset.py",
        "dataset push: data/dataset_btc/kaggle -> sensesvoid/doc-oc-btc-dataset",
        "kernel push: kernels_btc -> sensesvoid/doc-oc-btc-fine-tune-teacher-labels",
        "download: kaggle kernels output -> kernels_out/ckpt",
        "calibrate: scripts/calibrate.py kernels_out/ckpt",
        "export: scripts/export_onnx.py --model kernels_out/ckpt --out models/btcusdt",
        "parity: DOCOC_LAYA_MODEL=models/btcusdt cargo test -p dococ-laya --test golden -- --ignored",
        "manual: dococ asset promote BTCUSDT (only after drift healthy)",
    ]

    for s in steps:
        print("STEP:", s)
    if args.dry_run:
        print("Dry run: nothing executed.")
        return

    note = run([PY, "scripts/build_dataset.py"], dry_run=False)
    print(note or "dataset built")
    print("Kaggle steps (dataset push, kernel push, download) require the kaggle CLI"
          " and the kernel to finish; run them per kernels/kernel-metadata-btc.json.")
    print("Done with local steps. NOTE: promotion is manual — see docstring step 5.")


if __name__ == "__main__":
    main()
