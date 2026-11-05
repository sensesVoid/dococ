use dococ_laya::{LayaRuntime, OrderAction};
use dococ_oms::OrderManager;
use dococ_risk::{AccountState, RiskConfig, RiskGuardian};
use dococ_venue::SimVenue;

#[derive(Debug)]
pub struct TickResult {
    pub action: OrderAction,
    pub action_confidence: f64,
    pub risk_decision: dococ_risk::RiskDecision,
}

pub struct Engine {
    laya: LayaRuntime,
    guardian: RiskGuardian,
    oms: OrderManager,
    venue: SimVenue,
}

impl Engine {
    pub fn new(laya: LayaRuntime, risk_config: RiskConfig, venue: SimVenue) -> Self {
        Self {
            laya,
            guardian: RiskGuardian::new(risk_config),
            oms: OrderManager::new(),
            venue,
        }
    }

    pub fn tick(&mut self, state: &str, account: &AccountState) -> Result<TickResult, Box<dyn std::error::Error>> {
        let decision = self.laya.decide(state)?;
        let risk_decision = dococ_risk::RiskDecision::Accept; // placeholder
        Ok(TickResult {
            action: decision.action,
            action_confidence: decision.action_confidence,
            risk_decision,
        })
    }

    pub fn oms(&self) -> &OrderManager {
        &self.oms
    }

    pub fn venue(&self) -> &SimVenue {
        &self.venue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_tick_returns_action() {
        // This test requires a model; skip if not available.
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let mut venue = SimVenue::new(100_000.0, 5.0);
        venue.set_price("BTCUSDT", 64_000.0);
        let mut engine = Engine::new(laya, RiskConfig::default(), venue);
        let account = AccountState {
            equity: 100_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        let result = engine.tick("Account: paper. Position: flat.", &account).expect("tick");
        assert!(matches!(
            result.action,
            OrderAction::Long | OrderAction::Short | OrderAction::Hold | OrderAction::NoAction | OrderAction::Close
        ));
    }
}
