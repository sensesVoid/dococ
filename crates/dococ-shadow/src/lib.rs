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

/// Run a real shadow pass: replay the last `limit` bars of `csv_path`
/// through the Laya model, logging decisions. Never places orders.
pub fn run_shadow(
    model_dir: &std::path::Path,
    csv_path: &std::path::Path,
    limit: usize,
    symbol: &str,
) -> Result<ShadowLog, String> {
    use dococ_laya::LayaRuntime;
    let runtime = LayaRuntime::load(model_dir).map_err(|e| format!("load: {e}"))?;
    let text = std::fs::read_to_string(csv_path).map_err(|e| format!("read csv: {e}"))?;
    let mut bars: Vec<(i64, f64)> = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() > 7 {
            if let (Ok(ts), Ok(close)) = (cols[0].parse::<i64>(), cols[4].parse::<f64>()) {
                bars.push((ts / 1000, close));
            }
        }
    }
    let start = bars.len().saturating_sub(limit);
    let mut log = ShadowLog::new();
    let mut buf: Vec<(i64, f64)> = Vec::new();
    for (ts, close) in &bars[start..] {
        // Keep a rolling window of the last 32 closes for momentum/vol features.
        buf.push((*ts, *close));
        if buf.len() > 32 {
            buf.remove(0);
        }
        let state = if buf.len() > 24 {
            let first = buf[0].1;
            let mom = (close / first - 1.0) * 100.0;
            let mut rets = Vec::new();
            for w in buf.windows(2) {
                rets.push((w[1].1 / w[0].1).ln());
            }
            let mean = rets.iter().sum::<f64>() / rets.len() as f64;
            let var: f64 = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
                / rets.len() as f64;
            let vol = var.sqrt() * 24.0_f64.sqrt() * 100.0;
            let bars_text = buf
                .iter()
                .rev()
                .take(6)
                .map(|b| format!("C:{:.0}", b.1))
                .collect::<Vec<_>>()
                .join(" | ");
            format!(
                "Account: paper. Equity 100000 USD. Position: flat. Market: {symbol} perpetual. Last price {close:.0}. 24h momentum: {mom:+.2}%. GARCH vol (24h): {vol:.2}%. Recent bars: {bars_text}. Regime: unknown. News: none."
            )
        } else {
            format!(
                "Account: paper. Equity 100000 USD. Position: flat. Market: {symbol} perpetual. Last price {close:.0}."
            )
        };
        let decision = runtime.decide(&state).map_err(|e| format!("decide: {e}"))?;
        log.record(ShadowRecord {
            timestamp: *ts,
            symbol: symbol.to_string(),
            action: decision.action,
            confidence: decision.action_confidence,
            price: *close,
            note: "shadow".to_string(),
        });
    }
    Ok(log)
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
