use std::collections::{HashMap, VecDeque};

/// LRU cache for Laya runtime instances.
pub struct ModelCache {
    capacity: usize,
    cache: HashMap<String, CachedModel>,
    order: VecDeque<String>,
}

struct CachedModel {
    // In production, this would hold a LayaRuntime.
    // For now, we track metadata only to avoid lifetime issues.
    name: String,
    last_used: u64,
    access_count: u64,
}

impl ModelCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Check if a model is cached.
    pub fn contains(&self, name: &str) -> bool {
        self.cache.contains_key(name)
    }

    /// Number of cached models.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Capacity of the cache.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Insert a model into the cache. Evicts LRU if at capacity.
    pub fn insert(&mut self, name: &str) -> Option<String> {
        // If already present, update access time.
        if self.cache.contains_key(name) {
            self.touch(name);
            return None;
        }

        // Evict if at capacity.
        let evicted = if self.cache.len() >= self.capacity {
            if let Some(lru) = self.order.pop_front() {
                self.cache.remove(&lru);
                Some(lru)
            } else {
                None
            }
        } else {
            None
        };

        let access_count = self.order.len() as u64 + 1;
        self.cache.insert(
            name.to_string(),
            CachedModel {
                name: name.to_string(),
                last_used: access_count,
                access_count,
            },
        );
        self.order.push_back(name.to_string());
        evicted
    }

    /// Touch a model (mark as recently used).
    pub fn touch(&mut self, name: &str) {
        if self.cache.contains_key(name) {
            // Move to back of queue.
            self.order.retain(|n| n != name);
            self.order.push_back(name.to_string());
            if let Some(m) = self.cache.get_mut(name) {
                m.last_used = self.order.len() as u64;
            }
        }
    }

    /// Get the name of the least recently used model.
    pub fn lru(&self) -> Option<&str> {
        self.order.front().map(|s| s.as_str())
    }

    /// Remove a model from the cache.
    pub fn remove(&mut self, name: &str) -> bool {
        if self.cache.remove(name).is_some() {
            self.order.retain(|n| n != name);
            true
        } else {
            false
        }
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_is_empty() {
        let cache = ModelCache::new(2);
        assert!(cache.is_empty());
        assert_eq!(cache.capacity(), 2);
    }

    #[test]
    fn insert_and_contains() {
        let mut cache = ModelCache::new(2);
        assert!(cache.insert("a").is_none());
        assert!(cache.contains("a"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn evicts_lru_at_capacity() {
        let mut cache = ModelCache::new(2);
        cache.insert("a");
        cache.insert("b");
        let evicted = cache.insert("c");
        assert_eq!(evicted, Some("a".to_string()));
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn touch_prevents_eviction() {
        let mut cache = ModelCache::new(2);
        cache.insert("a");
        cache.insert("b");
        cache.touch("a"); // a is now most recently used
        let evicted = cache.insert("c");
        assert_eq!(evicted, Some("b".to_string())); // b evicted, not a
        assert!(cache.contains("a"));
    }

    #[test]
    fn remove_model() {
        let mut cache = ModelCache::new(2);
        cache.insert("a");
        assert!(cache.remove("a"));
        assert!(!cache.contains("a"));
        assert!(!cache.remove("a")); // already removed
    }

    #[test]
    fn clear_cache() {
        let mut cache = ModelCache::new(2);
        cache.insert("a");
        cache.insert("b");
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn capacity_minimum_one() {
        let cache = ModelCache::new(0);
        assert_eq!(cache.capacity(), 1);
    }

    #[test]
    fn lru_returns_oldest() {
        let mut cache = ModelCache::new(3);
        cache.insert("a");
        cache.insert("b");
        cache.insert("c");
        assert_eq!(cache.lru(), Some("a"));
    }
}
