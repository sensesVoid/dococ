# Doc Oc - Agent Context

Manual snapshot generated from `.agent_context/memory/` because `context-janitor.ps1` is not installed. Keep this file lean; regenerate it when the janitor is available.

## Project

- Rust daemon plus terminal client for autonomous, paper-first trading on funded accounts.
- Workspace: `C:\Users\geral\dococ`.
- Scope: BTC/USDT perpetuals and gold futures; paper both first, live BTC first.
- Laya decides; a deterministic guardian enforces hard risk; an OpenCode-hosted LLM analyzes macro, news, and regime every 15 minutes with no order authority.

## Current state

- Design closed; no code, repo, VPS, or live adapter yet.
- Read `Open_Questions.md`, `Implementation_Checklist.md`, and `Handoff.md` before acting.
- Token decision: leave the existing HuggingFace and Kaggle tokens as-is; they are unused but valid. Use only via MCP/CLI; never print or copy them.
- Development on Windows; production target is a Linux VPS, 8 vCPU/16 GB RAM.

## Key decisions

- Teacher: causal GARCH + Markov + deterministic rules; soft labels; purged walk-forward; teacher must beat costs.
- Actions: LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain; CLOSE = exit all.
- Models: one full checkpoint per asset, lazy-loaded; 8-16 GB RAM.
- State: compact 500-700 tokens.
- Runtime: embedded Rust `laya-onnx` on CPU; Python offline only.
- Retraining: weekly; shadow/paper gate; manual promote; auto-rollback.
- Risk: 0.25% risk per trade, 2% daily loss, 5% max drawdown, 1x notional cap; 2 weeks shadow + 4 weeks paper or 500 decisions.
- Data: free-first.
- SQAC: original notes only; strict time filtering.

## Files

- `Design_Document.md` - original design; the Decision Record supersedes the old Laya/LLM/onboarding sections.
- `Design_Recommendations.md` - full recommendation set.
- `Open_Questions.md` - closed register and locked decisions.
- `Implementation_Checklist.md` - phased build plan and gates.
- `Handoff.md` - current state and resume instructions.
- `.agent_context/memory/*.md` - durable facts, decisions, gotchas.

## Gotchas

- The token exposure is an accepted risk; no rotation blocker.
- Do not train on historical LLM analysis; it can leak the future.
- Laya's context is small; keep the state under budget.
- Full checkpoints are 1.3-1.7 GB each.
- Funded-firm automation permission is a go/no-go.
