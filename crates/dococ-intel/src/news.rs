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
    items
        .into_iter()
        .filter(|i| i.timestamp < decision_time)
        .collect()
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

/// Parse an economic calendar JSON (Forex Factory format).
pub fn parse_economic_calendar(json: &str, now: i64) -> Vec<NewsItem> {
    // Simplified: expects a JSON array of objects with title, country, date, impact.
    // We parse manually without serde.
    let mut items = Vec::new();
    // Find array elements.
    let mut pos = 0;
    while let Some(start) = json[pos..].find("\"title\"") {
        let abs = pos + start;
        // Find the enclosing object.
        let obj_start = json[..abs].rfind('{').unwrap_or(abs);
        let obj_end = json[abs..]
            .find('}')
            .map(|e| abs + e + 1)
            .unwrap_or(json.len());
        let block = &json[obj_start..obj_end];
        pos = obj_end;

        let title = extract_json_string(block, "title").unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let country = extract_json_string(block, "country").unwrap_or_default();
        let impact = extract_json_string(block, "impact").unwrap_or_default();

        let headline = if country.is_empty() {
            format!("{} [{}]", title, impact)
        } else {
            format!("{} ({}) [{}]", title, country, impact)
        };

        items.push(NewsItem::new(now, "economic_calendar", &headline));
    }
    items
}

fn parse_rfc2822(s: &str) -> Option<i64> {
    // Simplified: try to parse "Day, DD Mon YYYY HH:MM:SS TZ"
    // For now, return None (caller uses `now`).
    None
}

/// Parse exchange announcements (simplified JSON array).
pub fn parse_exchange_announcements(json: &str, source: &str) -> Vec<NewsItem> {
    let mut items = Vec::new();
    let mut pos = 0;
    while let Some(start) = json[pos..].find("\"title\"") {
        let abs = pos + start;
        let obj_start = json[..abs].rfind('{').unwrap_or(abs);
        let obj_end = json[abs..]
            .find('}')
            .map(|e| abs + e + 1)
            .unwrap_or(json.len());
        let block = &json[obj_start..obj_end];
        pos = obj_end;

        let title = extract_json_string(block, "title").unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let url = extract_json_string(block, "url");
        let mut item = NewsItem::new(0, source, &title);
        item.url = url;
        item.relevance = 0.9; // Exchange announcements are high relevance
        items.push(item);
    }
    items
}
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
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
    fn parse_economic_calendar_test() {
        let json = r#"[{"title":"Non-Farm Payrolls","country":"USD","date":"2026-11-05","impact":"High","forecast":"200K","previous":"180K"}]"#;
        let items = parse_economic_calendar(json, 1000);
        assert_eq!(items.len(), 1);
        assert!(items[0].headline.contains("Non-Farm Payrolls"));
        assert!(items[0].headline.contains("USD"));
    }

    #[test]
    fn parse_exchange_announcements_test() {
        let json = r#"[{"title":"New listing: XYZ/USDT","url":"https://example.com/1"}]"#;
        let items = parse_exchange_announcements(json, "binance");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].headline, "New listing: XYZ/USDT");
        assert_eq!(items[0].source, "binance");
        assert!((items[0].relevance - 0.9).abs() < 1e-9);
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
