# Doc Oc - Open Questions Register

Status: CLOSED. Token handling decided by the user: leave the existing tokens as-is.
Date: 2026-11-05
Supersedes: the "Open questions and unverified assumptions" section of `Design_Document.md`.

## Locked decisions

| ID | Decision | Answer |
|---|---|---|
| L1 | Trade authority | Laya selects and executes the action. The deterministic guardian enforces hard account rules and can reject, cap, or flatten. The LLM analyzes macro, news, and regime every 15 minutes with no order authority. |
| L2 | Teacher signal | Causal GARCH + Markov + deterministic rules. Soft `gold` labels; purged, embargoed walk-forward validation; the teacher must beat costs before Laya training. |
| L3 | Actions | LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain/no trade; CLOSE = exit all. No implicit reversal. |
| L4 | Per-asset models | One full `laya-train` checkpoint per asset, lazy-loaded with an LRU cache. Two assets resident for the first build; 8-16 GB RAM. |
| L5 | Laya state | Compact 500-700 token template: position, bars, GARCH, Markov, 1-3 headlines, 2-3 time-filtered SQAC cases, structured LLM brief. Use multilingual or typed-decisions. |
| L6 | Runtime | Embedded Rust `laya-onnx` on CPU. Python only for offline training and ONNX export. |
| L7 | Retraining | Weekly retrain; shadow/paper gate; manual promote; auto-rollback on drift or rule breach. |
| L8 | Venue path | Paper-first generic. BTC/USDT perps and gold futures are both in scope; live BTC first, then gold. Live adapters wait for the funded firms and APIs. |
| L9 | Data and news | Free-first: exchange REST/WebSocket for BTC, broker/CME or delayed gold data, GDELT + publisher RSS + economic calendar for news. |
| L10 | LLM transport | OpenCode CLI subprocess (`opencode run`) with a strict JSON prompt; timeout, validation, and a stale-brief fallback. |
| L11 | Risk defaults | Conservative: 0.25% equity risk per trade, 2% daily loss, 5% max drawdown, 1x notional cap. Promotion: 2 weeks shadow + 4 weeks paper or 500 decisions, whichever is longer. |
| L12 | SQAC content | Original notes, public docs, and trade cases only. Do not index proprietary QuantAlgo/course text without written permission. |
| L13 | Deployment | Windows is the development environment. The production target is a Linux VPS, 8 vCPU/16 GB RAM, before paper trading. |
| L14 | Token handling | Leave the existing HuggingFace and Kaggle tokens as-is; they are unused but valid. Use only via the configured MCP/CLI. No rotation. |

## Resolved technical questions

| Question | Resolution |
|---|---|
| Can Laya export to ONNX for native Rust inference? | Yes. The official `laya-onnx` crate (imported as `laya`) runs ONNX through `ort`; CPU by default, CUDA/DirectML features. Python is offline only. |
| Does Laya support adapters or head-only training? | Full `laya-train` fine-tuning is supported. `--freeze-encoder` trains the head only, but artifacts still bundle the encoder and adapter sharing is undocumented. Full checkpoint per asset is the critical path. |
| What is Laya's output contract? | `choice` over five actions, `noul` for edge/abstention, `score` for conviction. Use `answer_confidence`, not `action.act_probability`. Calibrate before use. |
| What is the context budget? | English Laya is 512 tokens with roughly 320 for state; multilingual and typed-decisions are 1024 with roughly 768 for state (multilingual up to 8192). The compact state targets 500-700 tokens. |
| How is Python-Rust parity enforced? | Golden fixed-state tests; Python and Rust outputs must match within tolerance; fail closed on mismatch. |
| How is calibration enforced? | Temperature scaling on a held-out fold; ECE/Brier gates; no uncalibrated checkpoint reaches paper. |
| How is lookahead avoided with the LLM? | Historical training uses deterministic, reconstructable features only. LLM briefs are live-only and collected forward for later retraining. Never backfill with an LLM that knows the future. |
| How are SQAC memories time-filtered? | Every retrieved case must have `timestamp < decision_time`; the adapter rejects future cases. |
| How is audit handled? | Append-only decision journal with state hash, model version, prompt hash, action, confidence, guardian result, and fill. Deterministic replay. |
| How is the system stopped? | Per-account pause, global kill, optional flatten; the kill switch works when the strategy layer is wedged. |
| How are secrets handled? | OS keyring or `age`-encrypted file. Trading-only keys, withdrawals disabled. No secrets in logs, prompts, or memory files. |
| How is prompt injection handled? | News is untrusted data. The LLM gets no tools, must return strict JSON, and every field is validated. |
| Are QuantAlgo materials licensed for SQAC? | No proprietary indexing. Use original notes, public docs, and trade cases only unless written permission exists. |
| Which first strategy passes the gate? | The causal GARCH/Markov teacher. Laya must beat the teacher, always-HOLD, and random after costs. |
| Will the engine run on a VPS? | Yes, Linux VPS for paper/live. Windows is development only. |

## Deferred to Phase 0 or a named gate

These are tasks, not unresolved design questions.

| Item | Gate |
|---|---|
| Multilingual vs typed-decisions checkpoint | Phase 0 benchmark. |
| CPU cold-load, latency, and RAM numbers | Phase 0 benchmark; target under 2 seconds per asset per cycle. |
| Exact GARCH family, Markov states, entry/exit rules, label horizon | Phase 0 teacher design and backtest. |
| `opencode run` JSON reliability and latency | Phase 0 LLM spike. |
| Gold contract specs, roll rules, and live broker adapter | Live adapter phase, after the firm/broker is chosen. |
| Funded firm rulebooks, automation permission, copy-trading limits | Live gating; go/no-go. |
| Forward LLM brief dataset and retraining trigger | After paper; 3 months or 5,000 decisions. |
| VPS provisioning and region selection | Before paper trading. |
| Dashboard polish | After `dococ daemon status` and `dococ report`. |
| Full rewrite of the outdated design-doc sections | Phase 0 documentation task; the Decision Record already supersedes them. |

## Token decision (user, 2026-11-05)

The HuggingFace and Kaggle tokens in `C:\Users\geral\.config\opencode\opencode.json` were exposed during this session. The user has decided to leave them as-is because they are unused but valid.

- No rotation is required.
- Use the tokens only through the configured MCP/CLI integrations.
- Do not print, copy, or paste the token values into chat, logs, prompts, or memory files.
- Revisit this decision if the config is shared or the tokens are granted broader scopes.

## Exit criteria

- [x] All design questions are RESOLVED or DEFERRED to a named gate.
- [x] Authority, action semantics, teacher, model management, state, runtime, retraining, venues, data, risk, and SQAC are decided.
- [x] Checklist and handoff created.
- [x] Design-document Decision Record added.
- [x] Memory-keeper records written.
- [x] SQAC checkpoint written.
- [x] Token handling decided: leave as-is; user accepts the exposure.
- [x] Token decision confirmed; implementation can start.
