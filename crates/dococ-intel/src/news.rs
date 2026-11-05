use std::collections::HashSet;

/// A news item with timestamp for time-filtering.
#[derive(Debug, Clone)]
pub struct NewsItem {
    pub timestamp: i64,
    pub source: String,
    pub headline: String,
    pub url: Option<String>,
    pub relevance: f64,
}

impl NewsItem {
    pub fn new(timestamp: i64, source: &str, headline: &str) -> Self {
        Self {
            timestamp,
            source: source.to_string(),
            headline: headline.to_string(),
            url: None,
            relevance: 1.0,
        }
    }
}

/// Deduplicate news items by headline hash.
pub fn deduplicate(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        // Simple dedup by lowercased headline.
        let key = item.headline.to_lowercase();
        if seen.insert(key) {
            result.push(item);
        }
    }
    result
}

/// Filter news items to only those before the decision time.
pub fn filter_before(items: Vec<NewsItem>, decision_time: i64) -> Vec<NewsItem> {
    items.into_iter().filter(|i| i.timestamp < decision_time).collect()
}

/// Sort news items by relevance (descending) then timestamp (descending).
pub fn sort_by_relevance(items: &mut [NewsItem]) {
    items.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.timestamp.cmp(&a.timestamp))
    });
}

/// Select top N news items.
pub fn select_top(items: &[NewsItem], n: usize) -> Vec<NewsItem> {
    let mut sorted = items.to_vec();
    sort_by_relevance(&mut sorted);
    sorted.into_iter().take(n).collect()
}

/// News feed configuration.
#[derive(Debug, Clone)]
pub struct NewsConfig {
    pub gdelt_rate_limit_ms: u64,
    pub max_headlines: usize,
    pub max_age_hours: i64,
}

impl Default for NewsConfig {
    fn default() -> Self {
        Self {
            gdelt_rate_limit_ms: 5000,
            max_headlines: 3,
            max_age_hours: 24,
        }
    }
}

/// Parse an RSS feed (simplified).
pub fn parse_rss(xml: &str, source: &str, now: i64) -> Vec<NewsItem> {
    let mut items = Vec::new();
    // Simple XML parsing: find <item> blocks.
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<item>") {
        let abs_start = pos + start;
        let end = match xml[abs_start..].find("</item>") {
            Some(e) => abs_start + e + 7,
            None => break,
        };
        let block = &xml[abs_start..end];
        pos = end;

        let title = extract_tag(block, "title").unwrap_or_default();
        let link = extract_tag(block, "link");
        let pub_date = extract_tag(block, "pubDate");

        if title.is_empty() {
            continue;
        }

        let timestamp = pub_date
            .as_ref()
            .and_then(|d| parse_rfc2822(d))
            .unwrap_or(now);

        let mut item = NewsItem::new(timestamp, source, &title);
        item.url = link;
        items.push(item);
    }
    items
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)?;
    let end = xml[start..].find(&close)?;
    Some(xml[start + open.len()..start + end].trim().to_string())
}

fn parse_rfc2822(s: &str) -> Option<i64> {
    // Simplified: try to parse "Day, DD Mon YYYY HH:MM:SS TZ"
    // For now, return None (caller uses `now`).
    // Full implementation would use chrono or manual parsing.
    None
}

/// GDELT rate limiter state.
pub struct GdeltRateLimiter {
    last_request: Option<i64>,
    min_interval_ms: u64,
}

impl GdeltRateLimiter {
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            last_request: None,
            min_interval_ms,
        }
    }

    pub fn can_request(&self, now: i64) -> bool {
        match self.last_request {
            None => true,
            Some(last) => (now - last) * 1000 >= self.min_interval_ms as i64,
        }
    }

    pub fn record_request(&mut self, now: i64) {
        self.last_request = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicate_removes_duplicates() {
        let items = vec![
            NewsItem::new(100, "rss", "Bitcoin hits new high"),
            NewsItem::new(200, "rss", "bitcoin hits new high"), // duplicate (case-insensitive)
            NewsItem::new(300, "gdelt", "Fed raises rates"),
        ];
        let result = deduplicate(items);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_before_removes_future() {
        let items = vec![
            NewsItem::new(100, "rss", "old news"),
            NewsItem::new(200, "rss", "future news"),
        ];
        let result = filter_before(items, 150);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].headline, "old news");
    }

    #[test]
    fn select_top_limits_count() {
        let items = vec![
            NewsItem::new(100, "rss", "low").tap(|i| i.relevance = 0.3),
            NewsItem::new(200, "rss", "high").tap(|i| i.relevance = 0.9),
            NewsItem::new(300, "rss", "mid").tap(|i| i.relevance = 0.5),
        ];
        let result = select_top(&items, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].headline, "high");
    }

    #[test]
    fn gdelt_rate_limiter() {
        let mut rl = GdeltRateLimiter::new(5000);
        assert!(rl.can_request(0)); // first request always ok
        rl.record_request(0);
        assert!(!rl.can_request(1)); // 1s < 5s
        assert!(rl.can_request(6)); // 6s > 5s
    }

    #[test]
    fn parse_rss_extracts_items() {
        let xml = r#"<rss><channel><item><title>Bitcoin surges</title><link>https://example.com/1</link></item><item><title>Ethereum update</title></item></channel></rss>"#;
        let items = parse_rss(xml, "coindesk", 1000);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].headline, "Bitcoin surges");
        assert_eq!(items[1].headline, "Ethereum update");
    }
}

// Helper trait for tests.
trait Tap {
    fn tap<F: FnOnce(&mut Self)>(self, f: F) -> Self;
}
impl<T> Tap for T {
    fn tap<F: FnOnce(&mut Self)>(mut self, f: F) -> Self {
        f(&mut self);
        self
    }
}
