use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Lifecycle stage of an asset pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Shadow,
    Paper,
    Live,
}

/// Asset pack: instrument metadata, model path, calibration, thresholds, stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPack {
    pub asset: String,
    pub instrument: String,
    pub model_path: PathBuf,
    pub model_name: String,
    pub calibration_temp: f64,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub stage: Stage,
    pub garch_params: PathBuf,
    pub markov_params: PathBuf,
}

impl AssetPack {
    pub fn new(asset: &str) -> Self {
        Self {
            asset: asset.to_string(),
            instrument: asset.to_string(),
            model_path: PathBuf::from(format!("models/{}/model.onnx", asset.to_lowercase())),
            model_name: format!("{}-v1", asset.to_lowercase()),
            calibration_temp: 1.0,
            entry_threshold: 0.6,
            exit_threshold: 0.4,
            stage: Stage::Shadow,
            garch_params: PathBuf::from(format!("models/{}/garch.json", asset.to_lowercase())),
            markov_params: PathBuf::from(format!("models/{}/markov.json", asset.to_lowercase())),
        }
    }

    /// Validate pack contents.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.asset.is_empty() {
            return Err("asset is empty");
        }
        if !(0.0..1.0).contains(&self.entry_threshold) {
            return Err("entry_threshold out of range");
        }
        if !(0.0..1.0).contains(&self.exit_threshold) {
            return Err("exit_threshold out of range");
        }
        if self.calibration_temp <= 0.0 {
            return Err("calibration_temp must be positive");
        }
        Ok(())
    }

    /// Advance the stage (Shadow -> Paper -> Live).
    pub fn promote(&mut self) -> Result<Stage, &'static str> {
        match self.stage {
            Stage::Shadow => self.stage = Stage::Paper,
            Stage::Paper => self.stage = Stage::Live,
            Stage::Live => return Err("already live"),
        }
        Ok(self.stage)
    }

    /// Demote the stage.
    pub fn demote(&mut self) -> Result<Stage, &'static str> {
        match self.stage {
            Stage::Shadow => return Err("already shadow"),
            Stage::Paper => self.stage = Stage::Shadow,
            Stage::Live => self.stage = Stage::Paper,
        }
        Ok(self.stage)
    }

    /// Serialize to JSON (metadata format used by `dococ asset`).
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Load from JSON.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pack_validates() {
        let pack = AssetPack::new("BTCUSDT");
        assert!(pack.validate().is_ok());
        assert_eq!(pack.stage, Stage::Shadow);
    }

    #[test]
    fn json_roundtrip() {
        let pack = AssetPack::new("BTCUSDT");
        let json = pack.to_json().unwrap();
        let loaded = AssetPack::from_json(&json).unwrap();
        assert_eq!(loaded.asset, "BTCUSDT");
        assert_eq!(loaded.model_name, "btcusdt-v1");
    }

    #[test]
    fn promote_and_demote() {
        let mut pack = AssetPack::new("BTCUSDT");
        assert_eq!(pack.promote(), Ok(Stage::Paper));
        assert_eq!(pack.promote(), Ok(Stage::Live));
        assert!(pack.promote().is_err());
        assert_eq!(pack.demote(), Ok(Stage::Paper));
        assert_eq!(pack.demote(), Ok(Stage::Shadow));
        assert!(pack.demote().is_err());
    }

    #[test]
    fn invalid_pack_rejected() {
        let mut pack = AssetPack::new("BTCUSDT");
        pack.calibration_temp = -1.0;
        assert!(pack.validate().is_err());
        pack.calibration_temp = 1.0;
        pack.entry_threshold = 2.0;
        assert!(pack.validate().is_err());
    }
}
