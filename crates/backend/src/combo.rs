use shared::UrgencyLevel;

pub const COMBO_WINDOW_MS: i64 = 6_000;

const SPEED_BONUS_XP: i64 = 15;

fn base_xp(urgency: UrgencyLevel) -> i64 {
    match urgency {
        UrgencyLevel::Low => 10,
        UrgencyLevel::Normal => 20,
        UrgencyLevel::High => 35,
        UrgencyLevel::Critical => 60,
    }
}

pub fn multiplier_for(combo_count: i32) -> f64 {
    if combo_count <= 1 {
        1.0
    } else if combo_count == 2 {
        1.5
    } else {
        2.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ComboState {
    pub count: i32,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearOutcome {
    pub xp_awarded: i64,
    pub multiplier: f64,
    pub combo_count: i32,
    pub window_remaining_ms: i64,
    pub combo_expires_at_ms: i64,
}

pub fn resolve_clear(
    now_ms: i64,
    previous: Option<ComboState>,
    urgency: UrgencyLevel,
    on_time: bool,
) -> ClearOutcome {
    let combo_count = match previous {
        Some(state) if now_ms <= state.expires_at_ms => state.count + 1,
        _ => 1,
    };
    let multiplier = multiplier_for(combo_count);
    let raw_xp = base_xp(urgency) + if on_time { SPEED_BONUS_XP } else { 0 };
    let xp_awarded = (raw_xp as f64 * multiplier).round() as i64;
    ClearOutcome {
        xp_awarded,
        multiplier,
        combo_count,
        window_remaining_ms: COMBO_WINDOW_MS,
        combo_expires_at_ms: now_ms + COMBO_WINDOW_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_clear_ever_gets_no_multiplier() {
        let outcome = resolve_clear(1_000, None, UrgencyLevel::Low, false);
        assert_eq!(outcome.combo_count, 1);
        assert_eq!(outcome.multiplier, 1.0);
        assert_eq!(outcome.xp_awarded, 10);
    }

    #[test]
    fn base_xp_scales_with_urgency() {
        let awarded = |urgency| resolve_clear(0, None, urgency, false).xp_awarded;
        assert_eq!(awarded(UrgencyLevel::Low), 10);
        assert_eq!(awarded(UrgencyLevel::Normal), 20);
        assert_eq!(awarded(UrgencyLevel::High), 35);
        assert_eq!(awarded(UrgencyLevel::Critical), 60);
    }

    #[test]
    fn on_time_clear_adds_speed_bonus() {
        let on_time = resolve_clear(0, None, UrgencyLevel::Normal, true).xp_awarded;
        let late = resolve_clear(0, None, UrgencyLevel::Normal, false).xp_awarded;
        assert_eq!(on_time - late, 15);
    }

    #[test]
    fn consecutive_clears_within_window_grow_multiplier_in_steps() {
        let first = resolve_clear(0, None, UrgencyLevel::Low, false);
        let second = resolve_clear(
            1_000,
            Some(ComboState {
                count: first.combo_count,
                expires_at_ms: first.combo_expires_at_ms,
            }),
            UrgencyLevel::Low,
            false,
        );
        let third = resolve_clear(
            2_000,
            Some(ComboState {
                count: second.combo_count,
                expires_at_ms: second.combo_expires_at_ms,
            }),
            UrgencyLevel::Low,
            false,
        );
        assert_eq!(
            (first.multiplier, second.multiplier, third.multiplier),
            (1.0, 1.5, 2.0)
        );
        assert_eq!(
            (first.combo_count, second.combo_count, third.combo_count),
            (1, 2, 3)
        );
    }

    #[test]
    fn multiplier_caps_at_double_past_the_third_clear() {
        let fourth = resolve_clear(
            0,
            Some(ComboState {
                count: 3,
                expires_at_ms: 10_000,
            }),
            UrgencyLevel::Low,
            false,
        );
        assert_eq!(fourth.combo_count, 4);
        assert_eq!(fourth.multiplier, 2.0);
    }

    #[test]
    fn window_lapse_resets_multiplier_without_touching_past_xp() {
        let lapsed = resolve_clear(
            10_001,
            Some(ComboState {
                count: 3,
                expires_at_ms: 10_000,
            }),
            UrgencyLevel::Low,
            false,
        );
        assert_eq!(lapsed.combo_count, 1);
        assert_eq!(lapsed.multiplier, 1.0);
        assert_eq!(lapsed.xp_awarded, 10);
    }

    #[test]
    fn clear_exactly_at_expiry_still_counts_as_in_window() {
        let still_in_window = resolve_clear(
            10_000,
            Some(ComboState {
                count: 1,
                expires_at_ms: 10_000,
            }),
            UrgencyLevel::Low,
            false,
        );
        assert_eq!(still_in_window.combo_count, 2);
    }

    #[test]
    fn clear_one_millisecond_past_expiry_breaks_the_combo() {
        let broken = resolve_clear(
            10_001,
            Some(ComboState {
                count: 1,
                expires_at_ms: 10_000,
            }),
            UrgencyLevel::Low,
            false,
        );
        assert_eq!(broken.combo_count, 1);
    }

    #[test]
    fn critical_on_time_clear_stacks_height_and_speed_bonus_with_multiplier() {
        let big_air = resolve_clear(
            0,
            Some(ComboState {
                count: 1,
                expires_at_ms: 10_000,
            }),
            UrgencyLevel::Critical,
            true,
        );
        assert_eq!(big_air.combo_count, 2);
        assert_eq!(big_air.multiplier, 1.5);
        assert_eq!(big_air.xp_awarded, 113);
    }

    #[test]
    fn every_clear_refreshes_the_window_to_the_full_duration() {
        let outcome = resolve_clear(
            5_000,
            Some(ComboState {
                count: 2,
                expires_at_ms: 6_000,
            }),
            UrgencyLevel::High,
            true,
        );
        assert_eq!(outcome.window_remaining_ms, COMBO_WINDOW_MS);
        assert_eq!(outcome.combo_expires_at_ms, 5_000 + COMBO_WINDOW_MS);
    }
}
