use serde::{Deserialize, Serialize};

const ANN_FACTOR: f64 = 24.0 * 365.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeacherAction {
    Long,
    Short,
    Hold,
    NoAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherConfig {
    pub temperature: f64,
    pub trend_weight: f64,
    pub regime_weight: f64,
    pub cost_scale: f64,
    pub entry_threshold: f64,
    pub vol_max_cutoff: f64,
    pub holding_period: usize,
    pub fee_bps: f64,
    pub slippage_bps: f64,
    pub horizon: usize,
}

impl Default for TeacherConfig {
    fn default() -> Self {
        Self {
            temperature: 2.0,
            trend_weight: 0.7,
            regime_weight: 0.3,
            cost_scale: 0.5,
            entry_threshold: 0.40,
            vol_max_cutoff: 0.20,
            holding_period: 24,
            fee_bps: 5.0,
            slippage_bps: 1.0,
            horizon: 24,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeacherDecision {
    pub action: TeacherAction,
    pub p_long: f64,
    pub p_short: f64,
    pub p_no_action: f64,
    pub trend_z: f64,
    pub regime_z: f64,
    pub edge: f64,
    pub net_edge: f64,
    pub garch_vol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub bars: usize,
    pub total_return: f64,
    pub annualized_sharpe: f64,
    pub max_drawdown: f64,
    pub bar_hit_rate: f64,
    pub active_bars: usize,
    pub position_changes: usize,
    pub total_cost: f64,
}

pub struct TeacherPolicy {
    config: TeacherConfig,
}

impl TeacherPolicy {
    pub fn new(config: TeacherConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TeacherConfig {
        &self.config
    }

    /// Compute teacher decision from precomputed signals:
    /// - `mom_24`: 24-period pct price return
    /// - `mom_72`: 72-period pct price return
    /// - `garch_vol`: conditional volatility (std scale, e.g. 0.02)
    /// - `posteriors`: Markov regime probabilities ([p_down, p_up] or [p_down, p_range, p_up])
    pub fn decide_from_signals(
        &self,
        mom_24: f64,
        mom_72: f64,
        garch_vol: f64,
        posteriors: &[f64],
    ) -> TeacherDecision {
        let vol = garch_vol.max(1e-9);

        let trend_z = 0.7 * mom_24 / vol + 0.3 * mom_72 / (vol * (3.0f64).sqrt());

        let p_down = if posteriors.is_empty() {
            0.0
        } else {
            posteriors[0]
        };
        let p_up = match posteriors.len() {
            0 => 0.0,
            1 => posteriors[0],
            2 => posteriors[1],
            _ => *posteriors.last().unwrap(),
        };

        let regime_z = p_up - p_down;
        let edge = self.config.trend_weight * trend_z + self.config.regime_weight * regime_z;

        let cost_per_side = (self.config.fee_bps + self.config.slippage_bps) / 10_000.0;
        let cost_round_trip = 2.0 * cost_per_side;
        let cost_z = cost_round_trip / vol;
        let net_edge = edge - self.config.cost_scale * cost_z;

        // Softmax over [T * net_edge, -T * net_edge, 0.0]
        let l0 = self.config.temperature * net_edge;
        let l1 = -self.config.temperature * net_edge;
        let l2 = 0.0;

        let max_l = l0.max(l1).max(l2);
        let e0 = (l0 - max_l).exp();
        let e1 = (l1 - max_l).exp();
        let e2 = (l2 - max_l).exp();
        let sum_e = e0 + e1 + e2;

        let p_long = e0 / sum_e;
        let p_short = e1 / sum_e;
        let p_no_action = e2 / sum_e;

        let action = if vol > self.config.vol_max_cutoff {
            TeacherAction::NoAction
        } else if p_long > p_short + 0.05 && p_long > self.config.entry_threshold {
            TeacherAction::Long
        } else if p_short > p_long + 0.05 && p_short > self.config.entry_threshold {
            TeacherAction::Short
        } else {
            TeacherAction::NoAction
        };

        TeacherDecision {
            action,
            p_long,
            p_short,
            p_no_action,
            trend_z,
            regime_z,
            edge,
            net_edge,
            garch_vol,
        }
    }
}

/// Holding period simulator matching Python `apply_holding_period`.
pub fn apply_holding_period(signals: &[i32], horizon: usize) -> Vec<f64> {
    let n = signals.len();
    let mut positions = vec![0.0f64; n];
    let mut remaining = 0;

    for (i, &desired) in signals.iter().enumerate() {
        if remaining > 0 {
            positions[i] = positions[i - 1];
            remaining -= 1;
        } else if desired != 0 {
            positions[i] = desired as f64;
            remaining = horizon;
        }
    }

    positions
}

/// Calculate performance metrics net of costs.
pub fn calculate_metrics(
    log_returns: &[f64],
    positions: &[f64],
    turnover: &[f64],
    cost_per_side: f64,
    initial_position: f64,
) -> BacktestMetrics {
    let n = log_returns.len().min(positions.len()).min(turnover.len());
    if n == 0 {
        return BacktestMetrics {
            bars: 0,
            total_return: 0.0,
            annualized_sharpe: 0.0,
            max_drawdown: 0.0,
            bar_hit_rate: 0.0,
            active_bars: 0,
            position_changes: 0,
            total_cost: 0.0,
        };
    }

    let mut strategy_returns = vec![0.0f64; n];
    let mut active_count = 0;
    let mut active_positive = 0;
    let mut position_changes = 0;
    let mut total_cost = 0.0;

    for t in 0..n {
        let pos_prev = if t == 0 {
            initial_position
        } else {
            positions[t - 1]
        };
        let cost = turnover[t] * cost_per_side;
        total_cost += cost;

        if turnover[t] > 0.0 {
            position_changes += 1;
        }

        let ret = pos_prev * log_returns[t] - cost;
        strategy_returns[t] = ret;

        if pos_prev != 0.0 {
            active_count += 1;
            if ret > 0.0 {
                active_positive += 1;
            }
        }
    }

    // Cumulative equity and drawdown
    let mut equity = vec![1.0f64; n];
    let mut cum = 0.0;
    let mut peak = 1.0f64;
    let mut max_dd = 0.0f64;

    for (t, &r) in strategy_returns.iter().enumerate() {
        cum += r;
        let eq = cum.exp();
        equity[t] = eq;
        if eq > peak {
            peak = eq;
        }
        let dd = eq / peak - 1.0;
        if dd < max_dd {
            max_dd = dd;
        }
    }

    let total_return = (cum).exp() - 1.0;

    // Mean and std
    let mean: f64 = strategy_returns.iter().sum::<f64>() / n as f64;
    let var: f64 = strategy_returns
        .iter()
        .map(|r| (r - mean) * (r - mean))
        .sum::<f64>()
        / n as f64;
    let std = var.sqrt();

    let annualized_sharpe = if std > 1e-12 {
        mean / std * ANN_FACTOR.sqrt()
    } else {
        0.0
    };

    let bar_hit_rate = if active_count > 0 {
        active_positive as f64 / active_count as f64
    } else {
        0.0
    };

    BacktestMetrics {
        bars: n,
        total_return,
        annualized_sharpe,
        max_drawdown: max_dd,
        bar_hit_rate,
        active_bars: active_count,
        position_changes,
        total_cost,
    }
}

/// Always-HOLD baseline: fully invested (position = 1.0).
pub fn always_hold_metrics(log_returns: &[f64], cost_per_side: f64) -> BacktestMetrics {
    let n = log_returns.len();
    let positions = vec![1.0f64; n];
    let mut turnover = vec![0.0f64; n];
    if n > 0 {
        turnover[0] = 1.0; // Initial entry fee
    }
    calculate_metrics(log_returns, &positions, &turnover, cost_per_side, 0.0)
}

/// Random trader baseline: pseudo-random discrete actions {-1, 0, 1}.
pub fn random_baseline_metrics(
    log_returns: &[f64],
    seed: u64,
    holding_period: usize,
    cost_per_side: f64,
) -> BacktestMetrics {
    let n = log_returns.len();
    let mut state = seed.max(1);
    let mut signals = vec![0i32; n];

    for s in signals.iter_mut() {
        // Simple 64-bit LCG
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let r = (state >> 62) as i32; // 0, 1, 2, 3
        *s = match r {
            0 => 1,
            1 => -1,
            _ => 0,
        };
    }

    let positions = apply_holding_period(&signals, holding_period);
    let mut turnover = vec![0.0f64; n];
    for t in 0..n {
        let prev = if t == 0 { 0.0 } else { positions[t - 1] };
        turnover[t] = (positions[t] - prev).abs();
    }

    calculate_metrics(log_returns, &positions, &turnover, cost_per_side, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softmax_probabilities_sum_to_one_and_symmetric() {
        let policy = TeacherPolicy::new(TeacherConfig {
            cost_scale: 0.0,
            ..Default::default()
        });
        // positive edge
        let dec = policy.decide_from_signals(0.05, 0.08, 0.02, &[0.1, 0.2, 0.7]);
        assert!((dec.p_long + dec.p_short + dec.p_no_action - 1.0).abs() < 1e-6);
        assert!(dec.p_long > dec.p_short);

        // neutral
        let dec_neutral = policy.decide_from_signals(0.0, 0.0, 0.02, &[0.2, 0.6, 0.2]);
        assert!((dec_neutral.p_long - dec_neutral.p_short).abs() < 1e-6);
    }

    #[test]
    fn teacher_generates_long_on_strong_bullish_signals() {
        let policy = TeacherPolicy::new(TeacherConfig::default());
        // Strong positive momentum + strong up regime
        let dec = policy.decide_from_signals(0.08, 0.15, 0.015, &[0.05, 0.15, 0.80]);
        assert_eq!(dec.action, TeacherAction::Long);
        assert!(dec.p_long > 0.40);
    }

    #[test]
    fn teacher_generates_short_on_strong_bearish_signals() {
        let policy = TeacherPolicy::new(TeacherConfig::default());
        // Strong negative momentum + strong down regime
        let dec = policy.decide_from_signals(-0.08, -0.15, 0.015, &[0.80, 0.15, 0.05]);
        assert_eq!(dec.action, TeacherAction::Short);
        assert!(dec.p_short > 0.40);
    }

    #[test]
    fn teacher_abstains_when_costs_dominate_or_vol_too_high() {
        let policy = TeacherPolicy::new(TeacherConfig {
            vol_max_cutoff: 0.05,
            ..Default::default()
        });
        // Volatility 0.10 > vol_max_cutoff 0.05
        let dec = policy.decide_from_signals(0.08, 0.15, 0.10, &[0.05, 0.15, 0.80]);
        assert_eq!(dec.action, TeacherAction::NoAction);
    }

    #[test]
    fn holding_period_maintains_position_across_bars() {
        let signals = vec![1, 0, 0, 0, 0];
        let positions = apply_holding_period(&signals, 3);
        assert_eq!(positions, vec![1.0, 1.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn backtest_metrics_calculation() {
        // 5 bars with alternating returns
        let returns = vec![0.01, -0.005, 0.02, 0.015, -0.01];
        let positions = vec![1.0, 1.0, 1.0, 1.0, 0.0];
        let turnover = vec![1.0, 0.0, 0.0, 0.0, 1.0];
        let cost_per_side = 0.0006;

        let m = calculate_metrics(&returns, &positions, &turnover, cost_per_side, 0.0);
        assert_eq!(m.bars, 5);
        assert!(m.total_cost > 0.0);
        assert!(m.annualized_sharpe.is_finite());
    }

    #[test]
    fn baselines_generate_metrics() {
        let returns = vec![0.005; 100];
        let hold_m = always_hold_metrics(&returns, 0.0006);
        assert_eq!(hold_m.bars, 100);
        assert_eq!(hold_m.active_bars, 99); // bar 0 enters, active from bar 1

        let rand_m = random_baseline_metrics(&returns, 42, 24, 0.0006);
        assert_eq!(rand_m.bars, 100);
    }
}
