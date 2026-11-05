use std::time::{SystemTime, UNIX_EPOCH};

/// Structured LLM brief schema.
#[derive(Debug, Clone)]
pub struct LlmBriefSchema {
    pub macro_regime: String,
    pub news_bias: String,
    pub event_risk: String,
    pub correlation_flag: bool,
    pub confidence: f64,
    pub as_of: i64,
    pub sources: Vec<String>,
    pub stale: bool,
    pub model_alias: String,
}

impl LlmBriefSchema {
    pub fn new(macro_regime: &str, news_bias: &str, event_risk: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            macro_regime: macro_regime.to_string(),
            news_bias: news_bias.to_string(),
            event_risk: event_risk.to_string(),
            correlation_flag: false,
            confidence: 0.5,
            as_of: now,
            sources: Vec::new(),
            stale: false,
            model_alias: String::new(),
        }
    }

    /// Check if the brief is stale (older than threshold seconds).
    pub fn is_stale(&self, now: i64, threshold: i64) -> bool {
        now - self.as_of > threshold
    }

    /// Deterministic fallback when LLM fails or times out.
    pub fn fallback(now: i64) -> Self {
        Self {
            macro_regime: "unknown".to_string(),
            news_bias: "neutral".to_string(),
            event_risk: "medium".to_string(),
            correlation_flag: false,
            confidence: 0.0,
            as_of: now,
            sources: Vec::new(),
            stale: true,
            model_alias: "fallback".to_string(),
        }
    }

    /// Validate that all required fields are present and valid.
    pub fn validate(&self) -> Result<(), String> {
        if self.macro_regime.is_empty() {
            return Err("macro_regime is empty".to_string());
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err("confidence out of range".to_string());
        }
        if self.as_of <= 0 {
            return Err("invalid as_of timestamp".to_string());
        }
        Ok(())
    }
}

/// Convert to the simpler LlmBrief (from lib.rs).
impl From<LlmBriefSchema> for crate::LlmBrief {
    fn from(schema: LlmBriefSchema) -> Self {
        crate::LlmBrief {
            macro_regime: schema.macro_regime,
            news_bias: schema.news_bias,
            event_risk: schema.event_risk,
            correlation_flag: schema.correlation_flag,
            confidence: schema.confidence,
            as_of: schema.as_of,
            sources: schema.sources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_brief_has_defaults() {
        let brief = LlmBriefSchema::new("risk-on", "bullish", "low");
        assert_eq!(brief.macro_regime, "risk-on");
        assert_eq!(brief.news_bias, "bullish");
        assert_eq!(brief.event_risk, "low");
        assert!(!brief.stale);
    }

    #[test]
    fn fallback_is_stale() {
        let brief = LlmBriefSchema::fallback(1000);
        assert!(brief.stale);
        assert_eq!(brief.confidence, 0.0);
        assert_eq!(brief.macro_regime, "unknown");
    }

    #[test]
    fn validate_ok() {
        let brief = LlmBriefSchema::new("risk-on", "bullish", "low");
        assert!(brief.validate().is_ok());
    }

    #[test]
    fn validate_empty_regime() {
        let mut brief = LlmBriefSchema::new("", "bullish", "low");
        assert!(brief.validate().is_err());
    }

    #[test]
    fn validate_bad_confidence() {
        let mut brief = LlmBriefSchema::new("risk-on", "bullish", "low");
        brief.confidence = 1.5;
        assert!(brief.validate().is_err());
    }

    #[test]
    fn stale_detection() {
        let brief = LlmBriefSchema::new("risk-on", "bullish", "low");
        assert!(!brief.is_stale(brief.as_of + 100, 1800));
        assert!(brief.is_stale(brief.as_of + 2000, 1800));
    }
}
