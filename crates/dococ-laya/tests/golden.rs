use dococ_laya::{LayaRuntime, OrderAction};
use std::path::PathBuf;

fn fixture() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden_fixture.json");
    let text = std::fs::read_to_string(path).expect("read golden fixture");
    serde_json::from_str(&text).expect("parse golden fixture")
}

fn action_from_name(name: &str) -> OrderAction {
    match name {
        "Long" => OrderAction::Long,
        "Short" => OrderAction::Short,
        "Hold" => OrderAction::Hold,
        "NoAction" => OrderAction::NoAction,
        "Close" => OrderAction::Close,
        other => panic!("unknown golden action: {other}"),
    }
}

/// Golden fixture integrity: the recorded Python reference values must be
/// present and sane. This runs in CI without the ONNX model.
#[test]
fn golden_fixture_values_are_sane() {
    let fix = fixture();
    let expected = &fix["expected"];
    assert_eq!(expected["action"], "Hold");
    for key in ["action_confidence", "edge_probability"] {
        let v = expected[key].as_f64().expect("numeric field");
        assert!((0.0..=1.0).contains(&v), "{key}={v} out of range");
    }
    let c = expected["conviction"].as_f64().expect("conviction numeric");
    assert!((0.0..=2.0).contains(&c), "conviction={c} out of range");
    assert!(fix["tolerance"].as_f64().unwrap() > 0.0);
}

/// Python/Rust parity golden test: needs the exported ONNX directory.
/// The known Python reference (smoke state) is Hold / 0.5918 / 0.8116 / 1.1479.
#[test]
#[ignore = "requires an exported Laya multilingual ONNX directory in DOCOC_LAYA_MODEL"]
fn golden_parity_with_python() {
    let model_dir = std::env::var("DOCOC_LAYA_MODEL")
        .expect("set DOCOC_LAYA_MODEL to the exported multilingual directory");
    let fix = fixture();
    let expected = &fix["expected"];
    let tol = fix["tolerance"].as_f64().unwrap();

    let runtime = LayaRuntime::load(model_dir).expect("load Laya model");
    let decision = runtime.decide(fix["state"].as_str().unwrap()).expect("decide");

    assert_eq!(decision.action, action_from_name(expected["action"].as_str().unwrap()));
    assert!(
        (decision.action_confidence - expected["action_confidence"].as_f64().unwrap()).abs() < tol,
        "action_confidence {} vs golden {}",
        decision.action_confidence,
        expected["action_confidence"]
    );
    assert!(
        (decision.edge_probability - expected["edge_probability"].as_f64().unwrap()).abs() < tol,
        "edge_probability {} vs golden {}",
        decision.edge_probability,
        expected["edge_probability"]
    );
    assert!(
        (decision.conviction - expected["conviction"].as_f64().unwrap()).abs() < tol,
        "conviction {} vs golden {}",
        decision.conviction,
        expected["conviction"]
    );
}
