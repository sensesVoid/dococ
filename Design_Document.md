# Doc Oc — Design Document

Nov 5, 2026 · @Geh

## Goals and principles

Doc Oc is a Rust daemon plus a terminal client that trades BTC/USDT, other crypto and gold futures on funded accounts, with risk rules enforced in code that no model can override. Its models can be fine-tuned per asset from the CLI, so new assets are onboarded by a pipeline rather than by new code. The binary is `dococ` and the crates are prefixed `dococ-`; rename freely.

**Goals**

- Manage several funded and personal accounts from one CLI, each with its own rule profile.
- Run automated strategies with a deterministic risk guardian between every signal and every order.
- Show realtime P&L, rule headroom, regime state, Laya action/confidence, and the structured news brief in a terminal dashboard.
- Replay anything: the same engine code runs live, on paper and in backtests.

**Non-goals for v1**

- No high-frequency or latency-arbitrage trading; many funded programs prohibit it.
- No LLM authority over orders, sizing or risk limits.
- No promise of profit. A strategy ships only after it passes its validation gate.

**Principles**

1. Money is fixed-point decimal, never floating point.
2. Fail closed: stale data, a lost feed or a state mismatch stops new risk, and flattens if needed.
3. Protective stops live at the venue, not only inside the bot.
4. Every decision is an event in an append-only log.
5. The bot's own limits sit inside the firm's limits, with a buffer.

## Decision Record - 2026-11-05

This record supersedes the Laya/LLM authority model and the asset-onboarding tiers below. The open-question register was closed 2026-11-05 (locked decisions in the Decision Record above, deferred items in `Implementation_Checklist.md`); the build plan is in `Implementation_Checklist.md`.

| Area | Decision |
| --- | --- |
| Authority | Laya selects and executes the action. A deterministic guardian enforces hard account rules and can reject, cap, or flatten. The OpenCode-hosted LLM analyzes macro, news, and regime every 15 minutes with no order authority. |
| Teacher | Causal GARCH + Markov + deterministic rules. Soft `gold` labels; purged, embargoed walk-forward validation; the teacher must beat costs before Laya is trained. |
| Actions | LONG, SHORT, HOLD, NO_ACTION, CLOSE. HOLD = maintain; NO_ACTION = abstain; CLOSE = exit all. No implicit reversal. |
| Models | One full `laya-train` checkpoint per asset, lazy-loaded with an LRU cache. Start with `laya-multilingual`; benchmark `laya-typed-decisions`. |
| State | Compact 500-700 token template: position, bars, GARCH, Markov, 1-3 headlines, 2-3 time-filtered SQAC cases, structured LLM brief. |
| Runtime | Embedded Rust `laya-onnx` on CPU. Python only for offline training and ONNX export. |
| Retraining | Weekly; shadow/paper gate; manual promote; auto-rollback on drift or rule breach. |
| Venues | Paper-first generic. BTC/USDT perps and gold futures are both in scope; live BTC first, then gold. Live adapters wait for the funded firms and APIs. |
| Data | Free-first: exchange REST/WebSocket, GDELT, publisher RSS, economic calendar. |
| Risk | 0.25% equity risk per trade, 2% daily loss, 5% max drawdown, 1x notional cap. Promotion: 2 weeks shadow + 4 weeks paper or 500 decisions. |
| Deployment | Windows is the development environment. Production target is a Linux VPS, 8 vCPU/16 GB RAM, before paper trading. |

## System overview

&#91;embedded content: architecture · 1 daemon, embedded Laya, 2 advisory sidecars, 2 clients\]

Market data flows left to right; Laya selects the action, the guardian approves, caps, or rejects it, and only the OMS reaches the venue adapters. The LLM and SQAC feed the state and have no order authority. The CLI and dashboard are clients of the daemon, so closing them never stops trading.

## Rust workspace

One cargo workspace of small crates keeps the dangerous parts (risk, order state) pure and testable, and keeps I/O at the edges.

| Crate | Responsibility | Notes |
| --- | --- | --- |
| `dococ-core` | Domain types: money, instrument, order, fill, position, account, event; clock abstraction | No I/O. `rust_decimal`, `serde`, `time` |
| `dococ-risk` | Risk guardian: rule profiles, pre-trade checks, drawdown tracking, kill switch | Pure functions over state. Property-tested |
| `dococ-portfolio` | Target positions in, order deltas out: netting across strategies, risk budgets, correlation caps, volatility targeting | Pure core; every target passes the guardian |
| `dococ-oms` | Order state machine, idempotent client IDs, position ledger, reconciliation | Pure core, driven by events |
| `dococ-venue` | `Venue` trait and adapters: crypto exchange, futures broker, `SimVenue` | `tokio`, `reqwest`, `tokio-tungstenite` |
| `dococ-data` | Feed handling, quality checks, tick-to-bar builders, session and contract-roll calendars | Same builders live and in replay |
| `dococ-regime` | Online GARCH variance recursion; Markov-switching filter once it earns its place | Parameters loaded from offline fits |
| `dococ-strategy` | `Strategy` trait: features in, target positions out | No credentials, no direct orders |
| `dococ-intel` | News ingestion, OpenCode LLM client, SQAC client; Laya ONNX runtime as a module or `dococ-laya` | Advisory context; every external call has a timeout |
| `dococ-assets` | Asset packs and the onboarding pipeline: ingest, statistical fit, Laya tuning, promotion stages | Drives the Python fit and training sidecars |
| `dococ-research` | Challenge simulator, cost model from fills, experiment registry, strategy lifecycle monitor, reports | Offline and monitoring tools; no order authority |
| `dococ-store` | Append-only SQLite event log, Parquet archive for ticks and bars | WAL mode; state rebuilt by replay |
| `dococ-engine` | The daemon: wires all of the above on `tokio`, serves a local socket | One process per host |
| `dococ-sim` | Deterministic replay, backtest and fault injection around `dococ-engine` | Same code path as live |
| `dococ-cli`, `dococ-tui` | `clap` command line and `ratatui` dashboard | Thin clients of the daemon |

The split that matters most: `dococ-risk` and `dococ-oms` never perform I/O, so tests can feed them any event sequence and check the invariants. Other likely dependencies: `tracing` for logs, `proptest` for property tests, `keyring` or `age` for secrets.

**Day-one layout.** Start with six crates (`dococ-core`, `dococ-risk`, `dococ-oms`, `dococ-venue`, `dococ-engine`, `dococ-cli`) and keep the rest as modules inside them. Split a module into its own crate when it gains a second consumer or its build time hurts. The table is the target layout, not the first commit.

## Accounts, rule profiles and the risk guardian

The risk guardian is a pure Rust module that every order intent must pass, whether it comes from a strategy or from you typing a manual trade. It enforces each account's rule profile with a safety buffer inside the firm's limits.

An account binds a venue, a credential reference, a rule profile, a mode (`paper` or `live`) and the strategies allowed to trade it. A rule profile is a versioned TOML file; each profile records which firm rulebook version and date it was written from.

| Rule type | How the guardian enforces it |
| --- | --- |
| Daily loss limit | Tracked from the firm's reset time and timezone; basis (balance or equity including open P&L) is a profile setting |
| Max or trailing drawdown | Static or trailing from the equity high-water mark; end-of-day versus intraday trailing is a profile setting. Headroom is recomputed on every tick |
| Profit target, minimum trading days | Tracked and shown on the dashboard; no effect on entries |
| Consistency rule | Caps any single day's share of total profit by throttling size after a large day |
| Position and leverage limits | Max contracts or lots per instrument and in total; instrument allowlist |
| Sessions and flatten times | Flat before the firm's daily close or weekend cutoff; no entries near it |
| News blackout | Blocks entries, and optionally forces exits, around high-impact calendar events when the firm requires it |
| Banned behaviours | Machine-checkable ones become checks (minimum hold time, order-rate cap, no size escalation after a loss); the rest are documented in the profile |

**Pre-trade checks, in order.** Instrument allowed, then session open, then no blackout, then size within limits, then worst-case loss at the stop (stop distance × size × contract value, plus fees and a slippage allowance) fits inside remaining headroom, then order rate, then combined exposure across accounts. The first failure rejects the intent and logs which rule fired.

**Escalation ladder.** Defaults to tune per profile, each measured as a share of the daily loss limit used:

1. 50%: halve position size.
2. 70%: no new entries; manage existing positions only.
3. 85%: flatten and halt until the next reset.

The drawdown limit has its own ladder. Reaching a step is an event, an alert and a dashboard state change.

**Kill switch.** `dococ kill` (optionally `--flatten`), a flag file and a signal all trip it. It cancels working orders, blocks new intents, and survives a daemon restart until you clear it.

**Multiple accounts.** Each account has independent limits. Some firms restrict copying identical trades across accounts or hedging between them; the guardian exposes combined exposure so you can set a cap, but whether a setup is allowed depends on each firm's terms.

**Portfolio layer.** Above the per-account checks, a portfolio module holds risk budgets per strategy and per asset, caps combined exposure using a rolling correlation estimate, and scales the whole book to a volatility target. Crypto positions are treated as highly correlated until measured otherwise, and gold is not assumed to diversify them.

Profiles ship as templates for you to fill in from each firm's current rulebook. I have not verified any firm's present rules.

## Execution and order management

Orders move through an explicit state machine, carry idempotent IDs, and are reconciled against the venue, so a crash or disconnect cannot leave an unknown position.

- **State machine.** Intent, approved, submitted, acknowledged, partially filled, then filled, canceled or rejected. A lost acknowledgement is its own `unknown` state that must be resolved by querying the venue. An illegal transition is a bug and halts the account.
- **Idempotent IDs.** Client order IDs are deterministic (account, strategy, sequence), so a retry can never create a second order. The engine queries before it ever resends.
- **Venue-side protection.** Entries go out with a resting stop (bracket or OCO where the venue supports it). If an entry cannot be protected at the venue, the guardian rejects it.
- **Reconciliation.** On startup, on a timer, and after every reconnect, positions, working orders and balance are compared with the ledger. Any mismatch stops new risk, alerts you, and needs `dococ reconcile --accept` to resume.
- **Connectivity.** Heartbeats, exponential backoff, and a stale-data detector: no tick for a set time means no new entries. Cancel-on-disconnect is used where the venue offers it.
- **Circuit breakers.** Repeated rejects, or slippage beyond a multiple of recent ATR, pause the strategy.
- **Fat-finger limits.** Maximum order size and a price band around the last trade.
- **Clock.** Clock skew beyond a threshold blocks trading, since daily-loss resets and blackouts depend on time.
- **Order style.** Limit or marketable-limit orders with timeouts; plain market orders only for forced exits.

**Targets, not orders.** Strategies publish a target position per instrument, or a basket for multi-leg ideas such as pairs. The portfolio layer nets targets across strategies, runs them through the guardian, and gives execution a delta. Execution then chooses order types and slicing, and handles netting versus hedge-mode venues. A multi-leg target is approved or rejected as a unit, and legs that cannot both be protected are not sent.

## Venues and market data

Crypto and gold futures sit behind one `Venue` trait, but their quirks live in separate adapters. The funded program usually dictates which platform and API you may use, so the adapter order depends on that choice.

| Aspect | Crypto (BTC/USDT and others) | Gold futures |
| --- | --- | --- |
| Session | 24/7 | Near-continuous with a daily maintenance break and a weekend close |
| Instruments | Spot or perpetuals; perps add funding payments | Full-size and micro contracts with expiries and rolls |
| Costs | Maker and taker fees, funding | Commissions, exchange fees, market-data fees |
| Access | Exchange REST and WebSocket APIs | Broker or prop-firm platform API; availability and automation rules vary by firm |
| Specific risks | Exchange outages, rate limits, liquidation on leveraged products | Roll handling, limit moves, session gaps |

Instrument metadata (tick size, tick value, contract multiplier, lot step, minimum notional) lives in config and is checked against the venue at startup; a mismatch refuses to trade that instrument. Contract specifications, session hours and margins for gold futures need to be confirmed with the exchange and your broker before they go into config.

**Build order.** `SimVenue` first, then one crypto exchange adapter against its testnet, then the futures adapter once you know which firm and platform you will use.

**Data handling.** Ticks and bars are archived to Parquet. Bar builders are deterministic and shared between live and replay, so a backtest sees exactly the bars the live engine would have built.

**Data quality.** Every feed passes gap, duplicate, out-of-order and bad-tick checks before bars are built. Gold series are roll-adjusted and the adjustment method is recorded. A failed check blocks new entries on that instrument and raises an alert.

## Regime engine and strategies

Regime models are fitted offline in Python and executed online in Rust, with parity tests proving the two agree. This keeps the mature statistics libraries for fitting and keeps the live path small, fast and deterministic.

**Build order.** Start with GARCH volatility targeting. Add the Markov filter only once it beats GARCH alone out of sample and its state labels stay stable across refits.

**Offline fit.** `arch` for GARCH-family models (GJR or EGARCH, Student-t errors) and `statsmodels` for Markov switching, on a rolling refit schedule. Each fit exports a versioned JSON file: parameters, data window, fit date and diagnostics.

**Online engine (`dococ-regime`).** The GARCH variance recursion costs one update per bar. The Markov model uses the forward (Hamilton) filter, which uses only past data; smoothed probabilities are never used live because they leak the future. Outputs are a volatility forecast, state probabilities, and a regime label with hysteresis so it does not flip on noise.

**Gates for the models themselves**

- Parity: Rust output matches the Python reference within a tight tolerance on a long fixture series.
- Skill: out-of-sample volatility forecasts beat a naive rolling-volatility baseline.
- Freshness: parameters older than a set age raise an alert and shrink position size.

**Strategy interface.** A strategy receives bars, features, regime state, news flags and its account's headroom (read-only), and returns a target position, or a basket of targets for multi-leg ideas, with a stop level. It never sees credentials and never places orders. Sizing is volatility-targeted from the forecast, then capped by the guardian and the portfolio layer.

**Inputs beyond price.** Candidate features include funding, open interest, liquidations and basis for crypto, and real yields, dollar strength and positioning for gold. Whether they carry more signal than headlines is a hypothesis to test through the same validation gate.

**Candidate strategies, each behind its own gate.** Regime-conditional mean reversion or trend, volatility dislocations, cointegration pairs among liquid cryptos, and funding or basis carry. None is assumed to work.

**Validation gate before any strategy trades live**

- Purged walk-forward testing with costs and slippage included.
- Deflated Sharpe ratio, or a similar multiple-testing correction, still positive.
- Behaviour is acceptable in every regime, not only on average.
- Replaying the full history under each target rule profile never breaches it.

## Challenge simulator and monitoring

A strategy only helps on a funded account if it can pass and survive that firm's rules, so these tools turn its results into numbers about the rules rather than about average return.

- **Challenge simulator.** A block-bootstrap Monte Carlo over paper and backtest returns (blocks keep autocorrelation and regime mix), run against a rule profile. It reports pass probability, expected days to the target, the probability of breaching the daily-loss or drawdown limit, and the distribution of worst days. It can sweep position size to find the largest size whose breach probability stays under a ceiling you set. The result is a distribution, not a promise, and it inherits any bias in its inputs.
- **Cost model from fills.** Paper and live fills update per-asset slippage and fee estimates by order size, time of day and volatility regime. Backtests and the simulator always use the latest model.
- **Experiment registry.** Every strategy variant, parameter sweep and data window is logged with code version, data hash and result. The trial count feeds the deflated Sharpe calculation, so it is counted rather than guessed.
- **Strategy lifecycle monitor.** Each strategy carries an expected-performance envelope from its validation. A sequential test (CUSUM or similar) on live versus expected returns pauses a strategy when it decays; resuming requires revalidation.
- **Reporting.** Attribution by strategy, asset and regime, daily and weekly reports, and tracking against the profit target, trading days and consistency rule.

The dashboard gains a strategy-health panel and a portfolio exposure view from these.

## Laya, LLMs and SQAC

Laya selects the action; the guardian enforces hard limits; the LLM and SQAC supply context. No component can bypass the guardian.

| Component | Job | Where it runs | Authority |
| --- | --- | --- | --- |
| Laya (fine-tuned) | Selects LONG/SHORT/HOLD/NO_ACTION/CLOSE from a compact state; returns action distribution, edge, and conviction | Embedded Rust `laya-onnx`, CPU | Emits the order intent; cannot bypass the guardian |
| LLM via OpenCode | Produces a strict JSON brief every 15 minutes: macro regime, news bias, event risk, correlation, confidence, as-of | OpenCode CLI subprocess from `dococ-intel` | No order authority; feature and guardian input |
| SQAC | Retrieves time-filtered procedure and trade cases; stores the decision journal | Local adapter | Read-only context; no order authority |
| Guardian | Enforces account rule profiles, size, drawdown, correlation, blackout, rate, and kill | Pure Rust in the daemon | Hard veto, cap, or flatten; cannot be overridden by a model |

**Rules of engagement**

- Laya is fine-tuned on labels from the causal GARCH/Markov/rules teacher, not on textbook text. The state is compact and versioned.
- Laya must beat the teacher, always-HOLD, and random after costs, and must be calibrated, before it can trade.
- The LLM brief is a structured feature and a guardian input. It cannot open, size, or close a position by itself. A brief older than 30 minutes blocks new entries.
- News is untrusted data. The LLM gets no tools and must return strict JSON; every field is validated. Send tickers and relative exposure, never balances or account identifiers.
- Every external call has a timeout. If the LLM or SQAC is down, the system uses the deterministic fallback and says so on the dashboard.
- SQAC is the runtime memory. TurboQuant or another dense tier is added only if a held-out query set shows SQAC's own tiers missing conceptual queries.

## Asset onboarding and fast fine-tune

> **Superseded by the Decision Record.** The tier hierarchy below is retained as a future optimization. The critical path is one full `laya-train` checkpoint per asset, lazy-loaded. Adapter and head-only sharing are undocumented.

Adding an asset is a pipeline, not a code change: `dococ asset add` builds an asset pack from history, fits the statistical models, tunes Laya for the asset, and leaves it in shadow mode until it passes its gates.

&#91;embedded content: asset onboarding · 7 stages, 1 validation gate\]

New assets start from the base checkpoint and are fine-tuned on the asset's own causal teacher labels. Failing validation sends the asset back to tuning; nothing advances on its own. A class adapter is a future optimization.

**What an asset pack holds**

- Instrument and venue mapping: tick and lot sizes, sessions, roll rules.
- Cost model: fees, funding or commissions, a slippage estimate.
- Bar and feature configuration.
- GARCH and Markov parameters, versioned, from the offline fit.
- Laya ONNX checkpoint, calibration curve, and action/abstention thresholds.
- Strategy parameters and the validation report.
- Current stage: draft, shadow, paper or live-eligible.

**Where "seamless" holds.** A new asset on an existing venue needs only a pack. A new asset class (FX, indices, equities) adds a calendar and a cost model. A new venue still needs an adapter, and a firm's instrument allowlist is a profile edit. Fine-tuning handles the models, not the integrations.

**Fast fine-tune tiers**

| Tier | What changes | Compute | When to use |
| --- | --- | --- | --- |
| 1 Calibrate | Per-asset thresholds and calibration curve on Laya's frozen outputs; optionally a small head on its embeddings | CPU, seconds to minutes | New asset with little data |
| 2 Adapter | A lightweight adapter or classification head trained on the asset's own market-labeled headlines | Local GPU if it fits in 4 GB, otherwise a Kaggle T4; minutes to about an hour | Enough labeled events have accumulated |
| 3 Class refresh | Broader fine-tune of the class-level adapter on pooled data from related assets | Kaggle T4; hours | Occasional refresh, not per asset |

Build the full per-asset checkpoint first. The tier hierarchy above is a future optimization; whether Laya supports shareable adapters or head-only artifacts is still undocumented, so it is not on the critical path. A full fine-tune of a 421M-parameter model fits the free T4s.

**Hierarchy.** Base Laya, then a full per-asset checkpoint and calibration. A class adapter or shared head is a future optimization, not the critical path. A brand-new asset runs on the base checkpoint with NO_ACTION until its checkpoint passes calibration.

**Labels come from the causal teacher.** GARCH volatility, Markov regime, and deterministic rules produce a soft distribution over LONG/SHORT/HOLD/NO_ACTION/CLOSE. Use non-overlapping events and purged, embargoed splits; validate after costs.

**Promotion.** Draft: pack built, nothing runs. Shadow: actions and briefs are logged, no orders. Paper: simulated orders on live data. Live-eligible: set by hand with `dococ asset promote`.

- Shadow requires model parity, calibration at least as good as the incumbent on held-out events, and the teacher/strategy validation gate.
- Paper requires weeks of fills that stay within the slippage model.
- The guardian rejects any intent on an asset missing from the account's profile allowlist, whatever its stage.

**Tuning safeguards**

- Every tune writes a versioned artifact: data hash, configuration, metrics and a model card. `dococ tune rollback` restores the previous one.
- A drift monitor tracks calibration error on live actions. A breach demotes the asset to its previous checkpoint and forces NO_ACTION until recalibration, which can only reduce risk.
- Scheduled retraining passes the same gates. There is no online learning from live trades.

**Commands**

| Command | What it does |
| --- | --- |
| `dococ asset add <sym> --venue <v> [--class <c>] [--like <sym>]` | Creates the pack, then runs ingest, fit, tune and validation in shadow |
| `dococ asset list\|show\|promote\|demote\|rm` | Inspects packs and moves them between stages |
| `dococ tune <sym> [--tier 1\|2\|3] [--from <adapter>]` | Runs a fine-tune; `--from` starts from a chosen adapter |
| `dococ tune status\|eval\|rollback <sym>` | Shows progress or held-out metrics, or restores the previous artifact |

**Order of work.** The pipeline is built in phase 3 on BTC/USDT. A second crypto asset is the first test of "seamless"; gold in phase 4 tests it across asset classes.

## CLI commands

The `dococ` binary is a thin client: every command that changes state goes through the daemon and the risk guardian, so a manual trade is held to the same rules as an automated one.

| Command | What it does |
| --- | --- |
| `dococ daemon start\|stop\|status` | Runs the engine; on a server, run it under systemd. `status` gives a one-screen summary before the dashboard exists |
| `dococ acct add <name> --venue <v> --profile <p> --mode paper\|live` | Adds an account; API keys are entered at a prompt and stored in the keyring or an encrypted file |
| `dococ acct list\|show\|pause\|resume\|rm` | Manages accounts; `show` includes rule headroom |
| `dococ profile show\|validate <file>` | Inspects and lints rule profiles |
| `dococ trade open <acct> <sym> <long\|short> --size <n> --stop <px> [--tp <px>]` | Manual entry; a stop is required |
| `dococ trade close\|flatten <acct> [sym]` and `dococ trade list` | Exits and open-trade listing |
| `dococ auto start\|stop\|status <acct> [--strategy <s>]` | Turns automation on or off per account; `dococ auto pause --until <time>` for a temporary hold |
| `dococ strategy status\|pause\|resume <s>` | Lifecycle monitor state and manual overrides |
| `dococ risk show <acct>` | Daily-loss and drawdown headroom, escalation step, profit-target progress |
| `dococ challenge sim <profile> --strategy <s>` | Monte Carlo pass probability, breach risk, days to target, and a risk-capped size |
| `dococ kill [--flatten]` | Trips the kill switch on all accounts, or one with `--acct` |
| `dococ regime <sym>` | Current volatility forecast and regime state, parameter age |
| `dococ news [--sym <s>]` and `dococ thesis <sym>` | Scored news feed; on-demand LLM thesis |
| `dococ data check <sym>` | Gap, duplicate and bad-tick scan |
| `dococ experiments list\|show` | Experiment registry and trial counts |
| `dococ report [--day\|--week] [--acct]` | Attribution and payout-tracking report |
| `dococ dash` | Opens the dashboard |
| `dococ backtest`, `dococ replay`, `dococ paper` | Historical test, deterministic replay, paper run on live data |
| `dococ reconcile <acct> [--accept]` | Compares ledger with the venue; `--accept` clears a halt after you have checked |
| `dococ journal query <text>` | Searches the decision journal |

Safeguards: an account is paper unless set to `live` explicitly, mutating commands support `--dry-run`, and `dococ kill` works even when the strategy layer is wedged. Asset and tuning commands (`dococ asset`, `dococ tune`) are described under Asset onboarding and fast fine-tune.

## Dashboard

The dashboard is a `ratatui` client that subscribes to a stream from the daemon, so closing it never affects trading. Earnings come from the daemon's own ledger marked to live prices, and are cross-checked against venue balances at each reconcile.

| Panel | Shows |
| --- | --- |
| Earnings | Realized and unrealized P&L per account and in total; today, week and all time; equity sparkline |
| Rule headroom | Gauges for remaining daily loss and drawdown (currency and share of limit), profit-target progress, trading-day count; colour follows the escalation ladder |
| Positions and orders | Open positions with entry, stop, target and open risk; working orders |
| Regime | Per symbol: state probabilities, volatility forecast against realized, parameter age |
| News | Time-ordered feed with the LLM brief, event risk and source links; selecting an item shows Laya's action, confidence, and the structured brief; filter to held symbols |
| System health | Feed latency, age of last tick, venue connectivity, reconcile status, clock skew, bot state per account, kill-switch state |

Keys: tab or number to switch account, `p` pause an account, `k` kill (with confirmation), `/` filter news. Anything dangerous asks for confirmation, and the dashboard itself can issue only the commands the CLI can.

## Persistence, security and operations

Live accounts should run on an always-on server, not on the laptop, with every decision recorded in an append-only log you can replay.

- **Event log.** SQLite in WAL mode stores every intent, risk decision (with the rule that fired), order event, fill and model output. State is rebuilt by replay, which is also how backtests and post-mortems work. Back it up on a schedule.
- **Secrets.** API keys in the OS keyring or an `age`-encrypted file. Create keys with trading permission only, withdrawals disabled, and an IP allowlist where the venue supports one. Keys never appear in logs and are never visible to any model.
- **Deployment.** The daemon runs under systemd on a VPS with automatic restart. The laptop runs the dashboard over an authenticated SSH tunnel to the daemon's socket. A laptop sleeping, losing Wi-Fi or running out of VRAM should never be able to hurt an open position.
- **Restarts.** Restart-and-reconcile is a tested path: the daemon comes back, reads the log, queries the venue, and resumes only when both agree. Deploys with open positions go through the same path.
- **Observability.** Structured `tracing` logs and metrics. Alerts to a chat webhook on halt, kill, reconcile mismatch, stale data or parameter expiry. An external dead-man heartbeat alerts you if the daemon goes silent.
- **Configuration.** TOML, versioned in git, with rule profiles carrying the firm and rulebook date they were written from.

## Battle-testing plan

A funded account is lost by one bug or one bad night, so testing aims at the failure modes, not at average behaviour. Durations below are suggested minimums; adjust to your risk tolerance.

| Layer | What is tested | Pass criterion |
| --- | --- | --- |
| Property tests (`proptest`) | Risk guardian, order state machine, money arithmetic, over arbitrary event sequences | No sequence of intents and fills breaches a profile; no illegal state; no rounding drift |
| Model parity | Rust GARCH and Markov filter against the Python reference on fixed series | Match within tolerance on a long series |
| Deterministic replay with fault injection | Real engine on historical data with dropped, duplicated and reordered messages, delayed acknowledgements, partial fills, disconnects mid-order, restarts | Final ledger equals venue truth; no profile breach; no orphaned position |
| Rule-conformance replay | Years of history through each rule profile, including gap days and news spikes | Zero breaches, including worst-case slippage |
| Parser fuzzing (`cargo-fuzz`) | Exchange and broker message parsers | No panics; bad input is rejected and logged |
| Chaos in staging | `kill -9` with open positions, network drops, clock skew, venue maintenance windows | Recovers, reconciles, resumes or halts safely |
| Testnet or simulator | Continuous run against exchange testnet or the firm's simulator | At least 2 weeks with no unreconciled state |
| Paper on live data | Full strategy stack with realistic fills | At least 4 to 8 weeks; fill and slippage model checked against reality |
| Staged live | Smallest size on an evaluation or personal account | Scale only after several weeks inside the risk budget with no manual state repair |

Every bug found in live or paper trading becomes a permanent regression test in the replay suite before the fix ships.

## Roadmap and gates

&#91;embedded content: roadmap · Phase 0 spikes plus 7 build phases, each ending in a gate\]

Each gate is pass or fail: a failed gate means fixing or dropping the work, not moving on. Durations depend on how many hours a week you can give it, so none are promised here; live trading starts only after the paper gates.

**Deferred until evidence justifies them**

- Full per-asset checkpoints: the critical path. Head-only or class adapters are a future optimization, not a gate.
- LLM analyst: on the 15-minute path as a structured feature and guardian input; no order authority.
- SQAC: the runtime memory. TurboQuant or another dense tier remains optional.
- Markov-switching regimes: until they beat GARCH alone out of sample with stable state labels.
- Dashboard polish: `dococ daemon status` and `dococ report` come first.
- The full crate split: start with six crates and split when a module earns it.

## Open questions and unverified assumptions (closed)

Closed. See `Implementation_Checklist.md` for the closed register and deferred Phase 0 tasks.

Resolved since the original draft:

- Laya ONNX export and native Rust inference are verified via the official `laya-onnx` crate.
- Laya's role is now action selection, not headline scoring.
- Per-asset models are full checkpoints, lazy-loaded.
- The causal GARCH/Markov teacher replaces the keyword baseline.
- The LLM runs every 15 minutes with no order authority.
- SQAC is the runtime memory; original notes only.
- The engine targets a Linux VPS, with Windows for development.

Still external and deferred to live gating: funded firms, crypto exchange, gold broker/API, and written confirmation that automation is allowed.

Compliance with each firm's terms and with the law in your jurisdiction is your responsibility. This document is a software design, not financial advice.
