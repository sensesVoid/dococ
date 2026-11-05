# Doc Oc - Agent Context

Manual snapshot generated from `.agent_context/memory/` because `context-janitor.ps1` is not installed. Keep this file lean; regenerate it when the janitor is available.

## Project

- Rust daemon plus terminal client for autonomous, paper-first trading on funded accounts.
- Workspace: `C:\Users\geral\dococ`.
- Scope: BTC/USDT perpetuals and gold futures; paper both first, live BTC first.
- Laya decides; a deterministic guardian enforces hard risk; an OpenCode-hosted LLM analyzes macro, news, and regime every 15 minutes with no order authority.

## Current state

- Phase 2 and Phase 3 close-out done; Phase 5 (training pipeline) is next. 9 Rust crates implemented (`dococ-core`, `dococ-risk`, `dococ-oms`, `dococ-venue`, `dococ-engine`, `dococ-cli`, `dococ-laya`, `dococ-intel`, `dococ-quant`) with 151 tests passing, 1 ignored (`model_smoke`).
- `dococ-quant` implements GARCH + Markov (Python parity), regime hysteresis/stale guards, teacher policy with backtest parity, and dynamic cost model. Engine now wires `spawn_blocking` with timeout fallback to NO_ACTION; registry has `atomic_switch`/rollback.
- Laya ONNX parity verified. Teacher v1 beats costs. Kaggle fine-tune pipeline validated (smoke v6 PASS).
- `dococ-intel` implements the `opencode run` subprocess adapter, brief schema, news sources (RSS, exchange announcements, economic calendar, GDELT throttle), SQAC time-filtered retrieval, and audit log.
- Branch `phase-0` at `b8870cf` (was pushed at `119da52`); one uncommitted checklist sync pending commit+push.
- VPS deferred to laptop+ngrok for development.
- Tokens left as-is per user decision.

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
- `Implementation_Checklist.md` - phased build plan with acceptance criteria and gates.
- `Handoff.md` - current state and resume instructions.
- `.agent_context/memory/*.md` - durable facts, decisions, gotchas.

## Gotchas

- The token exposure is an accepted risk; no rotation blocker.
- Do not train on historical LLM analysis; it can leak the future.
- Laya's context is small; keep the state under budget.
- Full checkpoints are 1.3-1.7 GB each.
- Funded-firm automation permission is a go/no-go.
- Laya inference takes ~5.5s on CPU, not 100-450ms as assumed.
- GDELT requires 1 req/5s rate limiting.

## House Rules (MUST FOLLOW)

### Skills
- At session start, invoke `using-superpowers` to establish skill usage.
- Before creative work (features, components, behavior changes), invoke `brainstorming`.
- Before executing a plan, invoke `executing-plans` or `subagent-driven-development`.
- Before any code change, invoke `test-driven-development` (RED-GREEN-REFACTOR).
- Before claiming work complete, invoke `verification-before-completion`.
- When debugging, invoke `systematic-debugging`.
- When receiving code review, invoke `receiving-code-review`.
- When finishing a branch, invoke `finishing-a-development-branch`.
- Use `memory-keeper` for durable facts, decisions, and gotchas.
- Use `find-skills` to discover installable skills when needed.

### Memory (SQAC)
- At session start, call `mem_bootstrap` once to load working context.
- Record meaningful turns with `mem_observe(role, text)`.
- At task boundaries, call `mem_checkpoint(goal, summary)`.
- Search durable knowledge with `mem_search`; recall recent work with `mem_recall`.
- Promote stable facts with `mem_graduate`; keep memory sparse with `mem_sparsify`.
- Never fabricate memory; say "I don't have that in memory" if nothing is found.

### Checklist
- Mark completed items in `Implementation_Checklist.md` with `[x]`.
- Mark in-progress items with `[~]`.
- Update `Handoff.md` at task boundaries.
- Commit after each logical change.

### Git
- Work on feature branches, not main.
- Never start implementation on main/master without explicit consent.
- Use `using-git-worktrees` for isolated workspaces when needed.