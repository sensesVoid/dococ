use dococ_core::{Bar, Position, Symbol};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// NA sentinel for missing data.
pub const NA: &str = "NA";

/// Markov regime probabilities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegimeProbs {
    pub p_down: f64,
    pub p_range: f64,
    pub p_up: f64,
}

/// LLM analyst brief (structured).
#[derive(Debug, Clone)]
pub struct LlmBrief {
    pub regime: String,
    pub news_bias: String,
    pub event_risk: String,
    pub correlation_flag: bool,
    pub confidence: f64,
    pub as_of: i64,
}

/// State builder for the compact Laya state (500-700 tokens).
#[derive(Debug, Clone)]
pub struct StateBuilder {
    pub position: Option<Position>,
    pub bars: Vec<Bar>,
    pub garch_vol: Option<f64>,
    pub markov: Option<RegimeProbs>,
    pub headlines: Vec<String>,
    pub sqac_cases: Vec<String>,
    pub llm_brief: Option<LlmBrief>,
    pub max_bars: usize,
    pub max_headlines: usize,
    pub max_sqac: usize,
}

impl Default for StateBuilder {
    fn default() -> Self {
        Self {
            position: None,
            bars: Vec::new(),
            garch_vol: None,
            markov: None,
            headlines: Vec::new(),
            sqac_cases: Vec::new(),
            llm_brief: None,
            max_bars: 8,
            max_headlines: 3,
            max_sqac: 3,
        }
    }
}

impl StateBuilder {
    /// Build the deterministic state text.
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // Position
        match &self.position {
            Some(pos) => {
                parts.push(format!(
                    "Position: {} qty={:.4} avg={:.2} ({}",
                    pos.symbol, pos.quantity, pos.avg_price, pos.symbol
                ));
                // unrealized PnL needs a price; skip for now
            }
            None => parts.push("Position: flat".to_string()),
        }

        // Bars (most recent first, deterministic order)
        let bars: Vec<&Bar> = self.bars.iter().rev().take(self.max_bars).collect();
        if !bars.is_empty() {
            let bar_strs: Vec<String> = bars
                .iter()
                .map(|b| {
                    format!(
                        "O:{:.0} H:{:.0} L:{:.0} C:{:.0} V:{:.0}",
                        b.open, b.high, b.low, b.close, b.volume
                    )
                })
                .collect();
            parts.push(format!("Bars: {}", bar_strs.join(" | ")));
        } else {
            parts.push("Bars: NA".to_string());
        }

        // GARCH
        match self.garch_vol {
            Some(v) => parts.push(format!("GARCH vol: {:.4}", v)),
            None => parts.push(format!("GARCH vol: {}", NA)),
        }

        // Markov
        match &self.markov {
            Some(m) => parts.push(format!(
                "Regime: down={:.2} range={:.2} up={:.2}",
                m.p_down, m.p_range, m.p_up
            )),
            None => parts.push(format!("Regime: {}", NA)),
        }

        // Headlines
        let headlines: Vec<&String> = self.headlines.iter().take(self.max_headlines).collect();
        if !headlines.is_empty() {
            parts.push(format!(
                "News: {}",
                headlines
                    .iter()
                    .map(|h| h.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        } else {
            parts.push("News: NA".to_string());
        }

        // SQAC cases
        let cases: Vec<&String> = self.sqac_cases.iter().take(self.max_sqac).collect();
        if !cases.is_empty() {
            parts.push(format!(
                "Memory: {}",
                cases
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }

        // LLM brief
        if let Some(brief) = &self.llm_brief {
            parts.push(format!(
                "LLM: regime={} bias={} risk={} corr={} conf={:.2} as_of={}",
                brief.regime,
                brief.news_bias,
                brief.event_risk,
                if brief.correlation_flag { "yes" } else { "no" },
                brief.confidence,
                brief.as_of
            ));
        }

        parts.join("\n")
    }

    /// Estimate token count (rough: ~4 chars per token).
    pub fn token_count(&self) -> usize {
        self.build().len() / 4
    }

    /// Template hash for reproducibility.
    pub fn template_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.build().hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dococ_core::Symbol;

    #[test]
    fn builds_state_with_position() {
        let mut sb = StateBuilder::default();
        sb.position = Some(Position {
            symbol: Symbol::new("BTCUSDT"),
            quantity: 0.5,
            avg_price: 64_000.0,
        });
        sb.garch_vol = Some(0.012);
        let state = sb.build();
        assert!(state.contains("Position:"));
        assert!(state.contains("GARCH vol: 0.0120"));
    }

    #[test]
    fn builds_state_flat() {
        let sb = StateBuilder::default();
        let state = sb.build();
        assert!(state.contains("Position: flat"));
        assert!(state.contains("GARCH vol: NA"));
        assert!(state.contains("Regime: NA"));
    }

    #[test]
    fn token_count_reasonable() {
        let mut sb = StateBuilder::default();
        sb.bars = vec![Bar {
            timestamp: 1_700_000_000,
            open: 64_000.0,
            high: 64_500.0,
            low: 63_800.0,
            close: 64_200.0,
            volume: 1_000.0,
        }];
        sb.garch_vol = Some(0.012);
        sb.markov = Some(RegimeProbs {
            p_down: 0.2,
            p_range: 0.3,
            p_up: 0.5,
        });
        let tc = sb.token_count();
        assert!(tc < 200, "token count {} too high", tc);
        assert!(tc > 0);
    }

    #[test]
    fn template_hash_deterministic() {
        let sb = StateBuilder::default();
        let h1 = sb.template_hash();
        let h2 = sb.template_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn max_bars_limits_output() {
        let mut sb = StateBuilder::default();
        sb.max_bars = 2;
        for i in 0..5 {
            sb.bars.push(Bar {
                timestamp: 1_700_000_000 + i * 3600,
                open: 64_000.0,
                high: 64_500.0,
                low: 63_800.0,
                close: 64_200.0,
                volume: 1_000.0,
            });
        }
        let state = sb.build();
        // Should only include 2 bars.
        let bar_count = state.matches("O:").count();
        assert_eq!(bar_count, 2);
    }

    #[test]
    fn headlines_limited() {
        let mut sb = StateBuilder::default();
        sb.max_headlines = 2;
        for i in 0..5 {
            sb.headlines.push(format!("headline {}", i));
        }
        let state = sb.build();
        assert!(state.contains("headline 0"));
        assert!(state.contains("headline 1"));
        assert!(!state.contains("headline 4"));
    }
}
