# Doc Oc VPS Setup Guide

## Prerequisites
- Linux VPS: 8 vCPU, 16 GB RAM, 100 GB SSD (Ubuntu 22.04+ recommended)
- SSH access as root or ubuntu user

## Quick Start

### 1. Create VPS
Provision a VPS on Hetzner, DigitalOcean, or AWS with:
- 8 vCPU / 16 GB RAM
- Ubuntu 22.04 LTS
- 100 GB SSD
- Low-latency region (close to exchange)

### 2. Run Setup Script
```bash
# SSH into the VPS
ssh ubuntu@<VPS_IP>

# Download and run the setup script
curl -sSL https://raw.githubusercontent.com/your-org/dococ/main/scripts/vps_setup.sh | bash
```

### 3. Clone and Build
```bash
cd /opt/dococ
git clone https://github.com/your-org/dococ.git .
cargo build --release
```

### 4. Export Laya Model
```bash
# Create Python venv
uv venv .venv-laya --python 3.11
uv pip install --python .venv-laya/bin/python torch onnx onnxruntime transformers safetensors huggingface_hub numpy

# Export model
python laya-rust/tools/export_onnx.py --model multilingual --out models/laya --verify
```

### 5. Configure
```bash
# Set environment variables
export DOCOC_LAYA_MODEL=/opt/dococ/models/laya/multilingual
export HF_TOKEN=<your_token>

# Run daemon
./target/release/dococ daemon --config config.toml
```

### 6. Benchmark
```bash
DOCOC_LAYA_MODEL=/opt/dococ/models/laya/multilingual \
  ./target/release/examples/laya_bench --threads 8 --reps 10 --n 5
```

## Notes
- The Laya model is ~1.3 GB; ensure sufficient disk space.
- ONNX Runtime will use all available cores by default.
- The daemon runs as a systemd service in production.
- See `Implementation_Checklist.md` Phase 0.6 for acceptance criteria.