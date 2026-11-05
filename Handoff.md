# Doc Oc - Handoff

Date: 2026-11-05
Owner: @Geh
Status: Phase 0 complete; Phase 1 starting
Next session starts at: Pre-start checklist and Phase 0 spikes

## What this project is

Doc Oc is a Rust daemon plus a terminal client for autonomous, paper-first trading on funded accounts. Laya selects the trading action. A deterministic guardian enforces account rules. An OpenCode-hosted LLM analyzes macro, news, and regime every 15 minutes but has no order authority. GARCH and Markov models provide the causal teacher and state features. SQAC supplies time-filtered memory. BTC/USDT perps and gold futures are both in scope; paper both first, live BTC first.

## Current state

- Phase 0 is in progress on branch `phase-0`.
- Rust workspace created and committed; `dococ-laya` runtime wrapper passes tests.
- Multilingual ONNX model exported and verified; Rust/Python parity exact.
- Free-first data sources tested; Binance BTCUSDT 1h data acquired (33,702 bars, no gaps).
- `opencode run` JSON and prompt-injection probes pass with `--standalone`.
- Teacher baseline v1 beats costs (positive net return, positive Sharpe) but needs improvement.
- The HuggingFace and Kaggle tokens in `opencode.json` were exposed during this session. The user has decided to leave them as-is because they are unused but valid. No rotation blocker; use them only via the configured MCP/CLI and never print or copy them.
- `Design_Document.md` now has a Decision Record at the top and updated Laya/LLM text; some older onboarding detail remains and a full rewrite is a Phase 0 documentation task.

## Source of truth

| File | Purpose |
|---|---|
| `Design_Document.md` | Original design; the new Decision Record supersedes the Laya/LLM/onboarding sections. |
| `Design_Recommendations.md` | Full recommendation set and rationale. |
| `Open_Questions.md` | Closed question register, locked decisions, deferred items, token-handling decision. |
| `Implementation_Checklist.md` | Phased build plan with acceptance criteria and gates. |
| `Handoff.md` | This file. |
| `AGENTS.md` | Manual memory snapshot for all coding agents. Regenerate with `context-janitor.ps1` when it is installed. |
| `.agent_context/memory/` | Durable project knowledge: facts, decisions, gotchas. |

## Locked decisions

| Area | Decision |
|---|---|
| Authority | Laya decides and executes; guardian enforces hard risk limits; LLM analyzes every 15 minutes with no order authority. |
| Teacher | Causal GARCH + Markov + deterministic rules; soft labels; purged walk-forward; teacher must beat costs before Laya training. |
| Actions | LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain; CLOSE = exit all. No implicit reversal. |
| Models | One full checkpoint per asset, lazy-loaded; two resident; 8-16 GB RAM. |
| State | Compact 500-700 tokens; position, bars, GARCH, Markov, 1-3 headlines, 2-3 timed SQAC cases, structured LLM brief. |
| Runtime | Embedded Rust `laya-onnx` on CPU; Python offline only. |
| Retraining | Weekly; shadow/paper gate; manual promote; auto-rollback. |
| Venues | Paper-first generic; BTC and gold in scope; live BTC first. |
| Data | Free-first: exchange REST/WebSocket, GDELT, publisher RSS, economic calendar. |
| LLM | OpenCode CLI subprocess; strict JSON; timeout; stale over 30 minutes blocks new entries. |
| Risk | 0.25% risk/trade, 2% daily loss, 5% max drawdown, 1x notional cap; 2 weeks shadow + 4 weeks paper or 500 decisions. |
| SQAC | Original notes only; no proprietary course text; strict time filtering. |
| Deployment | Windows development; Linux VPS 8 vCPU/16 GB for paper/live. Laptop + ngrok for development/paper trading. |

## What is done

- Laya research: model variants, context limits, fine-tuning CLI, calibration, Rust crate, ONNX path.
- Design conflict resolved: LLMs advise, Laya decides, guardian enforces.
- Teacher, state, action, model-management, runtime, retraining, venue, data, LLM, and risk decisions.
- Open-question register closed for design.
- Checklist and handoff written.
- Phase 0: Rust workspace bootstrapped (7 crates), committed on `phase-0`.
- Phase 0: Laya multilingual model exported and verified; Rust/Python parity exact.
- Phase 0: `LayaRuntime` wrapper with `load`, `load_with_threads`, `decide`, and `ActionDecision`.
- Phase 0: Benchmark tooling; single-thread ~5.6 s, 4-thread ~5.2 s per decision, peak RSS ~1.5 GB.
- Phase 0: Free-data spike (Binance, CoinTelegraph RSS, Forex Factory calendar); GDELT rate-limited.
- Phase 0: Binance BTCUSDT 1h data acquired (33,702 bars, no gaps).
- Phase 0: Teacher baseline v1 beats costs (positive net return, positive Sharpe).
- Phase 0: `opencode run` JSON and injection probes pass.

## What is pending

### Blocker

None. The user accepted the token exposure and chose to leave the existing tokens as-is.

### Pre-start

- [ ] Initialize the Rust workspace.
- [ ] Create `AGENTS.md` from `.agent_context/memory/` (janitor missing; create a manual snapshot or install the janitor).
- [ ] Commit the design baseline.

### Phase 0 spikes

- [x] Laya ONNX/Rust parity, latency, and memory (parity PASS; latency ~5.2-5.6 s, peak RSS ~1.5 GB).
- [~] Fine-tune one BTC checkpoint; export ONNX; parity test (export done; fine-tune pending on Kaggle).
- [~] Causal teacher backtest on BTC; cost model (v1 beats costs; needs improvement in Phase 1).
- [x] `opencode run` JSON reliability and prompt-injection test.
- [x] Free-first data and news ingestion.
- [ ] Linux VPS benchmark (guide written; provisioning needs user action).

### External decisions deferred to live gating

- [ ] Which funded firms for crypto and gold.
- [ ] Which crypto exchange and gold broker/API.
- [ ] Written confirmation that API automation is allowed.
- [ ] Exact firm rulebooks: drawdown, daily loss, news, consistency, copy-trading.

## Next five actions

1. Run the Laya benchmark on the Linux VPS to confirm latency and memory.
2. Fine-tune a BTC checkpoint with `laya-train` using the v1 teacher labels.
3. Improve the teacher (better regime model, longer features) before Laya training.
4. Implement the deterministic risk guardian (`dococ-risk`).
5. Build the venue trait and sim venue (`dococ-venue`).

## How to resume

1. Read `AGENTS.md` (if present), `Open_Questions.md`, and `Implementation_Checklist.md`.
2. Check `.agent_context/memory/` for durable facts and decisions.
3. Check the SQAC checkpoint for the last session state.
4. Confirm the token-handling decision: leave as-is; use only via MCP/CLI.
5. Start at the first unchecked item in `Implementation_Checklist.md`.

## Environment

- Development: Windows, current workspace `C:\Users\geral\dococ`.
- Production target: Linux VPS, 8 vCPU/16 GB RAM, 100 GB SSD, low-latency region.
- Rust: stable toolchain; `laya-onnx` with `ort`; CPU execution provider.
- Python: offline only, for `laya-train`, GARCH/Markov fitting, ONNX export, and parity tests.
- OpenCode: CLI subprocess for the 15-minute LLM brief.

## Security notes

- The plaintext HuggingFace and Kaggle tokens in `opencode.json` entered this session. The user accepted the risk and chose to leave them as-is; do not print or copy them.
- Never send balances, account identifiers, or API keys to the LLM.
- News is untrusted data; the LLM gets no tools and must return validated JSON.
- Keep trading keys withdrawal-disabled and IP-allowlisted where possible.
- Do not store secrets in `.agent_context/memory/` or `AGENTS.md`.

## Known risks

- The teacher may have no cost-adjusted edge. If so, Laya will learn noise; fix the teacher before training.
- LLM analysis of historical news can leak the future. Keep LLM briefs out of historical training; use deterministic features.
- CPU inference with two 1.3-1.7 GB checkpoints may be tight on 16 GB; benchmark before committing.
- Laya's context is small; the state template must stay within budget.
- Funded-program rules may forbid API automation; verify before building live adapters.
- Paper-first generic means live adapters are intentionally deferred; do not mistake paper results for live readiness.

## Design-document status

- A Decision Record is at the top of `Design_Document.md` and supersedes the old "Laya, LLMs and SQAC" and "Asset onboarding" sections.
- The Laya section and key onboarding text were updated; the open-questions section now points to `Open_Questions.md`.
- A full rewrite of the remaining outdated details is a Phase 0 documentation task.

## Contact

Owner: @Geh
Workspace: `C:\Users\geral\dococ`

## 2026-11-05 registry fix
- Fixed ModelRegistry len() brace and sha2 0.11 hex encoding; verify_sha256 + tests green.
- cargo test --workspace: 125 passed, 1 ignored, 0 failed.

