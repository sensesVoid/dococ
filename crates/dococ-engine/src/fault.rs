use crate::{Engine, LayaStrategy, Replay, Scheduler, Strategy};
use dococ_core::{Account, AccountId, Bar, Symbol};
use dococ_laya::{LayaRuntime, OrderAction};
use dococ_risk::RiskConfig;
use dococ_venue::SimVenue;

/// Fault injection scenarios for testing system resilience.
#[derive(Debug, Clone)]
pub enum Fault {
    /// Drop a bar (simulate missed market data).
    DropBar { index: usize },
    /// Duplicate a bar (simulate retransmission).
    DuplicateBar { index: usize },
    /// Reorder two bars.
    ReorderBars { i: usize, j: usize },
    /// Inject a partial fill for an order.
    PartialFill { order_qty: f64, fill_qty: f64 },
    /// Simulate a disconnect (skip N bars).
    Disconnect { skip: usize },
}

/// Apply faults to a sequence of bars.
pub fn inject_faults(bars: &[Bar], faults: &[Fault]) -> Vec<Bar> {
    let mut result: Vec<Bar> = bars.to_vec();

    for fault in faults {
        match fault {
            Fault::DropBar { index } => {
                if *index < result.len() {
                    result.remove(*index);
                }
            }
            Fault::DuplicateBar { index } => {
                if *index < result.len() {
                    let dup = result[*index].clone();
                    result.insert(*index + 1, dup);
                }
            }
            Fault::ReorderBars { i, j } => {
                if *i < result.len() && *j < result.len() {
                    result.swap(*i, *j);
                }
            }
            Fault::Disconnect { skip } => {
                // Remove `skip` bars starting from the middle.
                let start = result.len() / 2;
                let end = (start + skip).min(result.len());
                result.drain(start..end);
            }
            Fault::PartialFill { .. } => {
                // Partial fills are handled at the OMS level, not bar level.
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bars(n: usize) -> Vec<Bar> {
        (0..n)
            .map(|i| Bar {
                timestamp: 1_700_000_000 + i as i64 * 3600,
                open: 64_000.0,
                high: 64_500.0,
                low: 63_800.0,
                close: 64_200.0 + (i as f64 * 10.0),
                volume: 1_000.0,
            })
            .collect()
    }

    #[test]
    fn drop_bar_reduces_count() {
        let bars = make_bars(5);
        let faults = vec![Fault::DropBar { index: 2 }];
        let result = inject_faults(&bars, &faults);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn duplicate_bar_increases_count() {
        let bars = make_bars(5);
        let faults = vec![Fault::DuplicateBar { index: 1 }];
        let result = inject_faults(&bars, &faults);
        assert_eq!(result.len(), 6);
    }

    #[test]
    fn reorder_swaps_bars() {
        let bars = make_bars(5);
        let faults = vec![Fault::ReorderBars { i: 0, j: 4 }];
        let result = inject_faults(&bars, &faults);
        assert_eq!(result[0].timestamp, bars[4].timestamp);
        assert_eq!(result[4].timestamp, bars[0].timestamp);
    }

    #[test]
    fn disconnect_removes_bars() {
        let bars = make_bars(10);
        let faults = vec![Fault::Disconnect { skip: 3 }];
        let result = inject_faults(&bars, &faults);
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn no_faults_preserves_bars() {
        let bars = make_bars(5);
        let result = inject_faults(&bars, &[]);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn replay_survives_dropped_bar() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);
        let mut replay = Replay::new(engine);

        let bars = make_bars(5);
        let faults = vec![Fault::DropBar { index: 2 }];
        let faulty_bars = inject_faults(&bars, &faults);

        let symbol = Symbol::new("BTCUSDT");
        for bar in faulty_bars {
            let _ = replay.feed_bar(symbol.clone(), bar);
        }
        // Should have 4 decisions (one bar dropped).
        assert_eq!(replay.decisions().len(), 4);
    }

    #[test]
    fn replay_survives_duplicated_bar() {
        let model_dir = match std::env::var("DOCOC_LAYA_MODEL") {
            Ok(d) => d,
            Err(_) => return,
        };
        let laya = LayaRuntime::load(&model_dir).expect("load");
        let strategy = LayaStrategy::new(laya);
        let venue = SimVenue::new(100_000.0, 5.0);
        let account = Account::new(AccountId(1), 100_000.0);
        let scheduler = Scheduler::new(900, 1);
        let engine = Engine::new(strategy, RiskConfig::default(), venue, account, scheduler);
        let mut replay = Replay::new(engine);

        let bars = make_bars(3);
        let faults = vec![Fault::DuplicateBar { index: 1 }];
        let faulty_bars = inject_faults(&bars, &faults);

        let symbol = Symbol::new("BTCUSDT");
        for bar in faulty_bars {
            let _ = replay.feed_bar(symbol.clone(), bar);
        }
        // Should have 4 decisions (one bar duplicated).
        assert_eq!(replay.decisions().len(), 4);
    }
}
