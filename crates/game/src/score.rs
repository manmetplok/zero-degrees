//! XP and combo scoring: pure state, no rendering. Callers pass `now`
//! (seconds) instead of the module reading a clock, so the rules stay
//! testable without macroquad.

/// XP for clearing a hurdle, before multipliers. Scales with hurdle height
/// (urgency) once story 004 lands; until then every hurdle pays the base.
pub const BASE_XP: u32 = 100;

/// Seconds after a clear during which the next clear extends the combo.
pub const COMBO_WINDOW: f64 = 10.0;

/// Multiplier per consecutive clear in a chain; the last entry is the cap.
pub const MULTIPLIERS: [f32; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

pub struct Score {
    pub xp: u64,
    /// Clears in the current combo chain; 0 when no chain is running.
    chain: usize,
    /// When the current combo window lapses.
    deadline: f64,
}

/// What one clear earned, for the score pop-up.
pub struct ClearReward {
    pub xp: u32,
    pub multiplier: f32,
}

/// Live combo info for the HUD meter.
pub struct ComboState {
    /// Multiplier the next clear will earn.
    pub next_multiplier: f32,
    /// Fraction of the combo window still left (1.0 = just cleared).
    pub remaining: f32,
}

impl Score {
    pub fn new() -> Self {
        Self {
            xp: 0,
            chain: 0,
            deadline: f64::NEG_INFINITY,
        }
    }

    /// Register a cleared hurdle at time `now`; returns what it earned.
    pub fn on_clear(&mut self, base_xp: u32, now: f64) -> ClearReward {
        if now > self.deadline {
            // Window lapsed: the multiplier resets but earned XP is kept.
            self.chain = 0;
        }
        self.chain += 1;
        let multiplier = MULTIPLIERS[(self.chain - 1).min(MULTIPLIERS.len() - 1)];
        let xp = (base_xp as f32 * multiplier).round() as u32;
        self.xp += u64::from(xp);
        self.deadline = now + COMBO_WINDOW;
        ClearReward { xp, multiplier }
    }

    /// The running combo, if the window is still open at `now`.
    pub fn combo(&self, now: f64) -> Option<ComboState> {
        if self.chain == 0 || now > self.deadline {
            return None;
        }
        Some(ComboState {
            next_multiplier: MULTIPLIERS[self.chain.min(MULTIPLIERS.len() - 1)],
            remaining: ((self.deadline - now) / COMBO_WINDOW) as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_clear_awards_base_xp() {
        let mut score = Score::new();
        let reward = score.on_clear(BASE_XP, 0.0);
        assert_eq!(reward.xp, BASE_XP);
        assert_eq!(reward.multiplier, 1.0);
        assert_eq!(score.xp, u64::from(BASE_XP));
    }

    #[test]
    fn combo_builds_over_consecutive_clears_within_the_window() {
        let mut score = Score::new();
        assert_eq!(score.on_clear(100, 0.0).multiplier, 1.0);
        assert_eq!(score.on_clear(100, 3.0).multiplier, 1.5);
        assert_eq!(score.on_clear(100, 6.0).multiplier, 2.0);
        assert_eq!(score.xp, 100 + 150 + 200);
    }

    #[test]
    fn lapsed_window_resets_multiplier_but_keeps_xp() {
        let mut score = Score::new();
        score.on_clear(100, 0.0);
        score.on_clear(100, 1.0); // ×1.5
        let reward = score.on_clear(100, 1.0 + COMBO_WINDOW + 0.1);
        assert_eq!(reward.multiplier, 1.0);
        assert_eq!(score.xp, 100 + 150 + 100);
    }

    #[test]
    fn multiplier_caps_at_the_last_entry() {
        let mut score = Score::new();
        let mut last = 0.0;
        for i in 0..MULTIPLIERS.len() + 3 {
            last = score.on_clear(100, i as f64).multiplier;
        }
        assert_eq!(last, *MULTIPLIERS.last().unwrap());
    }

    #[test]
    fn combo_meter_shows_next_multiplier_and_drains_to_none() {
        let mut score = Score::new();
        assert!(score.combo(0.0).is_none());
        score.on_clear(100, 0.0);
        let combo = score.combo(COMBO_WINDOW / 2.0).unwrap();
        assert_eq!(combo.next_multiplier, 1.5);
        assert!((combo.remaining - 0.5).abs() < 1e-6);
        assert!(score.combo(COMBO_WINDOW + 0.1).is_none());
    }
}
