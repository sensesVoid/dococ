use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Drift signals measured per decision batch (Phase 5).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriftReport {
    /// ECE from the most recent calibration batch.
    pub calibration_error: f64,
    /// Action distribution (LONG/SHORT/HOLD/NO_ACTION/CLOSE -> fraction).
    pub action_distribution: HashMap<String, f64>,
    /// Mean realized PnL per decision (USD).
    pub pnl_attribution: f64,
    /// Feature drift (population stability index; 0.1 = stable).
    pub feature_drift: f64,
    pub num_decisions: usize,
}

impl DriftReport {
    /// Check whether any signal breached its threshold (auto-rollback trigger).
    pub fn breaches(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.calibration_error > 0.10 {
            out.push("calibration_error");
        }
        if self.feature_drift > 0.25 {
            out.push("feature_drift");
        }
        // Action distribution entropy collapse: one action > 95% over >= 50 decisions.
        if self.num_decisions >= 50
            && self.action_distribution.values().any(|&f| f > 0.95)
        {
            out.push("action_collapse");
        }
        out
    }

    /// Heuristic "healthy" check for tests: no breaches.
    pub fn is_healthy(&self) -> bool {
        self.breaches().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> DriftReport {
        let mut dist = HashMap::new();
        dist.insert("LONG".to_string(), 0.3);
        dist.insert("SHORT".to_string(), 0.3);
        dist.insert("HOLD".to_string(), 0.2);
        dist.insert("NO_ACTION".to_string(), 0.2);
        DriftReport {
            calibration_error: 0.05,
            action_distribution: dist,
            pnl_attribution: 12.5,
            feature_drift: 0.08,
            num_decisions: 100,
        }
    }

    #[test]
    fn healthy_report_passes() {
        assert!(healthy().is_healthy());
    }

    #[test]
    fn high_calibration_error_breaches() {
        let mut r = healthy();
        r.calibration_error = 0.2;
        assert_eq!(r.breaches(), vec!["calibration_error"]);
    }

    #[test]
    fn action_collapse_detected() {
        let mut r = healthy();
        r.action_distribution.clear();
        r.action_distribution.insert("HOLD".to_string(), 0.99);
        assert!(r.breaches().contains(&"action_collapse"));
    }

    #[test]
    fn action_collapse_ignored_for_small_samples() {
        let mut r = healthy();
        r.action_distribution.clear();
        r.action_distribution.insert("HOLD".to_string(), 0.99);
        r.num_decisions = 10;
        assert!(!r.breaches().contains(&"action_collapse"));
    }

    #[test]
    fn feature_drift_breaches() {
        let mut r = healthy();
        r.feature_drift = 0.4;
        assert!(r.breaches().contains(&"feature_drift"));
    }
}
