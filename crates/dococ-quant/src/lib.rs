pub mod guard;
/// Quant primitives for the causal teacher (Phase 2).
///
/// GARCH(1,1) variance recursion and the Markov (Hamilton) forward filter,
/// ported from the Python teacher (`scripts/teacher_*.py`) with parity tests.
pub mod regime;
pub mod teacher;
pub use guard::{StaleDataGuard, StaleGuardError};
pub use regime::{RegimeHysteresis, RegimeState};
pub use teacher::{
    BacktestMetrics, TeacherAction, TeacherConfig, TeacherDecision, TeacherPolicy,
    always_hold_metrics, apply_holding_period, calculate_metrics, random_baseline_metrics,
};

/// GARCH(1,1) parameters.
#[derive(Debug, Clone, Copy)]
pub struct GarchParams {
    pub omega: f64,
    pub alpha: f64,
    pub beta: f64,
}

/// Backcast: weighted average of the first up to 75 squared residuals.
fn garch_backcast(residuals: &[f64]) -> f64 {
    let n = residuals.len();
    let tau = n.min(75);
    if tau == 0 {
        return 0.0;
    }
    let mut w = vec![0.0f64; tau];
    for i in 0..tau {
        w[i] = 0.94f64.powi(i as i32);
    }
    let sw: f64 = w.iter().sum();
    let mut bc = 0.0;
    for i in 0..tau {
        bc += (w[i] / sw) * residuals[i] * residuals[i];
    }
    bc
}

/// GARCH(1,1) conditional variance series matching arch's recursion.
pub fn garch_conditional_variance(residuals: &[f64], params: &GarchParams) -> Vec<f64> {
    let n = residuals.len();
    if n == 0 {
        return vec![];
    }
    let bc = garch_backcast(residuals);
    // EWMA (lambda=0.94) of squared residuals -> var_bound base
    let mut vb = vec![0.0f64; n];
    let (a, b) = (1.0 - 0.94, 0.94);
    for t in 0..n {
        let mut s = 0.0;
        if t == 0 {
            s += a * bc;
        } else {
            s += a * residuals[t - 1] * residuals[t - 1];
        }
        if t == 0 {
            s += b * bc;
        } else {
            s += b * vb[t - 1];
        }
        vb[t] = s;
    }
    let v_var: f64 = residuals.iter().map(|r| r * r).sum::<f64>() / n as f64;
    let max_sq = residuals.iter().map(|r| r * r).fold(0.0f64, f64::max);
    let min_upper = 1.0 + max_sq;
    let lower_bound = v_var / 1e8;
    let upper_bound = 1e7 * (1.0 + max_sq);
    let mut lo = vec![0.0f64; n];
    let mut hi = vec![0.0f64; n];
    for t in 0..n {
        let mut l = vb[t] / 1e6;
        let mut h = vb[t] * 1e6;
        if l < lower_bound {
            l = lower_bound;
        }
        if h < min_upper {
            h = min_upper;
        }
        if h > upper_bound {
            h = upper_bound;
        }
        lo[t] = l;
        hi[t] = h;
    }
    let mut sigma2 = vec![0.0f64; n];
    for t in 0..n {
        let mut s = params.omega;
        if t == 0 {
            s += params.alpha * bc;
        } else {
            s += params.alpha * residuals[t - 1] * residuals[t - 1];
        }
        if t == 0 {
            s += params.beta * bc;
        } else {
            s += params.beta * sigma2[t - 1];
        }
        if s < lo[t] {
            s = lo[t];
        } else if s > hi[t] {
            if s.is_finite() {
                s = hi[t] + (s / hi[t]).ln();
            } else {
                s = hi[t] + 1000.0;
            }
        }
        sigma2[t] = s;
    }
    sigma2
}

/// Convert variance series to volatility (std).
pub fn garch_volatility(residuals: &[f64], params: &GarchParams) -> Vec<f64> {
    garch_conditional_variance(residuals, params)
        .iter()
        .map(|v| v.sqrt())
        .collect()
}

// ---------- Markov (Hamilton) forward filter ----------

/// A Gaussian HMM with diagonal covariances, for the 2-3 state regime model.
#[derive(Debug, Clone)]
pub struct GaussianHmm {
    pub n_states: usize,
    /// transition matrix A[i][j] = P(s_t=j | s_{t-1}=i)
    pub transition: Vec<Vec<f64>>,
    pub means: Vec<Vec<f64>>,     // [state][feature]
    pub variances: Vec<Vec<f64>>, // [state][feature] (diag)
    pub initial: Vec<f64>,        // initial distribution
}

impl GaussianHmm {
    /// Filtered posterior P(s_t | y_{1:t}) for each t. Returns (n, n_states).
    pub fn filter(&self, obs: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let n = obs.len();
        let k = self.n_states;
        let mut gamma = vec![vec![0.0; k]; n];
        if n == 0 {
            return gamma;
        }
        // t=0
        let pred = self.initial.clone();
        // update
        let mut lik = vec![0.0; k];
        for st in 0..k {
            lik[st] = self.emission(&obs[0], st) * pred[st];
        }
        let s: f64 = lik.iter().sum();
        if s > 0.0 {
            for st in 0..k {
                gamma[0][st] = lik[st] / s;
            }
        } else {
            for st in 0..k {
                gamma[0][st] = 1.0 / k as f64;
            }
        }
        for t in 1..n {
            // predict
            let mut p = vec![0.0; k];
            for j in 0..k {
                for i in 0..k {
                    p[j] += gamma[t - 1][i] * self.transition[i][j];
                }
            }
            // update
            let mut l = vec![0.0; k];
            for st in 0..k {
                l[st] = self.emission(&obs[t], st) * p[st];
            }
            let s: f64 = l.iter().sum();
            if s > 0.0 {
                for st in 0..k {
                    gamma[t][st] = l[st] / s;
                }
            } else {
                for st in 0..k {
                    gamma[t][st] = 1.0 / k as f64;
                }
            }
        }
        gamma
    }

    fn emission(&self, x: &[f64], st: usize) -> f64 {
        let d = x.len();
        let mut logp = 0.0;
        for f in 0..d {
            let mu = self.means[st][f];
            let var = self.variances[st][f].max(1e-12);
            let diff = x[f] - mu;
            logp += -0.5 * (diff * diff / var + var.ln() + (2.0 * std::f64::consts::PI).ln());
        }
        logp.exp()
    }
}
