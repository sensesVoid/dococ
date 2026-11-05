use std::time::Instant;

use dococ_laya::LayaRuntime;

fn state(repetitions: usize) -> String {
    const BLOCK: &str = "\
Account: paper. Equity 100000 USD. Position: flat.
Market: BTC/USDT perpetual. Last price 64000. 4h trend: up.
GARCH conditional volatility: 0.012. Markov regime: trending.
News: spot ETF inflows positive; funding neutral.
";
    BLOCK.repeat(repetitions)
}

fn main() {
    let model_dir =
        std::env::var("DOCOC_LAYA_MODEL").expect("set DOCOC_LAYA_MODEL to the ONNX model directory");
    let repetitions = std::env::var("DOCOC_LAYA_BENCH_REPS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    let iterations = std::env::var("DOCOC_LAYA_BENCH_N")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);

    let load_started = Instant::now();
    let runtime = LayaRuntime::load(&model_dir).expect("load Laya model");
    let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;

    let state = state(repetitions);
    // Warm the tokenizer and the ONNX session before recording timings.
    runtime.decide(&state).expect("warm-up decision");
    let _ = runtime.decide(&state).expect("second warm-up decision");

    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let decision = runtime.decide(&state).expect("benchmark decision");
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        timings.push(elapsed_ms);
        println!(
            "action={:?} confidence={:.4} edge={:.4} conviction={:.4} ms={:.1}",
            decision.action,
            decision.action_confidence,
            decision.edge_probability,
            decision.conviction,
            elapsed_ms
        );
    }

    timings.sort_by(|a, b| a.partial_cmp(b).expect("finite timings"));
    let mean = timings.iter().sum::<f64>() / timings.len() as f64;
    let p50 = timings[timings.len() / 2];
    let p95 = timings[(timings.len() as f64 * 0.95).ceil() as usize - 1];

    println!(
        "load_ms={:.1} state_chars={} reps={} n={} mean_ms={:.1} p50_ms={:.1} p95_ms={:.1}",
        load_ms,
        state.chars().count(),
        repetitions,
        iterations,
        mean,
        p50,
        p95
    );
}
