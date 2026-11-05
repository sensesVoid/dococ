## Facts

- Doc Oc is a Rust daemon plus a terminal client for autonomous, paper-first trading on funded accounts.
- Workspace: `C:\Users\geral\dococ`.
- Scope: BTC/USDT perpetuals and gold futures; paper both first, live BTC first.
- Runtime: embedded Rust `laya-onnx` on CPU; Python only for offline training and ONNX export.
- LLM: OpenCode CLI subprocess every 15 minutes; analyzes macro, news, and regime; no order authority.
- State: compact 500-700 tokens with GARCH, Markov, news, and time-filtered SQAC cases.
- Development: Windows. Production: Linux VPS, 8 vCPU/16 GB RAM.
- Key files: `Design_Document.md`, `Design_Recommendations.md`, `Open_Questions.md`, `Implementation_Checklist.md`, `Handoff.md`.

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
- No code, repo, VPS, or live adapter exists yet.
