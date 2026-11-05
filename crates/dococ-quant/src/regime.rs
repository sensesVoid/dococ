#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegimeState {
    Down,
    Range,
    Up,
}

#[derive(Debug, Clone)]
pub struct RegimeHysteresis {
    current: RegimeState,
    candidate: Option<RegimeState>,
    candidate_count: usize,
    min_margin: f64,
    min_persistence: usize,
}

impl RegimeHysteresis {
    pub fn new(initial: RegimeState, min_margin: f64, min_persistence: usize) -> Self {
        Self {
            current: initial,
            candidate: None,
            candidate_count: 0,
            min_margin,
            min_persistence: min_persistence.max(1),
        }
    }

    pub fn current(&self) -> RegimeState {
        self.current
    }

    /// Update hysteresis filter with posteriors.
    /// Supports 2 states [p_down, p_up] or 3 states [p_down, p_range, p_up].
    pub fn update(&mut self, posteriors: &[f64]) -> RegimeState {
        if posteriors.is_empty() {
            return self.current;
        }

        let (p_down, p_range, p_up) = match posteriors.len() {
            2 => (posteriors[0], 0.0, posteriors[1]),
            3 => (posteriors[0], posteriors[1], posteriors[2]),
            _ => (posteriors[0], 0.0, *posteriors.last().unwrap()),
        };

        let current_prob = match self.current {
            RegimeState::Down => p_down,
            RegimeState::Range => p_range,
            RegimeState::Up => p_up,
        };

        // Determine max probability state
        let mut best_state = RegimeState::Down;
        let mut best_prob = p_down;

        if p_range > best_prob {
            best_state = RegimeState::Range;
            best_prob = p_range;
        }
        if p_up > best_prob {
            best_state = RegimeState::Up;
            best_prob = p_up;
        }

        if best_state == self.current {
            self.candidate = None;
            self.candidate_count = 0;
            return self.current;
        }

        if best_prob >= current_prob + self.min_margin {
            if self.candidate == Some(best_state) {
                self.candidate_count += 1;
            } else {
                self.candidate = Some(best_state);
                self.candidate_count = 1;
            }

            if self.candidate_count >= self.min_persistence {
                self.current = best_state;
                self.candidate = None;
                self.candidate_count = 0;
            }
        } else {
            self.candidate = None;
            self.candidate_count = 0;
        }

        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_stays_in_initial_state_when_margin_not_met() {
        let mut h = RegimeHysteresis::new(RegimeState::Range, 0.15, 2);
        // Candidate Up is 0.40, current Range is 0.35. Margin is 0.05 < 0.15.
        let state = h.update(&[0.25, 0.35, 0.40]);
        assert_eq!(state, RegimeState::Range);
        assert_eq!(h.current(), RegimeState::Range);
    }

    #[test]
    fn hysteresis_transitions_after_persistence_bars() {
        let mut h = RegimeHysteresis::new(RegimeState::Range, 0.15, 2);
        // Candidate Up is 0.60, current Range is 0.30. Margin is 0.30 >= 0.15.
        // Bar 1: meets margin, but persistence is 1 < 2 -> should still report Range
        let s1 = h.update(&[0.10, 0.30, 0.60]);
        assert_eq!(s1, RegimeState::Range);

        // Bar 2: meets margin again -> persistence reaches 2 -> transitions to Up
        let s2 = h.update(&[0.10, 0.30, 0.60]);
        assert_eq!(s2, RegimeState::Up);
        assert_eq!(h.current(), RegimeState::Up);
    }

    #[test]
    fn hysteresis_resets_persistence_counter_if_interrupted() {
        let mut h = RegimeHysteresis::new(RegimeState::Range, 0.15, 3);
        // Bar 1 & 2 meet margin for Up
        assert_eq!(h.update(&[0.10, 0.30, 0.60]), RegimeState::Range);
        assert_eq!(h.update(&[0.10, 0.30, 0.60]), RegimeState::Range);

        // Bar 3 dips below margin (Up=0.40, Range=0.35)
        assert_eq!(h.update(&[0.25, 0.35, 0.40]), RegimeState::Range);

        // Bar 4 meets margin again, but counter was reset, so needs 3 more bars
        assert_eq!(h.update(&[0.10, 0.30, 0.60]), RegimeState::Range);
        assert_eq!(h.update(&[0.10, 0.30, 0.60]), RegimeState::Range);
        assert_eq!(h.update(&[0.10, 0.30, 0.60]), RegimeState::Up);
    }
}
