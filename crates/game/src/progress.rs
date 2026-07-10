//! Daily runs and streaks (story 016): pure state, no rendering. Callers
//! pass "today" in as a [`Day`] instead of the module reading a clock, the
//! same way score.rs takes `now` — the mock world runs on fake timestamps,
//! so the rules stay testable without touching the wall clock.

use crate::meta::{Sentiment, Urgency};

/// Seconds per calendar day.
const DAY_SECONDS: i64 = 86_400;

/// The mock world's "now" anchor: just after the newest sample message in
/// inbox.rs. The game derives world time as `MOCK_WORLD_EPOCH + get_time()`;
/// when real server time lands, only that derivation changes.
pub const MOCK_WORLD_EPOCH: i64 = 1_780_004_000;

/// A calendar day as a whole number of days since the Unix epoch. Purely a
/// value — deriving it from a timestamp is the caller's job, so tests can
/// hand-pick days without any clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(pub i64);

impl Day {
    pub fn from_unix(ts: i64) -> Self {
        Day(ts.div_euclid(DAY_SECONDS))
    }
}

/// Everything a trophy or streak check needs to know about one cleared
/// hurdle. Built in the game's clear path; urgency/sentiment come from
/// meta.rs until the dedicated story branches land and swap in their data.
pub struct ClearEvent {
    /// Which message was cleared — for server-side award sync and dedup;
    /// no local rule reads it yet.
    #[allow(dead_code)]
    pub message_id: u64,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
    /// Whether the hurdle was on fire when cleared (story 014). Wired to
    /// `false` until the burning-hurdles branch merges.
    pub was_burning: bool,
    /// How long the message waited for this reply, in seconds. Stand-in:
    /// time on this session's track, until real received→replied timing
    /// exists server-side.
    pub response_seconds: f64,
    /// Did this clear leave the track empty?
    pub track_cleared: bool,
    /// World timestamp (unix seconds) of the clear.
    pub at: i64,
}

/// Daily goal: either enough hurdles cleared or enough XP earned counts.
pub const GOAL_CLEARS: u32 = 5;
pub const GOAL_XP: u64 = 600;
/// Streak length at which a streak shield is (re)earned.
pub const SHIELD_AT: u32 = 7;

/// Something the streak logic wants the UI to celebrate or mourn.
#[derive(Debug, PartialEq, Eq)]
pub enum DayEvent {
    /// Today's goal was just reached; the streak already includes today.
    GoalMet { streak: u32, shield_earned: bool },
    /// A missed day was absorbed by the streak shield.
    ShieldUsed { streak: u32 },
    /// The streak broke; `best` is the record that survives it.
    StreakBroken { lost: u32, best: u32 },
}

/// Per-player daily-run state. Counters cover the current [`Day`]; streak
/// bookkeeping happens when the day rolls over (`tick`) or the goal is met.
pub struct Daily {
    pub day: Day,
    pub clears_today: u32,
    pub xp_today: u64,
    pub goal_met: bool,
    pub streak: u32,
    pub best_streak: u32,
    pub shields: u32,
}

impl Daily {
    pub fn new(today: Day) -> Self {
        Self {
            day: today,
            clears_today: 0,
            xp_today: 0,
            goal_met: false,
            streak: 0,
            best_streak: 0,
            shields: 0,
        }
    }

    /// Fraction of the daily goal reached, on whichever axis is furthest.
    pub fn goal_progress(&self) -> f32 {
        let by_clears = self.clears_today as f32 / GOAL_CLEARS as f32;
        let by_xp = self.xp_today as f32 / GOAL_XP as f32;
        by_clears.max(by_xp).min(1.0)
    }

    /// Advance to `today`, settling any days that ended in between. A day
    /// that ended below the goal is a miss; one miss is absorbed by a shield
    /// when the streak has earned one, otherwise the streak resets (the
    /// personal best is kept). Call once per frame — it is a no-op while the
    /// day is unchanged.
    pub fn tick(&mut self, today: Day) -> Option<DayEvent> {
        let gap = today.0 - self.day.0;
        if gap <= 0 {
            return None;
        }
        // Days fully skipped are misses; the day that just ended is one too
        // unless its goal was met.
        let misses = gap - 1 + if self.goal_met { 0 } else { 1 };
        self.day = today;
        self.clears_today = 0;
        self.xp_today = 0;
        self.goal_met = false;
        if misses == 0 || self.streak == 0 {
            return None;
        }
        if misses == 1 && self.streak >= SHIELD_AT && self.shields > 0 {
            self.shields -= 1;
            return Some(DayEvent::ShieldUsed { streak: self.streak });
        }
        let lost = self.streak;
        self.streak = 0;
        Some(DayEvent::StreakBroken {
            lost,
            best: self.best_streak,
        })
    }

    /// Register one cleared hurdle worth `xp` on `today`. Returns every
    /// event this produced (a pending rollover may settle first).
    pub fn on_clear(&mut self, xp: u32, today: Day) -> Vec<DayEvent> {
        let mut events = Vec::new();
        if let Some(ev) = self.tick(today) {
            events.push(ev);
        }
        self.clears_today += 1;
        self.xp_today += u64::from(xp);
        if !self.goal_met && (self.clears_today >= GOAL_CLEARS || self.xp_today >= GOAL_XP) {
            self.goal_met = true;
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
            let shield_earned = self.streak >= SHIELD_AT && self.shields == 0;
            if shield_earned {
                self.shields = 1;
            }
            events.push(DayEvent::GoalMet {
                streak: self.streak,
                shield_earned,
            });
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meet the goal on `day` by clearing hurdles; returns the GoalMet event.
    fn meet_goal(daily: &mut Daily, day: Day) -> DayEvent {
        for _ in 0..GOAL_CLEARS - 1 {
            assert!(daily.on_clear(50, day).is_empty());
        }
        let mut events = daily.on_clear(50, day);
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    #[test]
    fn day_from_unix_maps_timestamps_to_whole_days() {
        assert_eq!(Day::from_unix(0), Day(0));
        assert_eq!(Day::from_unix(86_399), Day(0));
        assert_eq!(Day::from_unix(86_400), Day(1));
        assert_eq!(Day::from_unix(-1), Day(-1));
    }

    #[test]
    fn goal_met_by_clears_increments_streak_once() {
        let mut daily = Daily::new(Day(10));
        let ev = meet_goal(&mut daily, Day(10));
        assert_eq!(
            ev,
            DayEvent::GoalMet {
                streak: 1,
                shield_earned: false
            }
        );
        // Clearing more the same day does not increment again.
        assert!(daily.on_clear(50, Day(10)).is_empty());
        assert_eq!(daily.streak, 1);
        assert_eq!(daily.best_streak, 1);
    }

    #[test]
    fn goal_met_by_xp_alone() {
        let mut daily = Daily::new(Day(10));
        let events = daily.on_clear(GOAL_XP as u32, Day(10));
        assert!(matches!(events[0], DayEvent::GoalMet { streak: 1, .. }));
    }

    #[test]
    fn consecutive_days_grow_the_streak() {
        let mut daily = Daily::new(Day(1));
        for d in 1..=3 {
            meet_goal(&mut daily, Day(d));
        }
        assert_eq!(daily.streak, 3);
        assert_eq!(daily.best_streak, 3);
    }

    #[test]
    fn missed_day_resets_streak_but_keeps_best() {
        let mut daily = Daily::new(Day(1));
        meet_goal(&mut daily, Day(1));
        meet_goal(&mut daily, Day(2));
        // Day 3 passes without the goal; noticed on day 4.
        let ev = daily.tick(Day(4));
        assert_eq!(ev, Some(DayEvent::StreakBroken { lost: 2, best: 2 }));
        assert_eq!(daily.streak, 0);
        assert_eq!(daily.best_streak, 2);
        // The next run starts a fresh streak under the old best.
        meet_goal(&mut daily, Day(4));
        assert_eq!(daily.streak, 1);
        assert_eq!(daily.best_streak, 2);
    }

    #[test]
    fn day_ending_below_goal_counts_as_a_miss() {
        let mut daily = Daily::new(Day(1));
        meet_goal(&mut daily, Day(1));
        daily.on_clear(50, Day(2)); // some play on day 2, goal not met
        let ev = daily.tick(Day(3));
        assert_eq!(ev, Some(DayEvent::StreakBroken { lost: 1, best: 1 }));
    }

    #[test]
    fn shield_is_earned_at_seven_and_absorbs_one_miss() {
        let mut daily = Daily::new(Day(0));
        for d in 1..=7 {
            let ev = meet_goal(&mut daily, Day(d));
            let expect_shield = d == 7;
            assert_eq!(
                ev,
                DayEvent::GoalMet {
                    streak: d as u32,
                    shield_earned: expect_shield
                }
            );
        }
        assert_eq!(daily.shields, 1);
        // Day 8 is missed entirely; the shield eats it.
        let ev = daily.tick(Day(9));
        assert_eq!(ev, Some(DayEvent::ShieldUsed { streak: 7 }));
        assert_eq!(daily.streak, 7);
        assert_eq!(daily.shields, 0);
        // The streak keeps growing and the shield is re-earned.
        let ev = meet_goal(&mut daily, Day(9));
        assert_eq!(
            ev,
            DayEvent::GoalMet {
                streak: 8,
                shield_earned: true
            }
        );
    }

    #[test]
    fn shield_does_not_cover_short_streaks_or_double_misses() {
        // Short streak: no shield earned, a miss breaks it.
        let mut daily = Daily::new(Day(0));
        for d in 1..=3 {
            meet_goal(&mut daily, Day(d));
        }
        assert_eq!(daily.shields, 0);
        assert_eq!(
            daily.tick(Day(5)),
            Some(DayEvent::StreakBroken { lost: 3, best: 3 })
        );

        // Long streak with a shield, but two consecutive misses still break.
        let mut daily = Daily::new(Day(0));
        for d in 1..=8 {
            meet_goal(&mut daily, Day(d));
        }
        assert_eq!(daily.shields, 1);
        let ev = daily.tick(Day(11)); // days 9 and 10 both missed
        assert_eq!(ev, Some(DayEvent::StreakBroken { lost: 8, best: 8 }));
        // The shield is only spent on saves, not on breaks.
        assert_eq!(daily.shields, 1);
    }

    #[test]
    fn rollover_settles_inside_on_clear_too() {
        let mut daily = Daily::new(Day(0));
        meet_goal(&mut daily, Day(0));
        // First clear two days later: break event, then normal counting.
        let events = daily.on_clear(50, Day(2));
        assert_eq!(events, vec![DayEvent::StreakBroken { lost: 1, best: 1 }]);
        assert_eq!(daily.clears_today, 1);
    }

    #[test]
    fn goal_progress_tracks_the_leading_axis() {
        let mut daily = Daily::new(Day(0));
        assert_eq!(daily.goal_progress(), 0.0);
        daily.on_clear(300, Day(0)); // 1/5 clears, 300/600 xp
        assert!((daily.goal_progress() - 0.5).abs() < 1e-6);
    }
}
