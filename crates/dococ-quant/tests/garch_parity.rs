use dococ_quant::{GarchParams, garch_conditional_variance};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct Fixture {
    returns: Vec<f64>,
    params: Params,
    expected_vol: Vec<f64>,
}
#[derive(Deserialize)]
struct Params {
    omega: f64,
    alpha: f64,
    beta: f64,
}

#[test]
fn garch_matches_arch_conditional_volatility() {
    let raw = fs::read_to_string("tests/garch_fixture.json").unwrap();
    let f: Fixture = serde_json::from_str(&raw).unwrap();
    let params = GarchParams {
        omega: f.params.omega,
        alpha: f.params.alpha,
        beta: f.params.beta,
    };
    let var = garch_conditional_variance(&f.returns, &params);
    assert_eq!(var.len(), f.expected_vol.len());
    let mut maxdiff = 0.0f64;
    for (v, ev) in var.iter().zip(f.expected_vol.iter()) {
        let diff = (v.sqrt() - *ev).abs();
        if diff > maxdiff {
            maxdiff = diff;
        }
    }
    println!("max vol diff = {}", maxdiff);
    assert!(maxdiff < 1e-6, "max diff {} too large", maxdiff);
}
