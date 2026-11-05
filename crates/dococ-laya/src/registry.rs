use std::collections::HashMap;
use std::path::PathBuf;

/// Model metadata.
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub asset: String,
    pub sha256: String,
    pub path: PathBuf,
    pub created_at: i64,
    pub calibration_temp: f64,
    pub context_length: usize,
}

/// Model registry for managing Laya checkpoints.
pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    active: Option<String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            active: None,
        }
    }

    /// Register a model.
    pub fn register(&mut self, meta: ModelMetadata) {
        let name = meta.name.clone();
        self.models.insert(name, meta);
    }

    /// Get a model by name.
    pub fn get(&self, name: &str) -> Option<&ModelMetadata> {
        self.models.get(name)
    }

    /// List all registered models.
    pub fn list(&self) -> Vec<&ModelMetadata> {
        self.models.values().collect()
    }

    /// Set the active model.
    pub fn set_active(&mut self, name: &str) -> Result<(), &'static str> {
        if !self.models.contains_key(name) {
            return Err("model not registered");
        }
        self.active = Some(name.to_string());
        Ok(())
    }

    /// Get the active model.
    pub fn active(&self) -> Option<&ModelMetadata> {
        self.active.as_ref().and_then(|name| self.models.get(name))
    }

    /// Remove a model.
    pub fn remove(&mut self, name: &str) -> Option<ModelMetadata> {
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
        self.models.remove(name)
    }

    /// Check if a model is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.models.contains_key(name)
    }

    /// Verify the SHA-256 hash of a model file.
    pub fn verify_sha256(&self, name: &str, file_bytes: &[u8]) -> Result<bool, &'static str> {
        let meta = self.get(name).ok_or("model not registered")?;
        Ok(sha256_hex(file_bytes) == meta.sha256)
    }

    /// Number of registered models.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    let mut hex = String::with_capacity(64);
    for b in &hash {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Invalid action handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionValidation {
    Valid,
    Invalid(String),
}

pub fn validate_action(action_str: &str) -> ActionValidation {
    match action_str {
        "LONG" | "SHORT" | "HOLD" | "NO_ACTION" | "CLOSE" => ActionValidation::Valid,
        _ => ActionValidation::Invalid(format!("unknown action: {}", action_str)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_meta(name: &str) -> ModelMetadata {
        ModelMetadata {
            name: name.to_string(),
            version: "1.0".to_string(),
            asset: "BTCUSDT".to_string(),
            sha256: "abc123".to_string(),
            path: PathBuf::from("/models/test"),
            created_at: 1000,
            calibration_temp: 1.0,
            context_length: 1024,
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = ModelRegistry::new();
        reg.register(make_meta("btc-v1"));
        assert!(reg.contains("btc-v1"));
        assert_eq!(reg.get("btc-v1").unwrap().name, "btc-v1");
    }

    #[test]
    fn set_active() {
        let mut reg = ModelRegistry::new();
        reg.register(make_meta("btc-v1"));
        assert!(reg.set_active("btc-v1").is_ok());
        assert_eq!(reg.active().unwrap().name, "btc-v1");
    }

    #[test]
    fn set_active_unknown_fails() {
        let mut reg = ModelRegistry::new();
        assert!(reg.set_active("unknown").is_err());
    }

    #[test]
    fn remove_model() {
        let mut reg = ModelRegistry::new();
        reg.register(make_meta("btc-v1"));
        reg.set_active("btc-v1").unwrap();
        let removed = reg.remove("btc-v1");
        assert!(removed.is_some());
        assert!(reg.active().is_none());
    }

    #[test]
    fn list_models() {
        let mut reg = ModelRegistry::new();
        reg.register(make_meta("btc-v1"));
        reg.register(make_meta("btc-v2"));
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn validate_action_valid() {
        assert_eq!(validate_action("LONG"), ActionValidation::Valid);
        assert_eq!(validate_action("SHORT"), ActionValidation::Valid);
        assert_eq!(validate_action("HOLD"), ActionValidation::Valid);
        assert_eq!(validate_action("NO_ACTION"), ActionValidation::Valid);
        assert_eq!(validate_action("CLOSE"), ActionValidation::Valid);
    }

    #[test]
    fn validate_action_invalid() {
        assert!(matches!(validate_action("BUY"), ActionValidation::Invalid(_)));
        assert!(matches!(validate_action(""), ActionValidation::Invalid(_)));
    }

    #[test]
    fn verify_sha256_correct() {
        let mut reg = ModelRegistry::new();
        let mut meta = make_meta("btc-v1");
        meta.sha256 = sha256_hex(b"hello");
        reg.register(meta);
        assert!(reg.verify_sha256("btc-v1", b"hello").unwrap());
        assert!(!reg.verify_sha256("btc-v1", b"world").unwrap());
    }

    #[test]
    fn verify_sha256_unknown_model() {
        let reg = ModelRegistry::new();
        assert!(reg.verify_sha256("unknown", b"hello").is_err());
    }
}
