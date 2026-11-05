use dococ_quant::GaussianHmm;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct Fixture {
    n_states: usize,
    transition: Vec<Vec<f64>>,
    means: Vec<Vec<f64>>,
    variances: Vec<Vec<f64>>,
    initial: Vec<f64>,
    obs: Vec<f64>,
    expected: Vec<Vec<f64>>,
}

#[test]
fn markov_forward_filter_matches_python() {
    let raw = fs::read_to_string("tests/markov_fixture.json").unwrap();
    let f: Fixture = serde_json::from_str(&raw).unwrap();
    let model = GaussianHmm {
        n_states: f.n_states,
        transition: f.transition,
        means: f.means,
        variances: f.variances,
        initial: f.initial,
    };
    let obs: Vec<Vec<f64>> = f.obs.iter().map(|x| vec![*x]).collect();
    let gamma = model.filter(&obs);
    assert_eq!(gamma.len(), f.expected.len());
    let mut maxdiff = 0.0f64;
    for (g, e) in gamma.iter().zip(f.expected.iter()) {
        for (a, b) in g.iter().zip(e.iter()) {
            let d = (a - b).abs();
            if d > maxdiff {
                maxdiff = d;
            }
        }
    }
    println!("max markov diff = {}", maxdiff);
    assert!(maxdiff < 1e-9, "max diff {} too large", maxdiff);
}
