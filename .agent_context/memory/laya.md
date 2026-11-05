## Facts

- Laya is ConvAI Innovations' Apache-2.0 non-autoregressive decision model.
- Checkpoints: English 421M/512 context; multilingual 322M/1024 (up to 8192); typed-decisions 421M/1024.
- Primitives: `choice`, `score`, `noul`; one forward pass; no text generation.
- CPU inference roughly 100-450 ms; about 33 ms on a T4.
- Base checkpoints are near chance on specialized decisions; fine-tuning is mandatory.
- `laya-train` accepts CSV/JSONL with `state`, `questions`, and `expected` or `gold`; supports RLCD/soft-CE, calibration, and `--freeze-encoder`.
- Kaggle 2xT4 full fine-tune is roughly 4-5 hours for about 30k questions.
- `laya-onnx` Rust crate (imported as `laya`) runs ONNX via `ort`; CPU by default, CUDA/DirectML features; no Python at runtime.
- Context budget: English leaves about 320 tokens for state; multilingual and typed-decisions leave about 768; multilingual can go to 8192.
- Five actions are well below the >20 choice degradation threshold.
- Use `answer_confidence`, not `action.act_probability`.
- `noul` can follow labels rather than state; use a 2-option choice with neutral keys if needed.
- Calibration is mandatory; shipped models are overconfident.

## Decisions

- Full checkpoint per asset; start with multilingual; benchmark typed-decisions in Phase 0.
- Compact state target: 500-700 tokens.
- Teacher labels come from the causal GARCH/Markov/rules policy.

## Gotchas

- Do not train on historical LLM analysis; it can leak the future.
- Full checkpoints are 1.3-1.7 GB each; two resident is roughly 2.6-3.4 GB.
- Adapter or head-only sharing is undocumented; keep it off the critical path.
- ONNX export must be parity-tested against the Python reference.
