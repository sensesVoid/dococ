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

use laya::{Question, QuestionSet};
use serde_json::Value;

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

#[cfg(test)]
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
}
