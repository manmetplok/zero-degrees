use chrono::{Duration, NaiveDate};

pub const SHIELD_STREAK_THRESHOLD: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreakState {
    pub current_streak: u32,
    pub best_streak: u32,
    pub has_shield: bool,
    pub last_settled: Option<NaiveDate>,
}

impl StreakState {
    pub fn new() -> Self {
        Self {
            current_streak: 0,
            best_streak: 0,
            has_shield: false,
            last_settled: None,
        }
    }

    pub fn settle_missed_days(&mut self, today: NaiveDate) {
        let last = match self.last_settled {
            Some(last) => last,
            None => {
                self.last_settled = Some(today);
                return;
            }
        };
        if last >= today {
            return;
        }
        let missed_days = (today - last).num_days() - 1;
        if missed_days <= 0 {
            return;
        }
        if missed_days == 1 && self.current_streak >= SHIELD_STREAK_THRESHOLD && self.has_shield {
            self.has_shield = false;
        } else {
            self.current_streak = 0;
        }
        self.last_settled = Some(today - Duration::days(1));
    }

    pub fn record_goal_met(&mut self, today: NaiveDate) {
        self.current_streak += 1;
        self.best_streak = self.best_streak.max(self.current_streak);
        self.last_settled = Some(today);
        if self.current_streak >= SHIELD_STREAK_THRESHOLD && !self.has_shield {
            self.has_shield = true;
        }
    }
}

impl Default for StreakState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(offset_days: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + Duration::days(offset_days)
    }

    #[test]
    fn goal_met_increments_streak_and_updates_best() {
        let mut state = StreakState::new();
        state.record_goal_met(date(0));
        assert_eq!(state.current_streak, 1);
        assert_eq!(state.best_streak, 1);

        state.settle_missed_days(date(1));
        state.record_goal_met(date(1));
        assert_eq!(state.current_streak, 2);
        assert_eq!(state.best_streak, 2);
    }

    #[test]
    fn missed_day_resets_streak_but_keeps_best() {
        let mut state = StreakState::new();
        for day in 0..5 {
            state.settle_missed_days(date(day));
            state.record_goal_met(date(day));
        }
        assert_eq!(state.current_streak, 5);
        assert_eq!(state.best_streak, 5);

        state.settle_missed_days(date(6));
        assert_eq!(state.current_streak, 0);
        assert_eq!(state.best_streak, 5);
    }

    #[test]
    fn shield_is_earned_at_seven_day_streak() {
        let mut state = StreakState::new();
        for day in 0..6 {
            state.settle_missed_days(date(day));
            state.record_goal_met(date(day));
        }
        assert!(!state.has_shield);

        state.settle_missed_days(date(6));
        state.record_goal_met(date(6));
        assert_eq!(state.current_streak, 7);
        assert!(state.has_shield);
    }

    #[test]
    fn shield_preserves_streak_across_a_single_missed_day() {
        let mut state = StreakState {
            current_streak: 8,
            best_streak: 8,
            has_shield: true,
            last_settled: Some(date(6)),
        };

        state.settle_missed_days(date(8));

        assert_eq!(state.current_streak, 8);
        assert!(!state.has_shield);
        assert_eq!(state.last_settled, Some(date(7)));
    }

    #[test]
    fn shield_does_not_cover_two_consecutive_missed_days() {
        let mut state = StreakState {
            current_streak: 8,
            best_streak: 8,
            has_shield: true,
            last_settled: Some(date(6)),
        };

        state.settle_missed_days(date(9));

        assert_eq!(state.current_streak, 0);
        assert!(state.has_shield);
    }

    #[test]
    fn shield_is_not_used_below_seven_day_streak() {
        let mut state = StreakState {
            current_streak: 3,
            best_streak: 3,
            has_shield: true,
            last_settled: Some(date(6)),
        };

        state.settle_missed_days(date(8));

        assert_eq!(state.current_streak, 0);
        assert!(state.has_shield);
    }

    #[test]
    fn settling_the_same_day_twice_is_a_no_op() {
        let mut state = StreakState::new();
        state.record_goal_met(date(0));
        let after_first = state;

        state.settle_missed_days(date(0));

        assert_eq!(state, after_first);
    }
}
