#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderAction {
    Long,
    Short,
    Hold,
    NoAction,
    Close,
}

impl OrderAction {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "LONG" => Some(Self::Long),
            "SHORT" => Some(Self::Short),
            "HOLD" => Some(Self::Hold),
            "NO_ACTION" => Some(Self::NoAction),
            "CLOSE" => Some(Self::Close),
            _ => None,
        }
    }
}

use std::path::Path;

use laya::{LayaEngine, LayaOptions, Question, QuestionSet};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum LayaDecisionError {
    #[error(transparent)]
    Laya(#[from] laya::LayaError),
    #[error("missing or unexpected answer for `{0}`")]
    UnexpectedAnswer(&'static str),
    #[error("unknown action label `{0}`")]
    UnknownAction(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionDecision {
    pub action: OrderAction,
    pub action_confidence: f64,
    pub edge_probability: f64,
    pub conviction: f64,
}

pub struct LayaRuntime {
    engine: LayaEngine,
}

impl LayaRuntime {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, LayaDecisionError> {
        Self::load_with_threads(directory, None)
    }

    pub fn load_with_threads(
        directory: impl AsRef<Path>,
        intra_op_threads: Option<u32>,
    ) -> Result<Self, LayaDecisionError> {
        let options = LayaOptions {
            model_directory: Some(directory.as_ref().to_path_buf()),
            intra_op_threads,
            ..LayaOptions::default()
        };
        Ok(Self {
            engine: LayaEngine::create(options)?,
        })
    }

    pub fn decide(&self, state: &str) -> Result<ActionDecision, LayaDecisionError> {
        let questions = build_questions()?;
        let answers = self.engine.predict(state, &questions)?;

        let action_answer = answers
            .get("action")
            .ok_or(LayaDecisionError::UnexpectedAnswer("action"))?;
        let choice = action_answer
            .as_choice()
            .ok_or(LayaDecisionError::UnexpectedAnswer("action"))?;
        let action = OrderAction::from_label(&choice.choice)
            .ok_or_else(|| LayaDecisionError::UnknownAction(choice.choice.clone()))?;

        let edge_answer = answers
            .get("edge")
            .ok_or(LayaDecisionError::UnexpectedAnswer("edge"))?;
        let edge = edge_answer
            .as_noul()
            .ok_or(LayaDecisionError::UnexpectedAnswer("edge"))?;

        let conviction_answer = answers
            .get("conviction")
            .ok_or(LayaDecisionError::UnexpectedAnswer("conviction"))?;
        let conviction = conviction_answer
            .as_score()
            .ok_or(LayaDecisionError::UnexpectedAnswer("conviction"))?;

        Ok(ActionDecision {
            action,
            action_confidence: action_answer.answer_confidence(),
            edge_probability: edge.probability,
            conviction: conviction.score,
        })
    }
}

pub fn build_questions() -> laya::Result<QuestionSet> {
    QuestionSet::new()
        .with(
            "action",
            Question::choice_labels(
                "What should the desk do?",
                ["LONG", "SHORT", "HOLD", "NO_ACTION", "CLOSE"],
            )?,
        )?
        .with(
            "edge",
            Question::noul(
                "Does this setup have positive expected value after costs?",
                Value::Null,
                Value::Null,
            ),
        )?
        .with(
            "conviction",
            Question::score("How strong is the conviction?", ["low", "medium", "high"])?,
        )
}

mod cache;
mod registry;
pub use cache::*;
pub use registry::*;

mod tests {
    use super::*;

    #[test]
    fn maps_laya_action_labels_to_order_actions() {
        assert_eq!(OrderAction::from_label("LONG"), Some(OrderAction::Long));
        assert_eq!(OrderAction::from_label("SHORT"), Some(OrderAction::Short));
        assert_eq!(OrderAction::from_label("HOLD"), Some(OrderAction::Hold));
        assert_eq!(
            OrderAction::from_label("NO_ACTION"),
            Some(OrderAction::NoAction)
        );
        assert_eq!(OrderAction::from_label("CLOSE"), Some(OrderAction::Close));
        assert_eq!(OrderAction::from_label("MAYBE"), None);
    }

    #[test]
    fn builds_action_edge_and_conviction_questions() {
        let questions = build_questions().expect("questions should build");
        let ids: Vec<&str> = questions.ids().collect();
        assert_eq!(ids, vec!["action", "edge", "conviction"]);
    }

    #[test]
    fn load_fails_for_missing_model_directory() {
        let result = LayaRuntime::load("definitely-not-a-model-directory");
        assert!(result.is_err());
    }

    #[test]
    fn load_with_threads_fails_for_missing_model_directory() {
        let result = LayaRuntime::load_with_threads("definitely-not-a-model-directory", Some(4));
        assert!(result.is_err());
    }
}
