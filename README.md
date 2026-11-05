# Doc Oc

Paper-first autonomous trading system. Laya selects the action; a deterministic guardian enforces account risk; an OpenCode-hosted LLM analyzes macro, news, and regime every 15 minutes without order authority.

Status: Phase 0 spikes. No live trading.

## Workspace

| Crate | Purpose |
|---|---|
| `dococ-core` | Domain types |
| `dococ-risk` | Deterministic risk guardian |
| `dococ-oms` | Order state machine and reconciliation |
| `dococ-venue` | Venue trait and sim venue |
| `dococ-engine` | Daemon and event loop |
| `dococ-cli` | Command-line client |
| `dococ-laya` | Laya action schema and ONNX runtime wrapper |

## Commands

```powershell
cargo test --workspace
cargo run -p dococ-cli
```

## Laya ONNX export

The ONNX artifacts are not published on HuggingFace yet. Export them locally:

```powershell
.\scripts\export_laya_model.ps1 -Model multilingual -Verify
```

The output lands in `models/laya/multilingual/`. The script expects `.venv-laya` with the export dependencies installed.

## Performance

```powershell
.\scripts\run_laya_bench.ps1 -Threads 4 -Reps 10 -N 5
```

Record load time, warm latency, and peak RSS for context lengths 512, 1024, and 2048. The first single-thread run on this laptop measured about 5.5 s per decision for a ~550-token state, with a peak RSS of about 1.5 GB.

## Design

- `Design_Document.md`
- `Design_Recommendations.md`
- `Open_Questions.md`
- `Implementation_Checklist.md`
- `Handoff.md`
