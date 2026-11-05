use std::time::{SystemTime, UNIX_EPOCH};

/// Audit log entry for LLM decisions.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: i64,
    pub prompt_hash: String,
    pub response_hash: String,
    pub model_alias: String,
    pub as_of: i64,
    pub stale: bool,
    pub valid: bool,
    pub latency_ms: u64,
}

impl AuditEntry {
    pub fn new(
        prompt: &str,
        response: &str,
        model: &str,
        as_of: i64,
        stale: bool,
        valid: bool,
        latency_ms: u64,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            timestamp: now,
            prompt_hash: simple_hash(prompt),
            response_hash: simple_hash(response),
            model_alias: model.to_string(),
            as_of,
            stale,
            valid,
            latency_ms,
        }
    }
}

/// Simple hash function (not cryptographic, for audit trail).
fn simple_hash(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{:016x}", hash)
}

/// Audit log.
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    max_entries: usize,
}

impl AuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.max(1),
        }
    }

    pub fn record(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the last entry.
    pub fn last(&self) -> Option<&AuditEntry> {
        self.entries.last()
    }

    /// Count valid entries.
    pub fn valid_count(&self) -> usize {
        self.entries.iter().filter(|e| e.valid).count()
    }

    /// Count stale entries.
    pub fn stale_count(&self) -> usize {
        self.entries.iter().filter(|e| e.stale).count()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let mut log = AuditLog::new(10);
        let entry = AuditEntry::new("prompt", "response", "model", 1000, false, true, 50);
        log.record(entry);
        assert_eq!(log.len(), 1);
        assert_eq!(log.valid_count(), 1);
        assert_eq!(log.stale_count(), 0);
    }

    #[test]
    fn max_entries_evicts_oldest() {
        let mut log = AuditLog::new(2);
        log.record(AuditEntry::new("p1", "r1", "m", 1000, false, true, 10));
        log.record(AuditEntry::new("p2", "r2", "m", 2000, false, true, 20));
        log.record(AuditEntry::new("p3", "r3", "m", 3000, false, true, 30));
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries()[0].as_of, 2000);
    }

    #[test]
    fn hash_deterministic() {
        let h1 = simple_hash("test");
        let h2 = simple_hash("test");
        assert_eq!(h1, h2);
        assert_ne!(simple_hash("test"), simple_hash("other"));
    }

    #[test]
    fn stale_count() {
        let mut log = AuditLog::new(10);
        log.record(AuditEntry::new("p1", "r1", "m", 1000, true, true, 10));
        log.record(AuditEntry::new("p2", "r2", "m", 2000, false, true, 20));
        assert_eq!(log.stale_count(), 1);
    }
}
