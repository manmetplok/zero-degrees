//! Trophy definitions and awarding (story 018): pure state, no rendering.
//! Each trophy counts qualifying [`ClearEvent`]s; crossing a tier threshold
//! awards that tier exactly once (counters only ever grow, and each event
//! adds at most one, so a threshold is crossed at most once).

use crate::meta::{Sentiment, Urgency};
use crate::progress::ClearEvent;

/// Response time under which a clear counts as "fast" (Speed Demon).
pub const FAST_CLEAR_SECONDS: f64 = 5.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrophyId {
    SpeedDemon,
    Firefighter,
    Peacekeeper,
    CleanSweep,
    HighJumper,
}

impl TrophyId {
    pub const ALL: [TrophyId; 5] = [
        TrophyId::SpeedDemon,
        TrophyId::Firefighter,
        TrophyId::Peacekeeper,
        TrophyId::CleanSweep,
        TrophyId::HighJumper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrophyId::SpeedDemon => "Speed Demon",
            TrophyId::Firefighter => "Firefighter",
            TrophyId::Peacekeeper => "Peacekeeper",
            TrophyId::CleanSweep => "Clean Sweep",
            TrophyId::HighJumper => "High Jumper",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            TrophyId::SpeedDemon => "Clear hurdles in under 5 min",
            TrophyId::Firefighter => "Douse burning hurdles",
            TrophyId::Peacekeeper => "Clear angry-aura hurdles",
            TrophyId::CleanSweep => "Empty the track",
            TrophyId::HighJumper => "Clear critical hurdles",
        }
    }

    /// Stable name for the save file.
    pub fn key(self) -> &'static str {
        match self {
            TrophyId::SpeedDemon => "speed_demon",
            TrophyId::Firefighter => "firefighter",
            TrophyId::Peacekeeper => "peacekeeper",
            TrophyId::CleanSweep => "clean_sweep",
            TrophyId::HighJumper => "high_jumper",
        }
    }

    /// Qualifying events needed for bronze (the story's earning condition).
    pub fn bronze_goal(self) -> u32 {
        match self {
            TrophyId::SpeedDemon | TrophyId::HighJumper => 10,
            TrophyId::Firefighter | TrophyId::Peacekeeper | TrophyId::CleanSweep => 5,
        }
    }

    /// Count needed for a tier: bronze at the story goal, then 3x and 10x.
    pub fn threshold(self, tier: Tier) -> u32 {
        self.bronze_goal() * tier.factor()
    }

    /// Does this event count toward this trophy?
    fn counts(self, ev: &ClearEvent) -> bool {
        match self {
            TrophyId::SpeedDemon => ev.response_seconds < FAST_CLEAR_SECONDS,
            TrophyId::Firefighter => ev.was_burning,
            TrophyId::Peacekeeper => ev.sentiment == Sentiment::Angry,
            TrophyId::CleanSweep => ev.track_cleared,
            TrophyId::HighJumper => ev.urgency == Urgency::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Bronze, Tier::Silver, Tier::Gold];

    pub fn label(self) -> &'static str {
        match self {
            Tier::Bronze => "Bronze",
            Tier::Silver => "Silver",
            Tier::Gold => "Gold",
        }
    }

    fn factor(self) -> u32 {
        match self {
            Tier::Bronze => 1,
            Tier::Silver => 3,
            Tier::Gold => 10,
        }
    }
}

/// A trophy (tier) that was just earned — one full-screen celebration each.
#[derive(Debug, PartialEq, Eq)]
pub struct TrophyAward {
    pub id: TrophyId,
    pub tier: Tier,
}

/// The player's trophy cabinet: one monotonic counter per trophy.
pub struct TrophyCase {
    counts: [u32; TrophyId::ALL.len()],
}

impl TrophyCase {
    pub fn new() -> Self {
        Self {
            counts: [0; TrophyId::ALL.len()],
        }
    }

    pub fn from_counts(counts: [u32; TrophyId::ALL.len()]) -> Self {
        Self { counts }
    }

    pub fn count(&self, id: TrophyId) -> u32 {
        self.counts[Self::index(id)]
    }

    fn index(id: TrophyId) -> usize {
        TrophyId::ALL.iter().position(|t| *t == id).expect("in ALL")
    }

    /// Highest tier earned, or None while still working toward bronze.
    pub fn tier(&self, id: TrophyId) -> Option<Tier> {
        Tier::ALL
            .iter()
            .rev()
            .copied()
            .find(|t| self.count(id) >= id.threshold(*t))
    }

    /// Progress toward the next tier as (count, needed); None once gold.
    pub fn next_tier_progress(&self, id: TrophyId) -> Option<(u32, u32)> {
        Tier::ALL
            .iter()
            .copied()
            .find(|t| self.count(id) < id.threshold(*t))
            .map(|t| (self.count(id), id.threshold(t)))
    }

    /// Feed one clear; returns every tier newly earned by it (each at most
    /// once, ever, since counters never decrease).
    pub fn on_clear(&mut self, ev: &ClearEvent) -> Vec<TrophyAward> {
        let mut awards = Vec::new();
        for id in TrophyId::ALL {
            if !id.counts(ev) {
                continue;
            }
            let n = &mut self.counts[Self::index(id)];
            *n += 1;
            for tier in Tier::ALL {
                if *n == id.threshold(tier) {
                    awards.push(TrophyAward { id, tier });
                }
            }
        }
        awards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Day;

    fn event() -> ClearEvent {
        ClearEvent {
            message_id: 1,
            urgency: Urgency::Normal,
            sentiment: Sentiment::Neutral,
            was_burning: false,
            response_seconds: 10_000.0,
            track_cleared: false,
            at: Day(0).0 * 86_400,
        }
    }

    #[test]
    fn plain_clear_awards_nothing() {
        let mut case = TrophyCase::new();
        assert!(case.on_clear(&event()).is_empty());
        for id in TrophyId::ALL {
            assert_eq!(case.count(id), 0);
        }
    }

    #[test]
    fn speed_demon_needs_ten_fast_clears() {
        let mut case = TrophyCase::new();
        let fast = ClearEvent {
            response_seconds: FAST_CLEAR_SECONDS - 1.0,
            ..event()
        };
        for _ in 0..9 {
            assert!(case.on_clear(&fast).is_empty());
        }
        let awards = case.on_clear(&fast);
        assert_eq!(
            awards,
            vec![TrophyAward {
                id: TrophyId::SpeedDemon,
                tier: Tier::Bronze
            }]
        );
        assert_eq!(case.tier(TrophyId::SpeedDemon), Some(Tier::Bronze));
    }

    #[test]
    fn each_condition_maps_to_its_trophy() {
        let mut case = TrophyCase::new();
        case.on_clear(&ClearEvent { was_burning: true, ..event() });
        case.on_clear(&ClearEvent { sentiment: Sentiment::Angry, ..event() });
        case.on_clear(&ClearEvent { track_cleared: true, ..event() });
        case.on_clear(&ClearEvent { urgency: Urgency::Critical, ..event() });
        assert_eq!(case.count(TrophyId::Firefighter), 1);
        assert_eq!(case.count(TrophyId::Peacekeeper), 1);
        assert_eq!(case.count(TrophyId::CleanSweep), 1);
        assert_eq!(case.count(TrophyId::HighJumper), 1);
        assert_eq!(case.count(TrophyId::SpeedDemon), 0);
    }

    #[test]
    fn one_event_can_feed_several_trophies() {
        let mut case = TrophyCase::new();
        let big = ClearEvent {
            urgency: Urgency::Critical,
            sentiment: Sentiment::Angry,
            was_burning: true,
            response_seconds: 1.0,
            track_cleared: true,
            ..event()
        };
        case.on_clear(&big);
        for id in TrophyId::ALL {
            assert_eq!(case.count(id), 1);
        }
    }

    #[test]
    fn awards_fire_once_then_progress_toward_the_next_tier() {
        let mut case = TrophyCase::new();
        let burning = ClearEvent { was_burning: true, ..event() };
        let mut bronzes = 0;
        let mut silvers = 0;
        for _ in 0..TrophyId::Firefighter.threshold(Tier::Silver) {
            for award in case.on_clear(&burning) {
                match award.tier {
                    Tier::Bronze => bronzes += 1,
                    Tier::Silver => silvers += 1,
                    Tier::Gold => panic!("gold too early"),
                }
            }
        }
        assert_eq!((bronzes, silvers), (1, 1));
        assert_eq!(case.tier(TrophyId::Firefighter), Some(Tier::Silver));
        let (have, need) = case.next_tier_progress(TrophyId::Firefighter).unwrap();
        assert_eq!(have, 15);
        assert_eq!(need, TrophyId::Firefighter.threshold(Tier::Gold));
    }

    #[test]
    fn gold_is_the_end_of_the_ladder() {
        let mut case = TrophyCase::new();
        let burning = ClearEvent { was_burning: true, ..event() };
        for _ in 0..TrophyId::Firefighter.threshold(Tier::Gold) + 5 {
            case.on_clear(&burning);
        }
        assert_eq!(case.tier(TrophyId::Firefighter), Some(Tier::Gold));
        assert_eq!(case.next_tier_progress(TrophyId::Firefighter), None);
    }

    #[test]
    fn counts_round_trip_through_from_counts() {
        let mut case = TrophyCase::new();
        for _ in 0..7 {
            case.on_clear(&ClearEvent { urgency: Urgency::Critical, ..event() });
        }
        let restored = TrophyCase::from_counts(case.counts);
        assert_eq!(restored.count(TrophyId::HighJumper), 7);
        // Restoring never re-fires past awards: the next award is bronze at
        // 10, reached by new events only.
        assert_eq!(restored.tier(TrophyId::HighJumper), None);
    }
}
