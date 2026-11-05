use dococ_laya::{LayaRuntime, OrderAction};
use std::path::PathBuf;

/// Multilingual fine-tuned BTC checkpoint golden test.
#[test]
#[ignore = "requires the multilingual fine-tuned BTC ONNX model via DOCOC_LAYA_MODEL (models/btcusdt_ml dir)"]
fn golden_finetuned_btc_ml() {
    let model_dir = std::env::var("DOCOC_LAYA_MODEL")
        .expect("set DOCOC_LAYA_MODEL to the multilingual fine-tuned BTC model directory");
    assert!(
        model_dir.contains("btcusdt_ml"),
        "this test expects the multilingual fine-tuned BTC model, got: {}",
        model_dir
    );
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden_btcusdt_ml_fixture.json");
    let text = std::fs::read_to_string(path).expect("read fixture");
    let fix: serde_json::Value = serde_json::from_str(&text).expect("parse fixture");
    let expected = &fix["expected"];
    let tol = fix["tolerance"].as_f64().unwrap();

    let runtime = LayaRuntime::load(model_dir).expect("load multilingual fine-tuned model");
    let decision = runtime.decide(fix["state"].as_str().unwrap()).expect("decide");

    let expected_action = match expected["action"].as_str().unwrap() {
        "Long" => OrderAction::Long,
        "Short" => OrderAction::Short,
        "Hold" => OrderAction::Hold,
        "NoAction" => OrderAction::NoAction,
        "Close" => OrderAction::Close,
        other => panic!("unknown golden action: {other}"),
    };
    assert_eq!(decision.action, expected_action);
    for (actual, key) in [
        (decision.action_confidence, "action_confidence"),
        (decision.edge_probability, "edge_probability"),
        (decision.conviction, "conviction"),
    ] {
        let want = expected[key].as_f64().unwrap();
        assert!(
            (actual - want).abs() < tol,
            "{key}: {actual} vs golden {want}"
        );
    }
}
