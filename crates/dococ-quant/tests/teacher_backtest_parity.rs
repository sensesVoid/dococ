use dococ_quant::{apply_holding_period, calculate_metrics, always_hold_metrics, random_baseline_metrics};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[test]
fn teacher_backtest_matches_python_metrics() {
    let file = File::open("../../data/teacher_labels_6k.csv")
        .or_else(|_| File::open("data/teacher_labels_6k.csv"))
        .expect("could not open teacher_labels_6k.csv");
    let reader = BufReader::new(file);

    let mut closes = Vec::new();
    let mut signals = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line.expect("read line");
        if i == 0 || line.trim().is_empty() {
            continue; // Header
        }
        let parts: Vec<&str> = line.split(',').collect();
        // open_time,close,garch_vol,p_down,p_range,p_up,p_long,p_short,p_no_action,signal,fwd_ret_24
        let close: f64 = parts[1].parse().expect("parse close");
        let sig: i32 = parts[9].parse().expect("parse signal");
        closes.push(close);
        signals.push(sig);
    }

    assert_eq!(closes.len(), 2244);

    // Compute log returns (diff of ln(close))
    let mut log_returns = Vec::with_capacity(closes.len() - 1);
    for i in 1..closes.len() {
        log_returns.push((closes[i] / closes[i - 1]).ln());
    }

    // Signals align with the bars; holding period = 24
    let all_positions = apply_holding_period(&signals, 24);
    // Align positions with log_returns (indices 1..2244)
    let positions = &all_positions[1..];
    assert_eq!(log_returns.len(), positions.len());

    // Turnover
    let mut turnover = vec![0.0f64; positions.len()];
    for t in 0..positions.len() {
        let prev = if t == 0 { all_positions[0] } else { positions[t - 1] };
        turnover[t] = (positions[t] - prev).abs();
    }

    let cost_per_side = 0.0006; // 6 bps = 5 bps fee + 1 bps slippage
    let m = calculate_metrics(&log_returns, positions, &turnover, cost_per_side, all_positions[0]);

    println!("Rust Teacher metrics: {:?}", m);

    // Verify key metrics against Python calculation (clean rows 1..2243)
    assert_eq!(m.bars, 2243);
    assert_eq!(m.position_changes, 41);
    assert_eq!(m.active_bars, 2237);
    assert!((m.total_cost - 0.0456).abs() < 1e-4);
    assert!((m.total_return - 0.25238).abs() < 1e-4);
    assert!((m.annualized_sharpe - 1.68197).abs() < 1e-3);
    assert!((m.max_drawdown - (-0.24114)).abs() < 1e-4);
    assert!((m.bar_hit_rate - 0.50156).abs() < 1e-3);

    // Baselines comparison
    let hold_m = always_hold_metrics(&log_returns, cost_per_side);
    println!("Always-HOLD metrics: {:?}", hold_m);
    assert_eq!(hold_m.bars, 2243);

    let rand_m = random_baseline_metrics(&log_returns, 42, 24, cost_per_side);
    println!("Random baseline metrics: {:?}", rand_m);
    assert_eq!(rand_m.bars, 2243);
}
