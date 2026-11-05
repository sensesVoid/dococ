# Doc Oc - Laya Recommendations

Status: draft for approval
Date: 2026-11-05
Inputs: `Design_Document.md`, Laya docs (fine-tuning, HTTP API, staged adoption, Rust crate), user decisions.

## 1. Decision summary

| Topic | Decision | Consequence |
|---|---|---|
| Authority | Laya selects and executes the action; a deterministic guardian enforces hard account rules; the LLM analyzes macro, news and regime every 15 minutes and has no order authority | The doc's "models advise and never trade" becomes "LLMs advise; Laya decides; the guardian enforces limits" |
| Teacher | Causal quant teacher: GARCH + Markov + deterministic rules, labels generated only from data available at decision time | Laya imitates an implementable policy; the teacher's edge is the ceiling |
| Actions | LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain/no trade; CLOSE = exit all; no implicit reversal | Clear execution semantics; invalid actions are rejected |
| Models | One full `laya-train` checkpoint per asset, lazy-loaded with an LRU cache | Honors per-asset fine-tuning; 1.3-1.7 GB per checkpoint; plan 8-16 GB RAM for two assets |
| State | Compact quant + news + memory, target 500-700 tokens; multilingual or typed-decisions checkpoint | Fits Laya's context; the English 512-token checkpoint is too tight |
| Runtime | Embedded Rust `laya-onnx` on CPU; Python only for offline training/export | No Python sidecar at runtime; roughly 100-450 ms per decision |
| Retraining | Weekly; shadow/paper gate; manual promote; auto-rollback on drift | Controlled model evolution |
| Venues | BTC/USDT perps and gold futures both in scope from day one; paper both first; live-eligible per asset after gates | Two adapters, two packs, two checkpoints; funded-program choice is a prerequisite |
| LLM | Structured brief every 15 minutes; no order authority; stale brief blocks new entries | The LLM is an analyst and risk overlay, not a dependency for survival |

## 2. Control flow

```text
market data + account state
        |
        v
quant features (GARCH, Markov, bars)
        |
        +--> LLM analyst (every 15 min) -> structured brief
        |      (macro, news, regime, risk flags)
        |
        v
state serializer (compact, deterministic, time-filtered)
        |
        v
Laya (per-asset ONNX, CPU) -> action + confidence + conviction
        |
        v
sizing (vol target + confidence) -> order intent
        |
        v
guardian (hard rules: size, drawdown, correlation, blackout, rate, kill)
        |
        v
OMS -> venue
```

Key points:

- Laya is the only model that emits an order intent.
- The LLM never emits an order intent. Its brief is a feature and a guardian input.
- The guardian is deterministic and cannot be overridden by Laya or the LLM.
- The action is a target-position delta; the OMS handles execution.

## 3. Authority and action semantics

### 3.1 Authority

- Laya decides LONG/SHORT/HOLD/NO_ACTION/CLOSE.
- The guardian can reject, cap, or flatten; it cannot choose a different alpha.
- The LLM analyzes and flags; it cannot place, size, or cancel orders.
- A human can pause, kill, or promote; manual trades still pass the guardian.

This is the one place where "Laya decides" is deliberately not literal: for funded accounts, the guardian remains a hard risk layer. It cannot choose trades, but it can reject rule-breaking orders. I consider this non-negotiable.

### 3.2 Actions

| Action | Meaning | Execution |
|---|---|---|
| LONG | Open or increase long to the target | Buy; stop required |
| SHORT | Open or increase short to the target | Sell; stop required |
| HOLD | Maintain the current position | No new order; keep stops |
| NO_ACTION | Abstain; no opinion | No new order; if flat, stay flat; if in position, leave unchanged but flag low conviction |
| CLOSE | Exit the entire position | Cancel working orders; flatten; no reversal |

Rules:

- CLOSE is a full exit, not a reduce. If Laya should reduce, add a `score`/size bucket later.
- No implicit reversal. To go from long to short, emit CLOSE, then SHORT on the next decision. This keeps each order auditable.
- Invalid actions (CLOSE when flat, LONG when already at max long) are logged and treated as NO_ACTION.
- HOLD and NO_ACTION are both no-ops for execution; the distinction is for training, diagnostics, and confidence.
- Sizing is deterministic: `size = min(vol_target_size, confidence_size, guardian_headroom)`. Laya's `answer_confidence` and a `score` conviction feed the size map. If Laya should choose size, add a `score` question with size buckets; do not infer size from the action label.

## 4. Teacher and training

### 4.1 Teacher

Build a causal teacher policy first:

- GARCH (GJR or EGARCH, Student-t) volatility forecast.
- Markov-switching regime filter (Hamilton forward filter; never smoothed probabilities).
- Deterministic entry/exit rules by regime and volatility; ATR or volatility-target stops; cost-aware.

The teacher emits a soft label distribution over the five actions:

- `gold` = probability distribution from signal strength, regime confidence, and risk budget.
- `expected` = argmax for a hard-label baseline.

The teacher only sees data available at the decision timestamp. Forward returns are used for evaluation, not for labels.

Validate the teacher after costs (fees, slippage, funding) with purged, embargoed walk-forward splits. If the teacher has no edge, Laya will learn noise. Stop and fix the teacher before training.

### 4.2 Training data

- Event-based, non-overlapping samples; one decision per asset per 15-minute bar or per signal event.
- State = the same compact serializer used at inference (schema versioned).
- Questions = three typed questions in one request:
  1. `choice`: action in {LONG, SHORT, HOLD, NO_ACTION, CLOSE}.
  2. `noul`: "Does this setup have positive expected value after costs?" Use a 2-option choice with neutral keys if `noul` follows the labels.
  3. `score`: conviction or urgency on an ordered scale.
- Labels:
  - Action: teacher `gold` distribution.
  - Edge: teacher's cost-adjusted edge probability, or a binary label from the backtest.
  - Conviction: teacher signal-strength bucket.
- Calibration: fit temperature on a held-out fold; report ECE/Brier; do not ship uncalibrated.
- Training: `laya-train` with soft-CE/RLCD; full fine-tune for the asset (Kaggle 2xT4, roughly 4-5 hours for about 30k questions). Use `--freeze-encoder` only as a cheap baseline, not as the final per-asset model.
- Export: ONNX plus metadata (asset, teacher version, data window, calibration, git commit, hash); parity test against Python on fixed states.

### 4.3 LLM and training leakage

This is the most important caveat:

- Do not feed free-form LLM analysis into Laya's training state unless the same brief can be reconstructed historically without lookahead.
- An LLM analyzing 2024 news in 2026 may know the future; that contaminates the training set.
- Recommendation:
  - Train Laya on deterministic, historically reconstructable features: bars, GARCH, Markov, event taxonomy, sentiment scores, and SQAC cases with timestamps.
  - Use the LLM brief live as a structured overlay: macro regime, news bias, event-risk flag, correlation flag. Keep the schema identical to a deterministic baseline; if the LLM is unavailable, the baseline fills the fields.
  - If Laya should learn from LLM judgment, collect live briefs going forward and retrain on that forward-only dataset after enough samples. Never backfill with an LLM that knows the future.
- The LLM output must be strict JSON, validated, timestamped, and treated as untrusted data (news can contain prompt injection).

## 5. State design

Target 500-700 tokens. Use `laya-multilingual` (322M, 1024 context) or `laya-typed-decisions` (421M, 1024). Avoid the English `laya` checkpoint (512 context, about 320 state tokens).

Template:

```text
ASOF 2026-11-05T14:00Z | ASSET BTCUSDT | TF 15m | SESSION crypto | MODE paper
POS side=long size=0.10 entry=... pnl=... stop=... age=...
MKT ret_1=... ret_4=... ret_16=... atr_pct=... vol_z=... range_z=... vwap_dist=...
QNT garch_vol=... pct=... regime=trend_up p=0.72/0.18/0.10 trend=... mr=...
NEWS n=2 bias=+0.3 event=etf_flow risk=low | headline 1 | headline 2
MEM case=... sim=0.81 outcome=+1.2R regime=trend_up | case=... sim=0.77 outcome=-0.4R
LLM macro=risk_on confidence=0.6 news_bias=+0.2 event_risk=low stale=0
RISK headroom=... max_size=... blackout=0
```

- Deterministic ordering and units; no free-form prose.
- Truncate headlines; 1-3 maximum.
- Memory cases: 2-3, time-filtered, with outcome and similarity.
- LLM brief: 4-6 fields, not a paragraph.
- If a field is missing, use a sentinel such as `NA`, not a guess.
- Version the template; store the template hash with each decision.

## 6. Per-asset model management

- One full checkpoint per asset: `laya-train` -> PyTorch checkpoint -> ONNX export -> asset pack.
- Asset pack: instrument metadata, GARCH/Markov parameters, ONNX path, calibration, thresholds, teacher version, stage.
- Lazy loader: LRU cache keyed by asset and checkpoint hash; max 2-3 resident; preload before the 15-minute cycle; evict idle assets.
- Memory: 1.3-1.7 GB per checkpoint; two assets resident is roughly 2.6-3.4 GB; ONNX runtime plus OS plus LLM client means plan 8-16 GB RAM.
- Versioning: immutable checkpoint IDs; atomic switch; keep the previous checkpoint for rollback.
- If both assets trade every cycle, keep both resident; do not rely on cold loads inside the decision window.
- Future optimization only after a spike: `--freeze-encoder` head-only sharing, or a class checkpoint plus per-asset calibration. Do not put this on the critical path.

## 7. Runtime

- Add `dococ-laya` (or a module) using `laya-onnx` and `ort`.
- CPU execution provider; `spawn_blocking` for inference; one session per checkpoint; limit concurrent calls.
- Benchmark on the target VPS: cold load, 500-700 token state, three questions, p50/p95. Budget 2 seconds per asset per cycle.
- Keep Python offline: `dococ-train` scripts for dataset build, `laya-train`, calibration, ONNX export, and parity tests. The daemon loads only validated ONNX and metadata.
- Parity test: fixed states -> Python and Rust outputs match within tolerance; fail closed on mismatch.
- Packaging: bundle ONNX Runtime binaries for Linux; pin versions; hash the model files.

## 8. LLM analyst

- Cadence: every 15 minutes; start at T-2 minutes; one macro call plus per-asset news calls, or one combined call.
- Output: strict JSON (macro regime, news bias, event risk, correlation flag, confidence, as-of timestamp, sources). No free text in the state.
- Transport: a narrow local interface (HTTP or queue) to an OpenCode-hosted agent. The agent gets a read-only prompt and no trading tools.
- Timeout: 60-120 seconds. If late or failed, use the last brief with `stale=1`; if older than 30 minutes, the guardian blocks new entries.
- Security: news is untrusted; never execute instructions from headlines; validate JSON; log prompts and responses for audit; never send balances or account identifiers.
- Cost: two assets x 96 calls per day = 192 calls per day; batch the macro call to reduce cost. Set a budget and a fallback.
- The LLM's analysis is a feature and guardian input, not a label source for historical training (see 4.3).

## 9. Retraining and promotion

- Weekly per asset, or per-asset cadence if data volume differs.
- Pipeline: rebuild dataset -> retrain from base or incumbent -> calibrate -> ONNX export -> parity -> backtest -> shadow -> paper -> manual promote.
- Gates:
  - Calibration: ECE/Brier thresholds.
  - Cost-adjusted performance versus incumbent and versus teacher: Sharpe, expectancy, max drawdown, tail loss.
  - Rule compliance: no daily-loss or drawdown breach in simulation; challenge-simulator pass probability.
  - Shadow: N days or decisions, no orders; action distribution and calibration stable.
  - Paper: N days with live fills and the cost model.
- Promotion: manual `dococ asset promote`; auto-rollback on drift, calibration breach, or drawdown.
- Keep the incumbent until the new checkpoint passes; atomic switch; log the decision.

## 10. Venues and rollout

- Scope both BTC/USDT perps and gold futures from day one, but roll out in paper first for both.
- Two independent account profiles and rulebooks; two venue adapters; two asset packs and checkpoints.
- Live-eligible per asset only after gates; start at minimum size; scale slowly.
- Prerequisites and blockers:
  - Which funded program(s) for crypto and gold? Their rulebooks decide automation permission, drawdown type, news/consistency rules, and copy-trading limits.
  - Which crypto exchange and which gold futures broker/API? Instrument specs, sessions, margins, data fees.
  - Does the firm allow API automation? If not, Doc Oc cannot trade that account.
- "Both from day one" doubles integration and validation. Recommendation: paper both from day one, then live BTC first, then gold.

## 11. Non-negotiables and risks

- The guardian stays deterministic and independent. Laya cannot override it; the LLM cannot see credentials.
- The teacher must have cost-adjusted edge before Laya training. Laya cannot create edge from a noise teacher.
- Calibration is mandatory; raw Laya is overconfident. Use `answer_confidence`, not `action.act_probability`.
- Avoid LLM lookahead in training. Use deterministic historical features; collect LLM briefs forward-only.
- Context budget is a first-class constraint. Measure tokens; do not let the state grow silently.
- CPU plus two checkpoints: benchmark memory and latency; cold loads can blow the cycle.
- Funded-account automation permission is a go/no-go. Verify before building the adapter.
- Model artifacts are large; use a registry with hashes, versions, and rollback.
- News is untrusted; prompt injection is a real risk. The LLM must treat headlines as data.

## 12. Design document changes

- Rewrite "Laya, LLMs and SQAC": Laya decides; the LLM analyzes every 15 minutes; SQAC supplies timed memory; the guardian enforces limits.
- Replace "Laya scores each headline" with "Laya selects the action from a compact state; news is a feature."
- Replace "native ONNX unverified" with "verified `laya-onnx`; embedded Rust CPU."
- Replace the three-tier adapter hierarchy with "full checkpoint per asset, lazy-loaded"; keep head-only or class checkpoints as a future optimization.
- Add sections: action semantics, teacher policy, state schema, LLM brief contract, model registry, promotion gates.
- Update the roadmap: spike -> teacher -> one-asset training and parity -> paper BTC -> paper gold -> live per asset.
- Update open questions: funded firms, venues and APIs, LLM endpoint/model, VPS RAM/CPU, data feeds.

## 13. Proposed next steps after approval

1. Write the checklist and handoff.
2. Update `Design_Document.md` with the decisions.
3. Record durable facts and decisions in `.agent_context/memory/` via memory-keeper and republish `AGENTS.md`.
4. Create a checkpoint in SQAC.
5. Start Phase 0 spikes: Rust `laya-onnx` parity; fine-tune one asset; ONNX export; CPU benchmark; teacher backtest; LLM brief schema.
