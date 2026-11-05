use dococ_laya::{LayaRuntime, OrderAction};

const STATE: &str = "\
Account: paper. Equity 100000 USD. Position: flat.
Market: BTC/USDT perpetual. Last price 64000. 4h trend: up.
GARCH conditional volatility: 0.012. Markov regime: trending.
News: spot ETF inflows positive; funding neutral.";

#[test]
#[ignore = "requires an exported Laya multilingual ONNX directory in DOCOC_LAYA_MODEL"]
fn decides_with_exported_multilingual_model() {
    let model_dir = std::env::var("DOCOC_LAYA_MODEL")
        .expect("set DOCOC_LAYA_MODEL to the exported multilingual directory");

    let runtime = LayaRuntime::load(model_dir).expect("load Laya model");
    let decision = runtime.decide(STATE).expect("run a Laya decision");

    assert!(matches!(
        decision.action,
        OrderAction::Long
            | OrderAction::Short
            | OrderAction::Hold
            | OrderAction::NoAction
            | OrderAction::Close
    ));
    assert!((0.0..=1.0).contains(&decision.action_confidence));
    assert!((0.0..=1.0).contains(&decision.edge_probability));
    assert!((0.0..=2.0).contains(&decision.conviction));
}
