## Facts

- Doc Oc is a Rust daemon plus a terminal client for autonomous, paper-first trading on funded accounts.
- Workspace: `C:\Users\geral\dococ`.
- Scope: BTC/USDT perpetuals and gold futures; paper both first, live BTC first.
- Runtime: embedded Rust `laya-onnx` on CPU; Python only for offline training and ONNX export.
- LLM: OpenCode CLI subprocess every 15 minutes; analyzes macro, news, and regime; no order authority.
- State: compact 500-700 tokens with GARCH, Markov, news, and time-filtered SQAC cases.
- Development: Windows. Production: Linux VPS, 8 vCPU/16 GB RAM.
- Key files: `Design_Document.md`, `Design_Recommendations.md`, `Open_Questions.md`, `Implementation_Checklist.md`, `Handoff.md`.
- Build state (2026-11-05): Phase 1 built across 8 crates; `cargo test --workspace` 125 passed, 1 ignored (`model_smoke`), 0 failed; branch `phase-0` pushed to `origin/phase-0` (`119da52`). Next: Phase 2 Rust teacher (GARCH/Markov ports).

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
- Code exists: 8 Rust crates on branch `phase-0` (pushed to origin). Remaining buildables: `spawn_blocking` engine wiring, atomic model switch, Python/Rust golden tests, 20-run LLM latency check; then Phase 2 Rust teacher.
- `sha2` 0.11 `Digest::finalize()` returns `Array<u8, N>` without `LowerHex` — encode to hex with a per-byte `{:02x}` loop.
- `laya-onnx` compiles only with `ort = 2.0.0-rc.13`; `rc.15` changes `Session::builder()` and breaks the crate.
