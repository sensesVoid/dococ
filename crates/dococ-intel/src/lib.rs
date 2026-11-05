use std::process::Command;
use std::time::{Duration, Instant};

/// LLM brief schema.
#[derive(Debug, Clone)]
pub struct LlmBrief {
    pub macro_regime: String,
    pub news_bias: String,
    pub event_risk: String,
    pub correlation_flag: bool,
    pub confidence: f64,
    pub as_of: i64,
    pub sources: Vec<String>,
}

impl LlmBrief {
    /// Check if the brief is stale (older than 30 minutes).
pub fn is_stale(&self, now: i64, threshold: i64) -> bool {
        now - self.as_of > threshold
    }
}

/// Configuration for the LLM adapter.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub stale_threshold_secs: i64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: "opencode/deepseek-v4.1-flash-free".to_string(),
            timeout_secs: 120,
            max_retries: 2,
            stale_threshold_secs: 1800,
        }
    }
}

/// LLM adapter result.
#[derive(Debug, Clone)]
pub enum LlmResult {
    Ok(LlmBrief),
    Timeout,
    InvalidJson(String),
    Failed(String),
}

/// Adapter for calling `opencode run` as a subprocess.
pub struct LlmAdapter {
    config: LlmConfig,
}

impl LlmAdapter {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    /// Build the prompt for the LLM.
    pub fn build_prompt(&self, headlines: &[String], state_summary: &str) -> String {
        let headlines_text = if headlines.is_empty() {
            "No recent headlines.".to_string()
        } else {
            headlines
                .iter()
                .map(|h| format!("- {}", h))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "You are a macro analyst. Analyze the following market state and news. \
             Return ONLY a JSON object with these fields: \
             macro_regime (string), news_bias (string: bullish/bearish/neutral), \
             event_risk (string: low/medium/high), correlation_flag (boolean), \
             confidence (number 0-1), as_of (unix timestamp), sources (array of strings). \
             No markdown, no prose, only JSON.\n\n\
             State: {}\n\nHeadlines:\n{}",
            state_summary, headlines_text
        )
    }

    /// Parse a JSON response into an LlmBrief.
    pub fn parse_response(&self, json_str: &str, now: i64) -> Result<LlmBrief, String> {
        // Simple JSON parsing without serde (to avoid dependency).
        // In production, use serde_json.
        let trimmed = json_str.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err("response is not a JSON object".to_string());
        }

        // Extract fields using simple string parsing.
        // This is a simplified version; production should use serde_json.
        let macro_regime = extract_string_field(trimmed, "macro_regime").unwrap_or_else(|| "unknown".to_string());
        let news_bias = extract_string_field(trimmed, "news_bias").unwrap_or_else(|| "neutral".to_string());
        let event_risk = extract_string_field(trimmed, "event_risk").unwrap_or_else(|| "medium".to_string());
        let correlation_flag = extract_bool_field(trimmed, "correlation_flag").unwrap_or(false);
        let confidence = extract_number_field(trimmed, "confidence").unwrap_or(0.5);
        let as_of = extract_number_field(trimmed, "as_of").map(|v| v as i64).unwrap_or(now);

        Ok(LlmBrief {
            macro_regime,
            news_bias,
            event_risk,
            correlation_flag,
            confidence: confidence.clamp(0.0, 1.0),
            as_of,
            sources: Vec::new(),
        })
    }

    /// Check if a brief is stale.
    pub fn is_stale(brief: &LlmBrief, now: i64, threshold: i64) -> bool {
        now - brief.as_of > threshold
    }
}

// Simple JSON field extractors (production should use serde_json).
fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\"", field);
    let idx = json.find(&pattern)?;
    let after = &json[idx + pattern.len()..];
    let colon = after.find(':')?;
    let val_start = after[colon + 1..].trim_start();
    if val_start.starts_with('"') {
        let end = val_start[1..].find('"')?;
        Some(val_start[1..1 + end].to_string())
    } else {
        None
    }
}

fn extract_bool_field(json: &str, field: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", field);
    let idx = json.find(&pattern)?;
    let after = &json[idx + pattern.len()..];
    let colon = after.find(':')?;
    let val = after[colon + 1..].trim_start();
    if val.starts_with("true") {
        Some(true)
    } else if val.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_number_field(json: &str, field: &str) -> Option<f64> {
    let pattern = format!("\"{}\"", field);
    let idx = json.find(&pattern)?;
    let after = &json[idx + pattern.len()..];
    let colon = after.find(':')?;
    let val = after[colon + 1..].trim_start();
    // Read until comma, brace, or whitespace.
    let end = val.find(|c: char| c == ',' || c == '}' || c.is_whitespace()).unwrap_or(val.len());
    val[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json() {
        let adapter = LlmAdapter::new(LlmConfig::default());
        let json = r#"{"macro_regime":"risk-on","news_bias":"bullish","event_risk":"low","correlation_flag":true,"confidence":0.85,"as_of":1700000000,"sources":["reuters"]}"#;
        let brief = adapter.parse_response(json, 1700000000).expect("parse");
        assert_eq!(brief.macro_regime, "risk-on");
        assert_eq!(brief.news_bias, "bullish");
        assert!(brief.correlation_flag);
        assert!((brief.confidence - 0.85).abs() < 1e-9);
    }

    #[test]
    fn parse_invalid_json() {
        let adapter = LlmAdapter::new(LlmConfig::default());
        let result = adapter.parse_response("not json", 1700000000);
        assert!(result.is_err());
    }

    #[test]
    fn stale_brief_detection() {
        let brief = LlmBrief {
            macro_regime: "risk-on".into(),
            news_bias: "bullish".into(),
            event_risk: "low".into(),
            correlation_flag: false,
            confidence: 0.8,
            as_of: 1000,
            sources: vec![],
        };
        assert!(LlmBrief::is_stale(&brief, 3000, 1800)); // 2000s > 1800
        assert!(!LlmBrief::is_stale(&brief, 2000, 1800)); // 1000s < 1800
    }

    #[test]
    fn build_prompt_includes_headlines() {
        let adapter = LlmAdapter::new(LlmConfig::default());
        let prompt = adapter.build_prompt(&["headline 1".into(), "headline 2".into()], "BTC at 64k");
        assert!(prompt.contains("headline 1"));
        assert!(prompt.contains("headline 2"));
        assert!(prompt.contains("BTC at 64k"));
    }

    #[test]
    fn build_prompt_no_headlines() {
        let adapter = LlmAdapter::new(LlmConfig::default());
        let prompt = adapter.build_prompt(&[], "BTC at 64k");
        assert!(prompt.contains("No recent headlines"));
    }

    #[test]
    fn confidence_clamped() {
        let adapter = LlmAdapter::new(LlmConfig::default());
        let json = r#"{"macro_regime":"x","news_bias":"y","event_risk":"z","correlation_flag":false,"confidence":1.5,"as_of":1000}"#;
        let brief = adapter.parse_response(json, 1000).expect("parse");
        assert!((brief.confidence - 1.0).abs() < 1e-9);
    }
}
