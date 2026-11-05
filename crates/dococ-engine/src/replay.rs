use crate::{Engine, LayaStrategy, Scheduler, Strategy};
use dococ_core::{Account, AccountId, Bar, Symbol};
use dococ_laya::{LayaRuntime, OrderAction};
use dococ_risk::RiskConfig;
use dococ_venue::SimVenue;

/// A recorded decision from a replay run.
#[derive(Debug, Clone)]
pub struct ReplayDecision {
    pub timestamp: i64,
    pub symbol: Symbol,
    pub action: OrderAction,
    pub confidence: f64,
}

/// A recorded event in the replay.
#[derive(Debug, Clone)]
pub enum ReplayEvent {
    Bar { symbol: Symbol, bar: Bar },
    Decision(ReplayDecision),
    LlmBrief { timestamp: i64, brief: String },
}

/// Deterministic replay harness.
pub struct Replay<S: Strategy> {
    engine: Engine<S>,
    decisions: Vec<ReplayDecision>,
    events: Vec<ReplayEvent>,
}

impl<S: Strategy> Replay<S> {
    pub fn new(engine: Engine<S>) -> Self {
        Self {
            engine,
            decisions: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Feed a bar through the engine and record the decision.
    pub fn feed_bar(&mut self, symbol: Symbol, bar: Bar) -> Result<Option<ReplayDecision>, Box<dyn std::error::Error>> {
        self.events.push(ReplayEvent::Bar {
            symbol: symbol.clone(),
            bar: bar.clone(),
        });

        let result = self.engine.on_bar(&bar, &symbol)?;
        if let Some(tick) = result {
            let decision = ReplayDecision {
                timestamp: bar.timestamp,
                symbol: symbol.clone(),
                action: tick.action,
                confidence: tick.action_confidence,
            };
            self.decisions.push(decision.clone());
            self.events.push(ReplayEvent::Decision(decision.clone()));
            Ok(Some(decision))
        } else {
            Ok(None)
        }
    }

    /// Feed an LLM brief.
    pub fn feed_brief(&mut self, brief: String, timestamp: i64) {
        self.engine.on_llm_brief(brief.clone(), timestamp);
        self.events.push(ReplayEvent::LlmBrief { timestamp, brief });
    }

    /// Get all recorded decisions.
    pub fn decisions(&self) -> &[ReplayDecision] {
        &self.decisions
    }

    /// Get all recorded events.
    pub fn events(&self) -> &[ReplayEvent] {
        &self.events
    }

    /// Count decisions by action type.
    pub fn action_counts(&self) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for d in &self.decisions {
            let key = format!("{:?}", d.action);
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Get the final LLM brief staleness.
    pub fn llm_stale(&self) -> bool {
        self.engine.llm_brief_stale()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dococ_core::{AccountId, Bar, Symbol};
    use dococ_laya::OrderAction;

    #[test]
    fn replay_records_decisions() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);
        let mut replay = Replay::new(engine);

        let symbol = Symbol::new("BTCUSDT");
        let bar = Bar {
            timestamp: 1_700_000_000,
            open: 64_000.0,
            high: 64_500.0,
            low: 63_800.0,
            close: 64_200.0,
            volume: 1_000.0,
        };
        replay.feed_bar(symbol.clone(), bar).expect("feed");
        assert_eq!(replay.decisions().len(), 1);
        assert_eq!(replay.events().len(), 2); // Bar + Decision
    }

    #[test]
    fn replay_feeds_brief() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);
        let mut replay = Replay::new(engine);

        replay.feed_brief("macro: risk-on".to_string(), 1_700_000_000);
        assert!(!replay.llm_stale());
        assert_eq!(replay.events().len(), 1);
    }

    #[test]
    fn replay_action_counts() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);
        let mut replay = Replay::new(engine);

        let symbol = Symbol::new("BTCUSDT");
        for i in 0..3 {
            let bar = Bar {
                timestamp: 1_700_000_000 + i * 3600,
                open: 64_000.0,
                high: 64_500.0,
                low: 63_800.0,
                close: 64_200.0 + (i as f64 * 100.0),
                volume: 1_000.0,
            };
            replay.feed_bar(symbol.clone(), bar).expect("feed");
        }
        let counts = replay.action_counts();
        assert!(!counts.is_empty());
        assert_eq!(replay.decisions().len(), 3);
    }
}
