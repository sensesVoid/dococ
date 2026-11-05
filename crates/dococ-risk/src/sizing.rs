/// Position sizing: volatility target + confidence + guardian headroom.

#[derive(Debug, Clone, Copy)]
pub struct SizingConfig {
    pub vol_target: f64,
    pub min_confidence: f64,
}

impl Default for SizingConfig {
    fn default() -> Self {
        Self {
            vol_target: 0.01,
            min_confidence: 0.3,
        }
    }
}

pub struct PositionSizer {
    config: SizingConfig,
}

impl PositionSizer {
    pub fn new(config: SizingConfig) -> Self {
        Self { config }
    }

    /// Compute position size (quantity in base units).
    pub fn size(&self, equity: f64, vol: f64, confidence: f64, price: f64, headroom: f64) -> f64 {
        if equity <= 0.0 || vol <= 0.0 || price <= 0.0 || confidence < self.config.min_confidence {
            return 0.0;
        }

        let risk_budget = equity * self.config.vol_target;
        let position_value = risk_budget / vol;
        let scaled_value = position_value * confidence;
        let capped_value = scaled_value.min(headroom);
        capped_value / price
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_by_vol_target() {
        let config = SizingConfig::default();
        let sizer = PositionSizer::new(config);
        let qty = sizer.size(100_000.0, 0.02, 0.5, 64_000.0, 100_000.0);
        assert!((qty - 0.390625).abs() < 1e-6);
    }

    #[test]
    fn scales_by_confidence() {
        let config = SizingConfig::default();
        let sizer = PositionSizer::new(config);
        let qty_full = sizer.size(100_000.0, 0.02, 1.0, 64_000.0, 100_000.0);
        let qty_half = sizer.size(100_000.0, 0.02, 0.5, 64_000.0, 100_000.0);
        assert!((qty_half - qty_full * 0.5).abs() < 1e-6);
    }

    #[test]
    fn respects_headroom() {
        let config = SizingConfig::default();
        let sizer = PositionSizer::new(config);
        let qty = sizer.size(100_000.0, 0.02, 1.0, 64_000.0, 50_000.0);
        assert!((qty - 0.78125).abs() < 1e-6);
    }

    #[test]
    fn returns_zero_on_invalid_inputs() {
        let config = SizingConfig::default();
        let sizer = PositionSizer::new(config);
        assert_eq!(sizer.size(0.0, 0.02, 0.5, 64_000.0, 100_000.0), 0.0);
        assert_eq!(sizer.size(100_000.0, 0.0, 0.5, 64_000.0, 100_000.0), 0.0);
        assert_eq!(sizer.size(100_000.0, 0.02, 0.5, 0.0, 100_000.0), 0.0);
    }

    #[test]
    fn never_exceeds_headroom() {
        let config = SizingConfig::default();
        let sizer = PositionSizer::new(config);
        let qty = sizer.size(100_000.0, 0.001, 1.0, 64_000.0, 50_000.0);
        let notional = qty * 64_000.0;
        assert!(notional <= 50_000.0 + 1e-6);
    }
}
