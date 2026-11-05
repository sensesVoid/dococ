## Facts

- Doc Oc is a Rust daemon plus a terminal client for autonomous, paper-first trading on funded accounts.
- Workspace: `C:\Users\geral\dococ`.
- Scope: BTC/USDT perpetuals and gold futures; paper both first, live BTC first.
- Runtime: embedded Rust `laya-onnx` on CPU; Python only for offline training and ONNX export.
- LLM: OpenCode CLI subprocess every 15 minutes; analyzes macro, news, and regime; no order authority.
- State: compact 500-700 tokens with GARCH, Markov, news, and time-filtered SQAC cases.
- Development: Windows. Production: Linux VPS, 8 vCPU/16 GB RAM.
- Key files: `Design_Document.md`, `Design_Recommendations.md`, `Open_Questions.md`, `Implementation_Checklist.md`, `Handoff.md`.
- Build state (2026-11-05): Phases 1-4 built across 9 crates (added `dococ-quant`: GARCH + Markov Python parity, hysteresis/stale guards, teacher policy + backtest parity, dynamic cost model); `cargo test --workspace` 151 passed, 1 ignored (`model_smoke`), 0 failed; branch `phase-0` at `b8870cf`. Next: Python/Rust golden tests in CI, then Phase 5 training pipeline.

## Decisions

- Laya decides and executes; the deterministic guardian enforces hard risk limits; the LLM advises.
- Teacher: causal GARCH + Markov + deterministic rules; soft labels; purged walk-forward; teacher must beat costs.
- Actions: LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain; CLOSE = exit all; no implicit reversal.
- One full Laya checkpoint per asset, lazy-loaded.
- Retrain weekly; shadow/paper gate; manual promote; auto-rollback.
- Paper-first generic venue path; live adapters deferred until firms are chosen.

## Gotchas

- The exposed HuggingFace and Kaggle tokens are left as-is per user; use only via MCP/CLI and never print or copy them.
- `Design_Document.md` contains outdated Laya/LLM sections; the new Decision Record supersedes them.
- Code exists: 9 Rust crates on branch `phase-0` at `b8870cf` (origin at `b1f5ba7`, ahead 5 pending push). Remaining buildables: Python/Rust golden tests in CI; then Phase 5 dataset builder + `laya-train` with teacher labels. Phase 2 (Rust teacher) is done.
- `sha2` 0.11 `Digest::finalize()` returns `Array<u8, N>` without `LowerHex` — encode to hex with a per-byte `{:02x}` loop.
- `laya-onnx` compiles only with `ort = 2.0.0-rc.13`; `rc.15` changes `Session::builder()` and breaks the crate.
