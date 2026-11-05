use dococ_core::{Account, Bar, Clock, Event, FixedClock, Instrument, Order, Side, Symbol};
use dococ_laya::{ActionValidation, LayaRuntime, ModelCache, OrderAction, validate_action};
use dococ_oms::OrderManager;
use dococ_risk::{AccountState, PositionSizer, RiskConfig, RiskGuardian, SizingConfig};
use dococ_venue::SimVenue;

/// Strategy interface: pluggable decision logic.
/// Returns (action, confidence).
pub trait Strategy: Send {
    fn name(&self) -> &str;
    fn decide(&mut self, state: &str) -> Result<(OrderAction, f64), Box<dyn std::error::Error>>;
}

/// Laya-based strategy.
pub struct LayaStrategy {
    runtime: LayaRuntime,
}

impl LayaStrategy {
    pub fn new(runtime: LayaRuntime) -> Self {
        Self { runtime }
    }
}

impl Strategy for LayaStrategy {
    fn name(&self) -> &str {
        "laya"
    }

    fn decide(&mut self, state: &str) -> Result<(OrderAction, f64), Box<dyn std::error::Error>> {
        let decision = self.runtime.decide(state)?;
        Ok((decision.action, decision.action_confidence))
    }
}

/// Target position calculator: converts action + confidence + risk headroom into target quantity.
pub struct TargetPosition {
    pub symbol: Symbol,
    pub target_qty: f64,
    pub confidence: f64,
}

impl TargetPosition {
    pub fn from_action(action: OrderAction, confidence: f64, max_qty: f64) -> Self {
        let target_qty = match action {
            OrderAction::Long => max_qty * confidence,
            OrderAction::Short => -max_qty * confidence,
            OrderAction::Hold | OrderAction::NoAction | OrderAction::Close => 0.0,
        };
        Self {
            symbol: Symbol::new(""),
            target_qty,
            confidence,
        }
    }
}

/// Decision scheduler: tracks when to run LLM brief and Laya evaluation.
pub struct Scheduler {
    llm_interval_secs: i64,
    last_llm_brief: i64,
    bar_interval_secs: i64,
    last_bar: i64,
}

impl Scheduler {
    pub fn new(llm_interval_secs: i64, bar_interval_secs: i64) -> Self {
        Self {
            llm_interval_secs,
            last_llm_brief: -llm_interval_secs,
            bar_interval_secs,
            last_bar: -bar_interval_secs,
        }
    }

    pub fn should_run_llm(&self, now: i64) -> bool {
        now - self.last_llm_brief >= self.llm_interval_secs
    }

    pub fn should_evaluate(&self, now: i64) -> bool {
        now - self.last_bar >= self.bar_interval_secs
    }

    pub fn mark_llm(&mut self, now: i64) {
        self.last_llm_brief = now;
    }

    pub fn mark_bar(&mut self, now: i64) {
        self.last_bar = now;
    }
}

/// Tick result from the engine.
#[derive(Debug)]
pub struct TickResult {
    pub action: OrderAction,
    pub action_confidence: f64,
    pub risk_decision: dococ_risk::RiskDecision,
    pub target_qty: f64,
}

/// Maximum state size in tokens (design: 500-700).
pub const MAX_STATE_TOKENS: usize = 700;

/// The daemon engine.
pub struct Engine<S: Strategy> {
    strategy: S,
    guardian: RiskGuardian,
    sizer: PositionSizer,
    oms: OrderManager,
    venue: SimVenue,
    account: Account,
    scheduler: Scheduler,
    llm_brief: Option<String>,
    llm_brief_stale: bool,
    timeout: InferenceTimeout,
    model_cache: ModelCache,
}

impl<S: Strategy> Engine<S> {
    pub fn new(
        strategy: S,
        risk_config: RiskConfig,
        venue: SimVenue,
        account: Account,
        scheduler: Scheduler,
    ) -> Self {
        Self {
            strategy,
            guardian: RiskGuardian::new(risk_config),
            sizer: PositionSizer::new(SizingConfig::default()),
            oms: OrderManager::new(),
            venue,
            account,
            scheduler,
            llm_brief: None,
            llm_brief_stale: true,
            timeout: InferenceTimeout::default(),
            model_cache: ModelCache::new(2),
        }
    }

    /// Process a market data bar.
    pub fn on_bar(
        &mut self,
        bar: &Bar,
        symbol: &Symbol,
    ) -> Result<Option<TickResult>, Box<dyn std::error::Error>> {
        let now = bar.timestamp;
        if !self.scheduler.should_evaluate(now) {
            return Ok(None);
        }
        self.scheduler.mark_bar(now);

        // Build the compact state via StateBuilder.
        let mut sb = StateBuilder::default();
        sb.position = self.account.get_position(symbol).cloned();
        sb.bars = vec![bar.clone()];
        if let Some(brief) = &self.llm_brief {
            sb.llm_brief = Some(LlmBrief {
                regime: brief.clone(),
                news_bias: "unknown".to_string(),
                event_risk: "unknown".to_string(),
                correlation_flag: false,
                confidence: 0.0,
                as_of: now,
            });
        }
        let state = sb.build();
        if sb.token_count() > MAX_STATE_TOKENS {
            // State exceeded the token budget; block the trade rather than truncate.
            return Ok(None);
        }

        // Run strategy with timing check.
        let start = std::time::Instant::now();
        let (mut action, confidence) = self.strategy.decide(&state)?;
        let elapsed = start.elapsed();
        if elapsed.as_millis() as u64 > self.timeout.infer_ms {
            // Inference took too long; fall back to NO_ACTION to prevent trading on stale state.
            action = OrderAction::NoAction;
        }

        // Validate action; invalid actions become NO_ACTION.
        let action_str = match action {
            OrderAction::Long => "LONG",
            OrderAction::Short => "SHORT",
            OrderAction::Hold => "HOLD",
            OrderAction::NoAction => "NO_ACTION",
            OrderAction::Close => "CLOSE",
        };
        let action =
            if dococ_laya::validate_action(action_str) == dococ_laya::ActionValidation::Valid {
                action
            } else {
                OrderAction::NoAction
            };

        // Get confidence from the strategy (Laya action_confidence).

        // Risk check.
        let trade_notional = self.account.equity * 0.01; // 1% notional
        let proposed = dococ_risk::ProposedTrade {
            side: if action == OrderAction::Long || action == OrderAction::Short {
                if action == OrderAction::Long {
                    dococ_risk::Side::Long
                } else {
                    dococ_risk::Side::Short
                }
            } else {
                dococ_risk::Side::Long
            },
            quantity: 0.0, // placeholder
            price: bar.close,
            stop_loss: None,
            notional: trade_notional,
        };

        let open_notional: f64 = self
            .account
            .positions
            .values()
            .map(|p| p.quantity.abs() * p.avg_price)
            .sum();
        let acct_state = AccountState {
            equity: self.account.equity,
            daily_pnl: self.account.daily_pnl,
            peak_equity: self.account.peak_equity,
            open_notional,
        };

        let risk_decision = if action == OrderAction::Long || action == OrderAction::Short {
            self.guardian.evaluate(&acct_state, &proposed)
        } else {
            dococ_risk::RiskDecision::Accept
        };

        // Sizing: volatility target + confidence + guardian headroom.
        let vol = 0.02; // placeholder; in production, use GARCH forecast
        let headroom = self.account.equity * 1.0; // 1x notional cap
        let sized_qty = self
            .sizer
            .size(self.account.equity, vol, confidence, bar.close, headroom);
        let target_qty = match action {
            OrderAction::Long => sized_qty,
            OrderAction::Short => -sized_qty,
            _ => 0.0,
        };

        // Update LLM brief staleness.
        if let Some(brief_time) = self.llm_brief.as_ref().map(|_| now) {
            if now - brief_time > 1800 {
                self.llm_brief_stale = true;
            }
        }

        Ok(Some(TickResult {
            action,
            action_confidence: confidence,
            risk_decision,
            target_qty,
        }))
    }

    /// Process an LLM brief arrival.
    pub fn on_llm_brief(&mut self, brief: String, timestamp: i64) {
        self.llm_brief = Some(brief);
        self.llm_brief_stale = false;
    }

    /// Check if LLM brief is stale.
    pub fn llm_brief_stale(&self) -> bool {
        self.llm_brief_stale
    }

    /// Get the account.
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// Cache a model in the LRU (for loading additional models).
    pub fn cache_model(&mut self, name: &str) {
        self.model_cache.insert(name);
    }

    /// Get the OMS.
    pub fn oms(&self) -> &OrderManager {
        &self.oms
    }

    /// Get the venue.
    /// Run a CPU-heavy decision task off the event loop using spawn_blocking.
    /// For async/external callers. The synchronous `on_bar` tick calls the
    /// strategy directly and enforces `InferenceTimeout` post-hoc (falls back
    /// to NO_ACTION when exceeded).
    pub fn decide_blocking<F>(f: F, timeout_ms: u64) -> Result<OrderAction, &'static str>
    where
        F: FnOnce() -> OrderAction + Send + 'static,
    {
        spawn_blocking(f, std::time::Duration::from_millis(timeout_ms))
    }

    pub fn venue(&self) -> &SimVenue {
        &self.venue
    }
}

mod fault;
pub use fault::*;
mod replay;
pub use replay::*;
mod state;
pub use state::*;
mod timeout;
pub use timeout::*;

#[cfg(test)]
mod tests {
    use super::*;
    use dococ_core::{AccountId, Bar, Symbol};
    use dococ_laya::OrderAction;

    #[test]
    fn scheduler_llm_interval() {
        let mut sched = Scheduler::new(900, 3600);
        assert!(sched.should_run_llm(1000));
        sched.mark_llm(1000);
        assert!(!sched.should_run_llm(1500));
        assert!(sched.should_run_llm(2000));
    }

    #[test]
    fn scheduler_bar_interval() {
        let mut sched = Scheduler::new(900, 3600);
        assert!(sched.should_evaluate(1000));
        sched.mark_bar(1000);
        assert!(!sched.should_evaluate(2000));
        assert!(sched.should_evaluate(5000));
    }

    #[test]
    fn target_position_long() {
        let tp = TargetPosition::from_action(OrderAction::Long, 0.8, 1.0);
        assert!((tp.target_qty - 0.8).abs() < 1e-9);
    }

    #[test]
    fn target_position_short() {
        let tp = TargetPosition::from_action(OrderAction::Short, 0.6, 1.0);
        assert!((tp.target_qty - (-0.6)).abs() < 1e-9);
    }

    #[test]
    fn target_position_hold() {
        let tp = TargetPosition::from_action(OrderAction::Hold, 0.9, 1.0);
        assert!((tp.target_qty - 0.0).abs() < 1e-9);
    }

    #[test]
    fn engine_tick_with_model() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let mut engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);

        let bar = Bar {
            timestamp: 1_700_000_000,
            open: 64_000.0,
            high: 64_500.0,
            low: 63_800.0,
            close: 64_200.0,
            volume: 1_000.0,
        };
        let symbol = Symbol::new("BTCUSDT");
        let result = engine.on_bar(&bar, &symbol).expect("tick");
        assert!(result.is_some());
    }

    #[test]
    fn engine_tick_blocks_when_state_exceeds_token_budget() {
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let mut engine = Engine::new(
            DummyStrategy,
            RiskConfig::default(),
            venue,
            account,
            scheduler,
        );
        // 5000 chars ~= 1250 tokens > MAX_STATE_TOKENS.
        engine.on_llm_brief("x".repeat(5000), 1_700_000_000);
        let bar = Bar {
            timestamp: 1_700_000_000,
            open: 64_000.0,
            high: 64_500.0,
            low: 63_800.0,
            close: 64_200.0,
            volume: 1_000.0,
        };
        let symbol = Symbol::new("BTCUSDT");
        let result = engine.on_bar(&bar, &symbol).expect("tick");
        assert!(result.is_none(), "oversized state must block the trade");
    }

    struct DummyStrategy;
    impl Strategy for DummyStrategy {
        fn name(&self) -> &str {
            "dummy"
        }
        fn decide(
            &mut self,
            _state: &str,
        ) -> Result<(OrderAction, f64), Box<dyn std::error::Error>> {
            Ok((OrderAction::Hold, 0.5))
        }
    }

    #[test]
    fn engine_decide_blocking_runs_off_thread() {
        let res = Engine::<DummyStrategy>::decide_blocking(|| OrderAction::Long, 1000);
        assert_eq!(res, Ok(OrderAction::Long));

        let timeout_res = Engine::<DummyStrategy>::decide_blocking(
            || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                OrderAction::Short
            },
            10,
        );
        assert_eq!(timeout_res, Err("timeout"));
    }
}
