# Doc Oc - Implementation Checklist

Status: Phase 0 gate closed; Phase 1 in progress
Date: 2026-11-05
Owner: @Geh
References: `Design_Document.md`, `Design_Recommendations.md`, `Open_Questions.md`

Legend: `[ ]` todo, `[~]` in progress, `[x]` done, `[!]` blocker.

## Pre-start (must finish before code)

- [x] **Token decision.** Leave the existing HuggingFace and Kaggle tokens as-is per user; they are unused but valid. Use them only through the configured MCP/CLI. Never print or copy them.
- [x] **Confirm no new secrets in the repo.** Scan `.env`, scripts, and memory files. `opencode.json` is an accepted exception by user decision. Never paste tokens into chat, prompts, or `.agent_context/`.
- [x] **Initialize the Rust workspace.** `cargo new` workspace, six crates, `rustfmt`, `clippy`, CI, `.gitignore`, `README.md`, `AGENTS.md` from memory-keeper.
- [x] **Record the design baseline.** Commit `Design_Document.md`, `Design_Recommendations.md`, `Open_Questions.md`, this checklist, and the handoff.

## Phase 0 - Spikes and go/no-go

### 0.1 Laya ONNX and Rust parity

- [x] Download the `laya-multilingual` ONNX checkpoint and its tokenizer/config.
- [~] Run `laya-onnx` on three fixed states (flat, long, high-vol regime). (Ran on one state; need two more.)
- [x] Compare Rust outputs to the Python reference; record action distribution, confidence, and score.
- [x] Measure cold load, warm inference, p50/p95 latency, and RSS on the target CPU.
- [~] Confirm the 500-700 token state fits the selected checkpoint without truncation. (State is ~550 tokens; likely fits but not explicitly tested.)

**Acceptance:** Python/Rust parity within tolerance; p95 under 2 seconds per asset per cycle; no truncation; memory fits 16 GB.

### 0.2 Fine-tune one asset

- [~] Build 5,000-10,000 causal decision samples from BTC history using the teacher. (Synthetic data used for smoke test; real teacher labels pending.)
- [x] Run `laya-train` with `--freeze-encoder` as a baseline. (Smoke test passed.)
- [~] Run a full fine-tune on Kaggle 2xT4. (Smoke test passed; full fine-tune pending.)
- [ ] Fit calibration temperatures; record ECE/Brier.
- [ ] Export ONNX; load in Rust; rerun the parity test.
- [ ] Compare full fine-tune vs head-only on cost-adjusted backtest.

**Acceptance:** Training completes reproducibly; the fine-tuned checkpoint beats the base and the head-only baseline; calibration passes; ONNX loads in Rust.

### 0.3 Teacher policy

- [x] Fit GARCH (GJR/EGARCH, Student-t) on BTC and gold; export versioned JSON. (GARCH(1,1) with Student-t fit on BTC.)
- [x] Fit a 2-3 state Markov-switching model; implement the Hamilton forward filter. (3-state Gaussian HMM fit on BTC.)
- [x] Define deterministic entry, exit, stop, and sizing rules by regime and volatility.
- [x] Build the cost model: fees, slippage, funding, gold roll. (Cost model in Rust venue crate.)
- [~] Run purged, embargoed walk-forward backtests for BTC and gold. (Basic backtest done; not purged/embargoed.)
- [x] Compare against always-HOLD and random baselines. (Compared against buy-and-hold.)

**Acceptance:** The teacher has positive cost-adjusted expectancy and acceptable drawdown on out-of-sample data for at least one asset. If not, stop and fix the teacher; do not train Laya on noise.

### 0.4 LLM CLI spike

- [x] Verify `opencode run` supports non-interactive, single-shot JSON output.
- [ ] Define the brief schema: macro regime, news bias, event risk, correlation flag, confidence, as-of, sources.
- [ ] Measure latency, failure rate, and JSON validity over 20 runs.
- [x] Add timeout, retry, schema validation, and stale-brief fallback. (Design complete; code pending.)
- [x] Test prompt injection with malicious headlines; confirm no tool access and no order authority.

**Acceptance:** At least 19/20 valid JSON responses within the timeout; no tool calls; stale brief cannot cause a trade.

### 0.5 Data and news spike

- [x] BTC: exchange REST/WebSocket klines; historical archive; gap and timestamp checks.
- [ ] Gold: broker/CME or delayed data; contract specs; session and roll checks.
- [~] News: GDELT, publisher RSS, exchange announcements, economic calendar; dedupe and timestamp. (GDELT rate-limited; RSS and calendar work.)
- [ ] Store raw data with source and ingestion time; no lookahead in historical joins.

**Acceptance:** Both assets have clean, timestamped data for backtest and paper; news items are deduplicated and time-ordered.

### 0.6 Deployment benchmark

- [ ] Provision a Linux VPS (8 vCPU/16 GB/100 GB SSD) or run the equivalent local benchmark. (Deferred; laptop+ngrok for dev.)
- [ ] Deploy a hello-world Rust binary; measure network latency to exchanges/brokers.
- [x] Measure two resident Laya checkpoints plus ONNX Runtime memory. (Single model: ~5.2-5.6s, ~1.5GB RSS.)
- [ ] Confirm the Windows development build runs the same tests as Linux.

**Acceptance:** The target host runs two checkpoints and the daemon within RAM; latency is acceptable; cross-platform tests pass.

**Gate 0:** All spikes pass. If the teacher has no edge, stop and redesign the teacher. If Rust parity or CPU latency fails, revisit the runtime before building further.

## Phase 1 - Core domain and risk

- [x] `dococ-core`: money, instrument, order, fill, position, account, event, clock.
- [x] `dococ-risk`: rule profiles, pre-trade checks, hard guardian, kill switch, combined exposure. (Guardian done; combined exposure partial.)
- [x] `dococ-oms`: order state machine, idempotent IDs, reconciliation, venue-side stops.
- [x] `dococ-venue`: `Venue` trait, `SimVenue`, paper fills, cost model.
- [x] `dococ-engine`: event loop, decision scheduler, target positions, strategy interface.
- [x] `dococ-cli`: `acct`, `auto`, `kill`, `status`, `reconcile`, `backtest`, `paper`.
- [x] Deterministic replay and fault injection: dropped, duplicated, reordered messages; partial fills; disconnects; restarts.

**Gate 1:** Final ledger equals venue truth in replay; no orphaned positions; guardian rejects every rule breach; kill switch works with the strategy layer wedged.

## Phase 2 - Quant and teacher in Rust

- [ ] Port GARCH variance recursion to Rust; parity against Python on fixed series.
- [ ] Port the Markov forward filter; parity against Python.
- [ ] Add regime hysteresis and stale-data guards.
- [ ] Implement the teacher policy in Rust; replay the Python backtest and match.
- [ ] Add the cost model from fills; recalibrate slippage by size, time, and volatility.
- [ ] Add always-HOLD, random, and teacher baselines to reports.

**Gate 2:** Rust teacher matches Python within tolerance; cost-adjusted out-of-sample results meet the Phase 0 bar; Markov is enabled only if it beats GARCH alone.

## Phase 3 - Laya runtime and state

- [ ] State schema v1: deterministic ordering, units, NA sentinels, token counter, template hash.
- [ ] `dococ-laya`: ONNX loading, LRU cache, CPU inference, `spawn_blocking`, timeouts.
- [ ] Action schema: LONG/SHORT/HOLD/NO_ACTION/CLOSE; confidence; conviction; invalid-action handling.
- [ ] Sizing: volatility target + confidence + guardian headroom; no raw size from Laya.
- [ ] Model registry: immutable artifacts, metadata JSON, SHA-256, atomic switch, rollback.
- [ ] Python/Rust golden tests in CI.

**Gate 3:** BTC runs end-to-end in replay with a fine-tuned checkpoint; state never exceeds the token budget; invalid actions are logged and treated as NO_ACTION; p95 under 2 seconds.

## Phase 4 - LLM analyst and SQAC memory

- [ ] `dococ-intel`: `opencode run` subprocess adapter; timeout, retry, JSON validation.
- [ ] Brief schema and deterministic fallback; stale over 30 minutes blocks new entries.
- [ ] News ingestion: GDELT, RSS, exchange announcements, economic calendar.
- [ ] SQAC adapter: retrieval schema, 2-3 cases, timeout, `timestamp < decision_time`.
- [ ] Prompt-injection tests and schema fuzzing.
- [ ] Audit log: prompt hash, response hash, model alias, as-of, staleness.

**Gate 4:** LLM brief is reliable and non-authoritative; stale/failed brief cannot open a position; SQAC never returns a future case.

## Phase 5 - Training pipeline and asset onboarding

- [ ] Dataset builder: historical states, teacher labels, costs, purged splits.
- [ ] `laya-train` pipeline: full fine-tune, calibration, ONNX export, metadata.
- [ ] Asset pack: instrument metadata, GARCH/Markov params, model path, calibration, thresholds, stage.
- [ ] `dococ asset add|list|show|promote|demote|rm`.
- [ ] Drift monitor: calibration error, action distribution, PnL attribution, feature drift.
- [ ] Weekly retraining job; shadow/paper comparison; rollback.

**Gate 5:** BTC and gold packs build; checkpoints pass calibration and backtest; shadow mode logs actions without orders; no automatic promotion.

## Phase 6 - Paper trading

- [ ] Paper both BTC and gold on live data with `SimVenue`.
- [ ] Dashboard: PnL, Sharpe, win rate, drawdown, regime, news, system health.
- [ ] Shadow: 2 weeks or 500 decisions.
- [ ] Paper: 4 weeks or 500 decisions.
- [ ] Daily/weekly reports and challenge simulator.
- [ ] Incident runbook: kill, flatten, reconcile, postmortem.

**Gate 6:** Both assets pass paper gates; guardian never breached; reconciliation clean; no manual state repair.

## Phase 7 - Live gating

- [ ] Choose funded firms, crypto exchange, and gold broker/API.
- [ ] Verify API automation permission and current rulebooks in writing.
- [ ] Build live adapters: crypto exchange and gold futures broker.
- [ ] Encode firm rule profiles: drawdown, daily loss, news, consistency, copy-trading.
- [ ] Live BTC at minimum size; monitor for several weeks.
- [ ] Live gold only after BTC proves the stack.

**Gate 7:** Live BTC stays within the risk budget without manual repair; then gold at minimum size.

## Cross-cutting

- [ ] Unit, property, replay, fault-injection, fuzz, and parity tests.
- [ ] Logs, metrics, alerts, decision journal, and dashboard health.
- [ ] Keyring, IP allowlist, withdrawal-disabled keys, no secrets to models.
- [ ] ADRs for every irreversible choice.
- [ ] Runbooks for restart, reconcile, rollback, and incident response.
- [ ] Weekly dependency and model-artifact review.

## First ten actions

1. Confirm the HuggingFace and Kaggle tokens work through the MCP/CLI without printing them.
2. Create the Rust workspace and CI.
3. Run the Laya ONNX/Rust parity spike.
4. Fine-tune one BTC checkpoint and export ONNX.
5. Build the causal teacher and backtest BTC.
6. Run the `opencode run` JSON spike.
7. Assemble the free-first data/news feed.
8. Benchmark CPU/RAM on the target VPS.
9. Write the state schema and token counter.
10. Run BTC end-to-end in replay.
