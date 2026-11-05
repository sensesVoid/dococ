use dococ_laya::OrderAction;
use serde::{Deserialize, Serialize};

/// One shadow decision record (logged, never traded).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowRecord {
    pub timestamp: i64,
    pub symbol: String,
    pub action: OrderAction,
    pub confidence: f64,
    pub price: f64,
    pub note: String,
}

/// Shadow-mode logger: records decisions without ever placing orders.
#[derive(Debug, Default)]
pub struct ShadowLog {
    pub records: Vec<ShadowRecord>,
    pub decisions: usize,
    pub orders: usize,
}

impl ShadowLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a decision (invariant: never increments `orders`).
    pub fn record(&mut self, rec: ShadowRecord) {
        debug_assert!(self.orders == 0, "shadow mode must never place orders");
        self.records.push(rec);
        self.decisions += 1;
    }

    /// Shadow gate: 2 weeks (≈ 336 one-hour decisions) or 500 decisions.
    pub fn gate_met(&self, days_elapsed: f64) -> bool {
        self.decisions >= 500 || (days_elapsed >= 14.0 && self.decisions >= 336)
    }

    /// Serialize the decision journal.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.records).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(action: OrderAction) -> ShadowRecord {
        ShadowRecord {
            timestamp: 1_700_000_000,
            symbol: "BTCUSDT".to_string(),
            action,
            confidence: 0.8,
            price: 64_200.0,
            note: "shadow".to_string(),
        }
    }

    #[test]
    fn records_decisions_without_orders() {
        let mut log = ShadowLog::new();
        log.record(rec(OrderAction::Long));
        log.record(rec(OrderAction::Hold));
        assert_eq!(log.decisions, 2);
        assert_eq!(log.orders, 0);
    }

    #[test]
    fn gate_needs_both_time_and_decisions() {
        let mut log = ShadowLog::new();
        for _ in 0..336 {
            log.record(rec(OrderAction::Hold));
        }
        assert!(!log.gate_met(1.0));
        assert!(log.gate_met(14.0));
    }

    #[test]
    fn gate_met_by_decision_count() {
        let mut log = ShadowLog::new();
        for _ in 0..500 {
            log.record(rec(OrderAction::Hold));
        }
        assert!(log.gate_met(1.0));
    }

    #[test]
    fn json_serializes() {
        let mut log = ShadowLog::new();
        log.record(rec(OrderAction::Short));
        let json = log.to_json().unwrap();
        assert!(json.contains("Short"));
    }
}
