//! Mock team + relay-handoff rules (story 013) and the deterministic
//! teammate simulation behind the team screens (stories 011/017/019).
//!
//! There is no multiplayer backend yet: three fake teammates advance on
//! fixed schedules over demo time, behind a small interface a real API can
//! replace (see api-changerequests/team-screens.md). The local player's own
//! numbers always come from the live game state; only teammates are mocked.
//!
//! Pure rules (clock, schedules, assignment) live at the top and are unit
//! tested; the avatar drawing helpers sit at the bottom.

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::Message;

use crate::view;

/// One real second advances the simulated inbox clock this many seconds, so
/// hurdles visibly age into "burning" during a short demo session.
pub const TIME_SCALE: f64 = 30.0;

/// A message is burning (overdue) once it has waited this long, in simulated
/// seconds. Stand-in for story 014's real rule — every caller goes through
/// `is_burning`, so swapping in the real predicate is a one-line change.
pub const BURNING_AGE_SECS: i64 = 3600;

pub fn is_burning(msg: &Message, sim_now: i64) -> bool {
    sim_now - msg.received_at >= BURNING_AGE_SECS
}

/// Maps real elapsed seconds to a simulated unix timestamp, anchored on the
/// newest message so the mock inbox starts "just now".
#[derive(Clone, Copy)]
pub struct SimClock {
    pub base_ts: i64,
}

impl SimClock {
    pub fn from_messages(received: impl Iterator<Item = i64>) -> Self {
        Self {
            base_ts: received.max().unwrap_or(0),
        }
    }

    pub fn now(&self, elapsed_real: f64) -> i64 {
        self.base_ts + (elapsed_real * TIME_SCALE) as i64
    }
}

/// A runner on the team: the local player or one of the mock teammates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunnerId {
    Me,
    Bot(usize),
}

/// Static profile + schedule of one mock teammate. XP baselines are what the
/// bot had "already earned" per leaderboard period when the session started;
/// live clears are added on top.
pub struct BotSpec {
    pub name: &'static str,
    pub color: Color,
    /// Real seconds into the session when the bot lands its first clear.
    pub first_clear_at: f64,
    /// Real seconds between clears after the first.
    pub clear_period: f64,
    pub xp_per_clear: u32,
    pub xp_today: u64,
    pub xp_week: u64,
    pub xp_all: u64,
    pub streak_days: u32,
    pub badges: u32,
    pub avg_response_secs: i64,
}

pub const BOTS: [BotSpec; 3] = [
    BotSpec {
        name: "Sana",
        color: Color::new(1.0, 0.55, 0.35, 1.0),
        first_clear_at: 12.0,
        clear_period: 18.0,
        xp_per_clear: 130,
        xp_today: 620,
        xp_week: 3400,
        xp_all: 15200,
        streak_days: 6,
        badges: 9,
        avg_response_secs: 14 * 60,
    },
    BotSpec {
        name: "Kim",
        color: Color::new(0.45, 0.80, 1.0, 1.0),
        first_clear_at: 20.0,
        clear_period: 26.0,
        xp_per_clear: 110,
        xp_today: 540,
        xp_week: 4100,
        xp_all: 22400,
        streak_days: 12,
        badges: 14,
        avg_response_secs: 9 * 60,
    },
    BotSpec {
        name: "Diego",
        color: Color::new(0.72, 0.88, 0.34, 1.0),
        first_clear_at: 9.0,
        clear_period: 22.0,
        xp_per_clear: 90,
        xp_today: 380,
        xp_week: 1900,
        xp_all: 8800,
        streak_days: 2,
        badges: 4,
        avg_response_secs: 21 * 60,
    },
];

pub fn runner_name(runner: RunnerId) -> &'static str {
    match runner {
        RunnerId::Me => "You",
        RunnerId::Bot(i) => BOTS[i].name,
    }
}

pub fn runner_color(runner: RunnerId) -> Color {
    match runner {
        RunnerId::Me => view::ACCENT,
        RunnerId::Bot(i) => BOTS[i].color,
    }
}

/// How many clears a bot's schedule has produced after `elapsed` real
/// seconds: one at `first_clear_at`, then one every `clear_period`.
pub fn scheduled_clears(spec: &BotSpec, elapsed: f64) -> u32 {
    if elapsed < spec.first_clear_at {
        0
    } else {
        ((elapsed - spec.first_clear_at) / spec.clear_period) as u32 + 1
    }
}

/// Team state: baton assignments (story 013) plus the live progress of the
/// mock teammates. A scheduled bot clear only "lands" when an eligible open
/// hurdle exists; landed clears are what count for XP and progress.
pub struct Team {
    assignments: HashMap<u64, RunnerId>,
    /// Scheduled clears consumed per bot, landed or skipped.
    consumed: [u32; BOTS.len()],
    /// Clears that actually landed on a hurdle, per bot.
    pub bot_clears: [u32; BOTS.len()],
    /// Real time of each runner's latest clear, for live pulses in the UI.
    pub last_clear_at: HashMap<RunnerId, f64>,
}

impl Team {
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
            consumed: [0; BOTS.len()],
            bot_clears: [0; BOTS.len()],
            last_clear_at: HashMap::new(),
        }
    }

    // ---- story 013: baton ownership ----

    pub fn assignee(&self, id: u64) -> Option<RunnerId> {
        self.assignments.get(&id).copied()
    }

    /// Assign (or reassign) a hurdle to exactly one runner; returns the
    /// previous owner. The hurdle keeps all message data — only the lane
    /// changes.
    pub fn assign(&mut self, id: u64, to: RunnerId) -> Option<RunnerId> {
        self.assignments.insert(id, to)
    }

    /// Claim an unassigned hurdle. Fails when someone already owns it.
    /// (The track's tap-cycle goes through `cycle`; this is the rule the
    /// real assignment API will enforce, kept and tested for that swap.)
    #[allow(dead_code)]
    pub fn claim(&mut self, id: u64, by: RunnerId) -> bool {
        if self.assignments.contains_key(&id) {
            return false;
        }
        self.assignments.insert(id, by);
        true
    }

    #[allow(dead_code)]
    pub fn unassign(&mut self, id: u64) -> Option<RunnerId> {
        self.assignments.remove(&id)
    }

    /// Tap-cycle used by the track: unassigned → you (claim) → each teammate
    /// (pass the baton) → unassigned. Returns the new owner.
    pub fn cycle(&mut self, id: u64) -> Option<RunnerId> {
        let next = match self.assignee(id) {
            None => Some(RunnerId::Me),
            Some(RunnerId::Me) => Some(RunnerId::Bot(0)),
            Some(RunnerId::Bot(i)) if i + 1 < BOTS.len() => Some(RunnerId::Bot(i + 1)),
            Some(RunnerId::Bot(_)) => None,
        };
        match next {
            Some(r) => {
                self.assignments.insert(id, r);
            }
            None => {
                self.assignments.remove(&id);
            }
        }
        next
    }

    /// "My lane": a hurdle is in a runner's lane iff they own it. (The live
    /// lane view goes through `FilterRequest::Assignee`; this is the pure
    /// rule, kept and tested for the backend swap.)
    #[allow(dead_code)]
    pub fn in_lane(&self, id: u64, runner: RunnerId) -> bool {
        self.assignee(id) == Some(runner)
    }

    // ---- deterministic teammate simulation ----

    /// Whether this bot's schedule has an unconsumed clear pending.
    pub fn clear_due(&self, bot: usize, elapsed: f64) -> bool {
        scheduled_clears(&BOTS[bot], elapsed) > self.consumed[bot]
    }

    /// A scheduled clear landed on a real hurdle.
    pub fn record_landed(&mut self, bot: usize, now: f64) {
        self.consumed[bot] += 1;
        self.bot_clears[bot] += 1;
        self.last_clear_at.insert(RunnerId::Bot(bot), now);
    }

    /// A scheduled clear found no eligible hurdle and is forfeited.
    pub fn record_skipped(&mut self, bot: usize) {
        self.consumed[bot] += 1;
    }
}

// ---- drawing ----

/// Round runner avatar: colored disc, ring, and the name's initial.
pub fn avatar(cx: f32, cy: f32, r: f32, runner: RunnerId) {
    let color = runner_color(runner);
    draw_circle(cx, cy, r, color);
    draw_circle_lines(cx, cy, r, (r * 0.16).max(1.5), view::INK);
    let initial: String = runner_name(runner).chars().take(1).collect();
    let fs = r * 1.5;
    let dims = measure_text(&initial, None, fs as u16, 1.0);
    draw_text(&initial, cx - dims.width / 2.0, cy + fs * 0.36, fs, view::SKY);
}

/// Assignee avatar pinned to a hurdle's sign (mirrors the sign geometry in
/// view::hurdle so it sits on the sign's top-left corner).
pub fn hurdle_avatar(x: f32, ground_y: f32, unit: f32, runner: RunnerId) {
    let sign = unit * 0.9;
    let sign_cy = ground_y - unit * 1.3 - sign * 0.75;
    avatar(x - sign * 0.62, sign_cy - sign * 0.62, sign * 0.26, runner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Channel, MessageStatus};

    fn msg(id: u64, received_at: i64) -> Message {
        Message {
            id,
            channel: Channel::Email,
            sender: "t@example.com".into(),
            subject: "subject".into(),
            body: "body".into(),
            received_at,
            status: MessageStatus::Open,
        }
    }

    #[test]
    fn burning_starts_exactly_at_the_age_threshold() {
        let m = msg(1, 1_000);
        assert!(!is_burning(&m, 1_000 + BURNING_AGE_SECS - 1));
        assert!(is_burning(&m, 1_000 + BURNING_AGE_SECS));
    }

    #[test]
    fn sim_clock_anchors_on_newest_message_and_scales_time() {
        let clock = SimClock::from_messages([100, 900, 400].into_iter());
        assert_eq!(clock.base_ts, 900);
        assert_eq!(clock.now(0.0), 900);
        assert_eq!(clock.now(2.0), 900 + (2.0 * TIME_SCALE) as i64);
    }

    #[test]
    fn scheduled_clears_follow_first_clear_and_period() {
        let spec = &BOTS[0];
        assert_eq!(scheduled_clears(spec, spec.first_clear_at - 0.1), 0);
        assert_eq!(scheduled_clears(spec, spec.first_clear_at), 1);
        assert_eq!(
            scheduled_clears(spec, spec.first_clear_at + spec.clear_period * 2.5),
            3
        );
    }

    #[test]
    fn claim_only_succeeds_on_unassigned_hurdles() {
        let mut team = Team::new();
        assert!(team.claim(7, RunnerId::Me));
        assert!(!team.claim(7, RunnerId::Bot(0)), "taken hurdles can't be claimed");
        assert_eq!(team.assignee(7), Some(RunnerId::Me));
    }

    #[test]
    fn reassignment_moves_lanes_and_reports_the_previous_owner() {
        let mut team = Team::new();
        team.assign(7, RunnerId::Bot(1));
        assert!(team.in_lane(7, RunnerId::Bot(1)));
        let prev = team.assign(7, RunnerId::Me);
        assert_eq!(prev, Some(RunnerId::Bot(1)));
        assert!(team.in_lane(7, RunnerId::Me), "hurdle enters the new lane");
        assert!(!team.in_lane(7, RunnerId::Bot(1)), "and leaves the old one");
    }

    #[test]
    fn cycle_walks_claim_then_each_teammate_then_unassigned() {
        let mut team = Team::new();
        assert_eq!(team.cycle(1), Some(RunnerId::Me));
        assert_eq!(team.cycle(1), Some(RunnerId::Bot(0)));
        assert_eq!(team.cycle(1), Some(RunnerId::Bot(1)));
        assert_eq!(team.cycle(1), Some(RunnerId::Bot(2)));
        assert_eq!(team.cycle(1), None);
        assert_eq!(team.assignee(1), None);
    }

    #[test]
    fn bot_clears_only_count_when_they_land() {
        let mut team = Team::new();
        let elapsed = BOTS[0].first_clear_at + 0.1;
        assert!(team.clear_due(0, elapsed));
        team.record_skipped(0);
        assert!(!team.clear_due(0, elapsed), "skipped clears are consumed");
        assert_eq!(team.bot_clears[0], 0);
        let later = BOTS[0].first_clear_at + BOTS[0].clear_period;
        assert!(team.clear_due(0, later));
        team.record_landed(0, 1.0);
        assert_eq!(team.bot_clears[0], 1);
    }
}
