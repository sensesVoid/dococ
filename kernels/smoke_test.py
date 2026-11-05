import sys
import os
import json
import time

print("=== Doc Oc Kaggle Smoke Test ===")
print(f"Python: {sys.version}")
print(f"Working dir: {os.getcwd()}")

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

# Check other packages
for pkg in ["numpy", "pandas", "scipy", "onnx", "onnxruntime"]:
    try:
        mod = __import__(pkg)
        print(f"{pkg}: {getattr(mod, '__version__', 'installed')}")
    except ImportError:
        print(f"{pkg}: NOT installed")

# Tiny training smoke test
import numpy as np
X = np.random.randn(1000, 4).astype(np.float32)
y = (X[:, 0] + X[:, 1] > 0).astype(np.float32)

import torch
import torch.nn as nn
model = nn.Sequential(nn.Linear(4, 16), nn.ReLU(), nn.Linear(16, 1), nn.Sigmoid())
opt = torch.optim.Adam(model.parameters(), lr=0.01)
loss_fn = nn.BCELoss()

Xt = torch.from_numpy(X)
yt = torch.from_numpy(y).unsqueeze(1)

for epoch in range(5):
    opt.zero_grad()
    pred = model(Xt)
    loss = loss_fn(pred, yt)
    loss.backward()
    opt.step()
    print(f"Epoch {epoch+1}/5 - loss: {loss.item():.4f}")

# Save a checkpoint
os.makedirs("/kaggle/working/ckpt", exist_ok=True)
torch.save(model.state_dict(), "/kaggle/working/ckpt/smoke.pt")
print("Checkpoint saved to /kaggle/working/ckpt/smoke.pt")

# Write a status file
with open("/kaggle/working/smoke_done.txt", "w") as f:
    f.write("smoke test complete\n")

print("=== Smoke test PASSED ===")
PYEOF