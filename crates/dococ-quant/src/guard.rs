use dococ_core::Bar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleGuardError {
    EmptyBars,
    StaleTimestamp { last_ts: i64, now: i64, diff_secs: i64 },
    FutureTimestamp { last_ts: i64, now: i64 },
    GapDetected { prev_ts: i64, next_ts: i64, gap_secs: i64 },
    InvalidValues(String),
}

#[derive(Debug, Clone)]
pub struct StaleDataGuard {
    max_stale_secs: i64,
    expected_interval_secs: i64,
}

impl StaleDataGuard {
    pub fn new(max_stale_secs: i64, expected_interval_secs: i64) -> Self {
        Self {
            max_stale_secs,
            expected_interval_secs,
        }
    }

    pub fn max_stale_secs(&self) -> i64 {
        self.max_stale_secs
    }

    pub fn expected_interval_secs(&self) -> i64 {
        self.expected_interval_secs
    }

    pub fn check_bars(&self, bars: &[Bar], now: i64) -> Result<(), StaleGuardError> {
        if bars.is_empty() {
            return Err(StaleGuardError::EmptyBars);
        }

        let last = &bars[bars.len() - 1];
        if last.timestamp > now {
            return Err(StaleGuardError::FutureTimestamp {
                last_ts: last.timestamp,
                now,
            });
        }

        let diff_secs = now - last.timestamp;
        if diff_secs > self.max_stale_secs {
            return Err(StaleGuardError::StaleTimestamp {
                last_ts: last.timestamp,
                now,
                diff_secs,
            });
        }

        let max_gap = (self.expected_interval_secs as f64 * 1.5) as i64;

        for (i, bar) in bars.iter().enumerate() {
            if !bar.open.is_finite() || !bar.high.is_finite() || !bar.low.is_finite() || !bar.close.is_finite() || !bar.volume.is_finite() {
                return Err(StaleGuardError::InvalidValues(format!("non-finite values at index {}", i)));
            }
            if bar.open <= 0.0 || bar.high <= 0.0 || bar.low <= 0.0 || bar.close <= 0.0 || bar.volume < 0.0 {
                return Err(StaleGuardError::InvalidValues(format!("non-positive prices or negative volume at index {}", i)));
            }
            if bar.high < bar.low {
                return Err(StaleGuardError::InvalidValues(format!("high < low at index {}", i)));
            }

            if i > 0 {
                let prev = &bars[i - 1];
                let gap = bar.timestamp - prev.timestamp;
                if gap > max_gap {
                    return Err(StaleGuardError::GapDetected {
                        prev_ts: prev.timestamp,
                        next_ts: bar.timestamp,
                        gap_secs: gap,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn check_features(&self, features: &[f64]) -> Result<(), StaleGuardError> {
        for (i, &f) in features.iter().enumerate() {
            if !f.is_finite() {
                return Err(StaleGuardError::InvalidValues(format!("feature at index {} is non-finite: {}", i, f)));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bar(ts: i64, price: f64) -> Bar {
        Bar {
            timestamp: ts,
            open: price,
            high: price + 10.0,
            low: price - 10.0,
            close: price,
            volume: 100.0,
        }
    }

    #[test]
    fn guard_accepts_clean_recent_bars() {
        let guard = StaleDataGuard::new(7200, 3600); // 2 hours max stale, 1 hour interval
        let bars = vec![
            sample_bar(10000, 50000.0),
            sample_bar(13600, 50100.0),
            sample_bar(17200, 50200.0),
        ];
        let now = 18000; // 800s after last bar (< 7200s)
        assert!(guard.check_bars(&bars, now).is_ok());
    }

    #[test]
    fn guard_rejects_empty_bars() {
        let guard = StaleDataGuard::new(7200, 3600);
        let err = guard.check_bars(&[], 10000).unwrap_err();
        assert_eq!(err, StaleGuardError::EmptyBars);
    }

    #[test]
    fn guard_rejects_stale_data() {
        let guard = StaleDataGuard::new(7200, 3600);
        let bars = vec![sample_bar(10000, 50000.0)];
        let now = 20000; // 10000s after last bar (> 7200s)
        let err = guard.check_bars(&bars, now).unwrap_err();
        match err {
            StaleGuardError::StaleTimestamp { diff_secs, .. } => assert_eq!(diff_secs, 10000),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn guard_rejects_future_timestamp() {
        let guard = StaleDataGuard::new(7200, 3600);
        let bars = vec![sample_bar(20000, 50000.0)];
        let now = 10000; // bar timestamp is ahead of now by 10000s
        let err = guard.check_bars(&bars, now).unwrap_err();
        assert_eq!(err, StaleGuardError::FutureTimestamp { last_ts: 20000, now: 10000 });
    }

    #[test]
    fn guard_rejects_gap_in_bars() {
        let guard = StaleDataGuard::new(7200, 3600); // 1h expected interval
        let bars = vec![
            sample_bar(10000, 50000.0),
            sample_bar(20000, 50100.0), // Gap of 10000s > 1.5 * 3600s
        ];
        let now = 21000;
        let err = guard.check_bars(&bars, now).unwrap_err();
        match err {
            StaleGuardError::GapDetected { gap_secs, .. } => assert_eq!(gap_secs, 10000),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn guard_rejects_nan_or_non_positive_price() {
        let guard = StaleDataGuard::new(7200, 3600);
        let mut bar = sample_bar(10000, 50000.0);
        bar.close = f64::NAN;
        let err = guard.check_bars(&[bar], 11000).unwrap_err();
        match err {
            StaleGuardError::InvalidValues(_) => (),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn guard_checks_features() {
        let guard = StaleDataGuard::new(7200, 3600);
        assert!(guard.check_features(&[0.1, 0.2, 0.3]).is_ok());
        assert!(guard.check_features(&[0.1, f64::NAN, 0.3]).is_err());
    }
}
