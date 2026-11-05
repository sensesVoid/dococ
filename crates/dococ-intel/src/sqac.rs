use std::time::{Duration, Instant};

/// A retrieved case from SQAC memory.
#[derive(Debug, Clone)]
pub struct SqacCase {
    pub timestamp: i64,
    pub content: String,
    pub relevance: f64,
}

/// SQAC retrieval configuration.
#[derive(Debug, Clone)]
pub struct SqacConfig {
    pub max_cases: usize,
    pub timeout_ms: u64,
    pub min_relevance: f64,
}

impl Default for SqacConfig {
    fn default() -> Self {
        Self {
            max_cases: 3,
            timeout_ms: 5000,
            min_relevance: 0.3,
        }
    }
}

/// SQAC adapter for retrieving time-filtered memory cases.
pub struct SqacAdapter {
    config: SqacConfig,
}

impl SqacAdapter {
    pub fn new(config: SqacConfig) -> Self {
        Self { config }
    }

    /// Filter cases to only those before the decision time.
    pub fn filter_before(&self, cases: Vec<SqacCase>, decision_time: i64) -> Vec<SqacCase> {
        cases
            .into_iter()
            .filter(|c| c.timestamp < decision_time)
            .filter(|c| c.relevance >= self.config.min_relevance)
            .collect()
    }

    /// Select top N cases by relevance.
    pub fn select_top(&self, cases: Vec<SqacCase>) -> Vec<SqacCase> {
        let mut sorted = cases;
        sorted.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.timestamp.cmp(&a.timestamp))
        });
        sorted.into_iter().take(self.config.max_cases).collect()
    }

    /// Retrieve cases with timeout. Returns empty vec on timeout.
    pub fn retrieve_with_timeout<F>(
        &self,
        fetch: F,
        decision_time: i64,
    ) -> Vec<SqacCase>
    where
        F: FnOnce() -> Vec<SqacCase>,
    {
        let start = Instant::now();
        let cases = fetch();
        let elapsed = start.elapsed();

        if elapsed > Duration::from_millis(self.config.timeout_ms) {
            return Vec::new(); // Timeout: return empty
        }

        let filtered = self.filter_before(cases, decision_time);
        self.select_top(filtered)
    }

    /// Format cases for inclusion in the Laya state.
    pub fn format_cases(cases: &[SqacCase]) -> String {
        if cases.is_empty() {
            return "NA".to_string();
        }
        cases
            .iter()
            .map(|c| format!("[{}] {}", c.timestamp, c.content))
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_before_removes_future_cases() {
        let adapter = SqacAdapter::new(SqacConfig::default());
        let cases = vec![
            SqacCase { timestamp: 100, content: "old".into(), relevance: 0.8 },
            SqacCase { timestamp: 200, content: "future".into(), relevance: 0.9 },
        ];
        let result = adapter.filter_before(cases, 150);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "old");
    }

    #[test]
    fn filter_removes_low_relevance() {
        let adapter = SqacAdapter::new(SqacConfig::default());
        let cases = vec![
            SqacCase { timestamp: 100, content: "low".into(), relevance: 0.1 },
            SqacCase { timestamp: 100, content: "high".into(), relevance: 0.8 },
        ];
        let result = adapter.filter_before(cases, 200);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "high");
    }

    #[test]
    fn select_top_limits_count() {
        let adapter = SqacAdapter::new(SqacConfig { max_cases: 2, ..Default::default() });
        let cases = vec![
            SqacCase { timestamp: 100, content: "a".into(), relevance: 0.5 },
            SqacCase { timestamp: 200, content: "b".into(), relevance: 0.9 },
            SqacCase { timestamp: 300, content: "c".into(), relevance: 0.7 },
        ];
        let result = adapter.select_top(cases);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "b");
    }

    #[test]
    fn retrieve_with_timeout_returns_empty_on_timeout() {
        let adapter = SqacAdapter::new(SqacConfig { timeout_ms: 1, ..Default::default() });
        let result = adapter.retrieve_with_timeout(
            || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                vec![SqacCase { timestamp: 100, content: "late".into(), relevance: 0.9 }]
            },
            200,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn retrieve_with_timeout_returns_cases() {
        let adapter = SqacAdapter::new(SqacConfig::default());
        let result = adapter.retrieve_with_timeout(
            || vec![SqacCase { timestamp: 100, content: "case".into(), relevance: 0.8 }],
            200,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "case");
    }

    #[test]
    fn format_cases_empty() {
        assert_eq!(SqacAdapter::format_cases(&[]), "NA");
    }

    #[test]
    fn format_cases_nonempty() {
        let cases = vec![SqacCase { timestamp: 100, content: "test case".into(), relevance: 0.8 }];
        let formatted = SqacAdapter::format_cases(&cases);
        assert!(formatted.contains("test case"));
    }
}
