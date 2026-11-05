/// Deterministic risk guardian for Doc Oc.
///
/// The guardian can reject, cap, or flatten a proposed trade. No model or LLM
/// can override it.

#[derive(Debug, Clone, Copy)]
pub struct RiskConfig {
    /// Fraction of equity risked per trade (0.0025 = 0.25%).
    pub risk_per_trade: f64,
    /// Daily loss limit as a fraction of equity (0.02 = 2%).
    pub daily_loss_limit: f64,
    /// Maximum drawdown from peak equity (0.05 = 5%).
    pub max_drawdown: f64,
    /// Maximum total open notional as a multiple of equity (1.0 = 1x).
    pub max_notional_cap: f64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            risk_per_trade: 0.0025,
            daily_loss_limit: 0.02,
            max_drawdown: 0.05,
            max_notional_cap: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AccountState {
    pub equity: f64,
    pub daily_pnl: f64,
    pub peak_equity: f64,
    pub open_notional: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy)]
pub struct ProposedTrade {
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
    pub stop_loss: Option<f64>,
    pub notional: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskDecision {
    Accept,
    Reject(&'static str),
    Cap { max_quantity: f64 },
    Flatten,
}

pub struct RiskGuardian {
    config: RiskConfig,
}

impl RiskGuardian {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    pub fn is_killed(&self, state: &AccountState) -> bool {
        if state.equity <= 0.0 {
            return true;
        }
        let daily_loss_pct = -state.daily_pnl / state.equity;
        if daily_loss_pct >= self.config.daily_loss_limit {
            return true;
        }
        if state.peak_equity > 0.0 {
            let drawdown = (state.peak_equity - state.equity) / state.peak_equity;
            if drawdown >= self.config.max_drawdown {
                return true;
            }
        }
        false
    }

    pub fn evaluate(&self, state: &AccountState, trade: &ProposedTrade) -> RiskDecision {
        if self.is_killed(state) {
            return RiskDecision::Flatten;
        }

        // Notional cap check.
        let new_notional = state.open_notional + trade.notional;
        if new_notional > state.equity * self.config.max_notional_cap {
            let remaining = state.equity * self.config.max_notional_cap - state.open_notional;
            if remaining <= 0.0 {
                return RiskDecision::Reject("notional cap exceeded");
            }
            let max_qty = remaining / trade.price;
            return RiskDecision::Cap { max_quantity: max_qty };
        }

        // Per-trade risk check (if stop loss is provided).
        if let Some(stop) = trade.stop_loss {
            let risk_amount = state.equity * self.config.risk_per_trade;
            let stop_distance = (trade.price - stop).abs();
            if stop_distance > 0.0 {
                let max_qty = risk_amount / stop_distance;
                if trade.quantity > max_qty {
                    return RiskDecision::Cap { max_quantity: max_qty };
                }
            }
        }

        RiskDecision::Accept
    }

    pub fn position_size(&self, equity: f64, stop_distance: f64, price: f64) -> f64 {
        if equity <= 0.0 || stop_distance <= 0.0 || price <= 0.0 {
            return 0.0;
        }
        let risk_amount = equity * self.config.risk_per_trade;
        risk_amount / (stop_distance * price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kills_when_daily_loss_exceeds_limit() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 100_000.0,
            daily_pnl: -2_500.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        assert!(guardian.is_killed(&state));
    }

    #[test]
    fn kills_when_drawdown_exceeds_limit() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 94_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        assert!(guardian.is_killed(&state));
    }

    #[test]
    fn does_not_kill_within_limits() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 99_000.0,
            daily_pnl: -1_000.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        assert!(!guardian.is_killed(&state));
    }

    #[test]
    fn rejects_trade_when_killed() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 94_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        let trade = ProposedTrade {
            side: Side::Long,
            quantity: 1.0,
            price: 64_000.0,
            stop_loss: Some(62_000.0),
            notional: 64_000.0,
        };
        assert_eq!(guardian.evaluate(&state, &trade), RiskDecision::Flatten);
    }

    #[test]
    fn caps_trade_when_notional_exceeded() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 100_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 90_000.0,
        };
        let trade = ProposedTrade {
            side: Side::Long,
            quantity: 1.0,
            price: 64_000.0,
            stop_loss: None,
            notional: 64_000.0,
        };
        match guardian.evaluate(&state, &trade) {
            RiskDecision::Cap { max_quantity } => {
                // remaining notional = 100_000 - 90_000 = 10_000; max_qty = 10_000 / 64_000
                assert!((max_quantity - 10_000.0 / 64_000.0).abs() < 1e-9);
            }
            other => panic!("expected Cap, got {other:?}"),
        }
    }

    #[test]
    fn caps_trade_when_risk_exceeded() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 100_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        // risk_per_trade = 0.25% of 100k = 250; stop distance = 2000; max_qty = 250/2000 = 0.125
        let trade = ProposedTrade {
            side: Side::Long,
            quantity: 1.0,
            price: 64_000.0,
            stop_loss: Some(62_000.0),
            notional: 64_000.0,
        };
        match guardian.evaluate(&state, &trade) {
            RiskDecision::Cap { max_quantity } => {
                assert!((max_quantity - 0.125).abs() < 1e-9);
            }
            other => panic!("expected Cap, got {other:?}"),
        }
    }

    #[test]
    fn accepts_trade_within_all_limits() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        let state = AccountState {
            equity: 100_000.0,
            daily_pnl: 0.0,
            peak_equity: 100_000.0,
            open_notional: 0.0,
        };
        let trade = ProposedTrade {
            side: Side::Long,
            quantity: 0.01,
            price: 64_000.0,
            stop_loss: Some(62_000.0),
            notional: 640.0,
        };
        assert_eq!(guardian.evaluate(&state, &trade), RiskDecision::Accept);
    }

    #[test]
    fn position_size_formula() {
        let config = RiskConfig::default();
        let guardian = RiskGuardian::new(config);
        // equity=100k, risk=0.25%=250, stop_distance=2000, price=64000 => 250/(2000*64000)=1.953e-7
        let qty = guardian.position_size(100_000.0, 2_000.0, 64_000.0);
        assert!((qty - 250.0 / (2_000.0 * 64_000.0)).abs() < 1e-12);
    }
}
