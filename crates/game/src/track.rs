//! Track model: pure game state, no rendering. One open message = one hurdle
//! on a 1-D track, laid out by triage priority (urgency + sentiment + age,
//! story 009) instead of arrival order: the most important open hurdle sits
//! nearest the runner, new criticals drop in front, and waiting hurdles creep
//! up as they age. Rendering maps track distance to screen space elsewhere.

use shared::{Message, MessageStatus};

use crate::meta;

/// Distance between consecutive hurdles, in track units (1 unit ≈ 1 meter).
pub const HURDLE_SPACING: f32 = 6.0;
/// Where the first hurdle stands, giving the runner a short lead-in.
pub const FIRST_HURDLE_AT: f32 = 4.0;
/// Extra track after the last hurdle before the finish line.
pub const FINISH_MARGIN: f32 = 5.0;
/// How far ahead of the runner the front slot sits when the course reshuffles.
pub const DROP_LEAD: f32 = 3.0;
/// Hurdles this close to the runner are "engaged": relayout leaves them put
/// so an approach or jump in progress stays lined up.
pub const PIN_RANGE: f32 = 2.6;
/// Seconds between priority relayouts; age creeps priorities up slowly.
const RELAYOUT_EVERY: f32 = 1.0;

pub struct Hurdle {
    pub message: Message,
    /// Position along the track, in track units.
    pub at: f32,
    /// Where the priority layout wants this hurdle; `at` eases toward it.
    pub slot: f32,
}

pub struct Track {
    pub hurdles: Vec<Hurdle>,
    /// Runner position along the track, in track units.
    pub runner_at: f32,
    /// Where the runner is headed; `runner_at` eases toward this each frame.
    pub runner_target: f32,
    /// Free-run mode (story 009): lay the course out by arrival instead of
    /// priority, letting the player take hurdles in any order.
    pub free_run: bool,
    relayout_in: f32,
}

impl Track {
    pub fn new(messages: Vec<Message>, now: i64) -> Self {
        let hurdles = messages
            .into_iter()
            .map(|message| Hurdle {
                message,
                at: f32::INFINITY,
                slot: f32::INFINITY,
            })
            .collect();
        let mut track = Self {
            hurdles,
            runner_at: 0.0,
            runner_target: 0.0,
            free_run: false,
            relayout_in: RELAYOUT_EVERY,
        };
        track.relayout(now);
        for h in &mut track.hurdles {
            h.at = h.slot;
        }
        track
    }

    /// Hurdles still open, i.e. remaining until the finish line.
    pub fn remaining(&self) -> usize {
        self.hurdles
            .iter()
            .filter(|h| h.message.status == MessageStatus::Open)
            .count()
    }

    /// The hurdle the runner is facing: the first open one at or ahead of the
    /// runner's target position.
    pub fn next_hurdle(&self) -> Option<&Hurdle> {
        let mut open: Vec<&Hurdle> = self
            .hurdles
            .iter()
            .filter(|h| h.message.status == MessageStatus::Open)
            .collect();
        // Ties (a fresh drop-in sharing a position until the displaced
        // hurdle eases away) resolve by slot: the layout's intended order.
        open.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.slot.total_cmp(&b.slot)));
        open.iter()
            .find(|h| h.at + 0.01 >= self.runner_target)
            .or_else(|| open.first())
            .copied()
    }

    pub fn finish_at(&self) -> f32 {
        self.hurdles
            .iter()
            .map(|h| h.at.max(h.slot))
            .fold(FIRST_HURDLE_AT, f32::max)
            + FINISH_MARGIN
    }

    fn priority_of(&self, index: usize, now: i64) -> f32 {
        let message = &self.hurdles[index].message;
        // Manual category overrides don't feed priority: it depends only on
        // urgency, sentiment, and age.
        meta::priority(&meta::enrich(message), now - message.received_at)
    }

    /// Recompute every open hurdle's slot from triage priority (or arrival
    /// order in free-run mode). Highest priority lands nearest the runner;
    /// `tick` then eases positions toward their slots.
    pub fn relayout(&mut self, now: i64) {
        let mut order: Vec<usize> = (0..self.hurdles.len())
            .filter(|&i| self.hurdles[i].message.status == MessageStatus::Open)
            .collect();
        if self.free_run {
            order.sort_by_key(|&i| {
                (self.hurdles[i].message.received_at, self.hurdles[i].message.id)
            });
        } else {
            let priorities: Vec<f32> =
                order.iter().map(|&i| self.priority_of(i, now)).collect();
            let mut ranked: Vec<(usize, f32)> = order.iter().copied().zip(priorities).collect();
            ranked.sort_by(|(ia, pa), (ib, pb)| {
                pb.partial_cmp(pa)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        let (a, b) = (&self.hurdles[*ia].message, &self.hurdles[*ib].message);
                        a.received_at.cmp(&b.received_at).then(a.id.cmp(&b.id))
                    })
            });
            order = ranked.into_iter().map(|(i, _)| i).collect();
        }

        // The engaged hurdle (nearest the runner) keeps its position so an
        // approach or jump in progress stays lined up.
        let pinned = order
            .iter()
            .copied()
            .filter(|&i| (self.hurdles[i].at - self.runner_at).abs() <= PIN_RANGE)
            .min_by(|&a, &b| {
                (self.hurdles[a].at - self.runner_at)
                    .abs()
                    .total_cmp(&(self.hurdles[b].at - self.runner_at).abs())
            });

        let mut next = FIRST_HURDLE_AT.max(self.runner_at + DROP_LEAD);
        if let Some(p) = pinned {
            self.hurdles[p].slot = self.hurdles[p].at;
            next = next.max(self.hurdles[p].at + HURDLE_SPACING);
            order.retain(|&i| i != p);
        }
        for (rank, &i) in order.iter().enumerate() {
            if rank == 0 && pinned.is_none() && self.hurdles[i].at > self.runner_at {
                // The front hurdle may creep toward the runner but never
                // retreats from it: an approach in progress must converge.
                next = next.min(self.hurdles[i].at);
            }
            self.hurdles[i].slot = next;
            next += HURDLE_SPACING;
        }
    }

    /// Per-frame housekeeping: periodic priority relayout (age creeps
    /// priorities up) and easing every open hurdle toward its slot.
    pub fn tick(&mut self, dt: f32, now: i64) {
        self.relayout_in -= dt;
        if self.relayout_in <= 0.0 {
            self.relayout(now);
            self.relayout_in = RELAYOUT_EVERY;
        }
        for h in &mut self.hurdles {
            if h.message.status != MessageStatus::Open {
                continue;
            }
            let gap = h.slot - h.at;
            if gap.abs() < 1e-3 {
                h.at = h.slot;
                continue;
            }
            // Slow creep for small corrections, faster for big reshuffles.
            let speed = (gap.abs() * 0.9).clamp(0.8, 3.0);
            h.at += gap.signum() * (speed * dt).min(gap.abs());
        }
        // Never run past an open hurdle that crept behind the current
        // target: pull the target back to just short of the front one.
        let front = self
            .hurdles
            .iter()
            .filter(|h| h.message.status == MessageStatus::Open && h.at > self.runner_at)
            .map(|h| h.at)
            .fold(f32::INFINITY, f32::min);
        if front.is_finite() {
            self.runner_target = self.runner_target.min((front - 1.2).max(self.runner_at));
        }
    }

    /// Resolve the hurdle the runner is facing and move on to the next one.
    pub fn resolve_next(&mut self, status: MessageStatus, now: i64) -> Option<u64> {
        let id = self.next_hurdle().map(|h| h.message.id)?;
        let hurdle = self
            .hurdles
            .iter_mut()
            .find(|h| h.message.id == id)
            .expect("id came from next_hurdle");
        hurdle.message.status = status;
        let at = hurdle.at;
        // Resolved hurdles freeze in place; only open ones re-slot.
        hurdle.slot = at;
        self.relayout(now);
        self.runner_target = match self.next_hurdle() {
            // Stop just short of the next hurdle; approach happens there.
            Some(next) => next.at - 1.5,
            None => self.finish_at(),
        }
        .max(at);
        Some(id)
    }

    /// A new message arrives mid-run: slot it by priority — criticals drop
    /// directly in front of the runner, ahead of everything waiting.
    pub fn add_message(&mut self, message: Message, now: i64) -> &Hurdle {
        self.hurdles.push(Hurdle {
            message,
            at: f32::INFINITY,
            slot: f32::INFINITY,
        });
        self.relayout(now);
        let last = self.hurdles.len() - 1;
        // Snap straight to the slot; the drop-in animation covers the arrival.
        self.hurdles[last].at = self.hurdles[last].slot;
        &self.hurdles[last]
    }

    /// Ease the runner toward its target. Returns true while moving.
    pub fn advance(&mut self, dt: f32) -> bool {
        let gap = self.runner_target - self.runner_at;
        if gap.abs() < 0.01 {
            self.runner_at = self.runner_target;
            return false;
        }
        // Constant running speed with a soft arrival.
        let speed = (gap.abs() * 3.0).clamp(2.2, 9.0);
        self.runner_at += gap.signum() * (speed * dt).min(gap.abs());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::sample_messages;
    use crate::meta::DEMO_EPOCH;
    use shared::Channel;

    const NOW: i64 = DEMO_EPOCH;

    fn msg(id: u64, subject: &str, received_at: i64) -> Message {
        Message {
            id,
            channel: Channel::Email,
            sender: "test@example.com".to_string(),
            subject: subject.to_string(),
            body: String::new(),
            received_at,
            status: MessageStatus::Open,
        }
    }

    fn slot_of(track: &Track, id: u64) -> f32 {
        track
            .hurdles
            .iter()
            .find(|h| h.message.id == id)
            .unwrap()
            .slot
    }

    #[test]
    fn clearing_advances_runner_and_decrements_remaining() {
        let mut track = Track::new(sample_messages(3), NOW);
        assert_eq!(track.remaining(), 3);
        let first = track.next_hurdle().unwrap().message.id;
        track.resolve_next(MessageStatus::Cleared, NOW);
        assert_eq!(track.remaining(), 2);
        assert_ne!(track.next_hurdle().unwrap().message.id, first);
        assert!(track.runner_target > 0.0);
    }

    #[test]
    fn skipped_hurdles_stay_marked_and_track_finishes() {
        let mut track = Track::new(sample_messages(2), NOW);
        track.resolve_next(MessageStatus::Skipped, NOW);
        track.resolve_next(MessageStatus::Cleared, NOW);
        assert_eq!(track.remaining(), 0);
    }

    #[test]
    fn mid_run_message_gets_a_finite_slot_ahead_of_the_runner() {
        let mut track = Track::new(sample_messages(2), NOW);
        let incoming = crate::inbox::next_incoming(
            &track.hurdles.iter().map(|h| h.message.clone()).collect::<Vec<_>>(),
        );
        let hurdle = track.add_message(incoming, NOW);
        assert!(hurdle.at.is_finite());
        assert!(hurdle.at >= track.runner_at + DROP_LEAD - 0.01);
        assert_eq!(track.remaining(), 3);
    }

    #[test]
    fn finishing_all_hurdles_targets_finish_line() {
        let mut track = Track::new(sample_messages(1), NOW);
        track.resolve_next(MessageStatus::Cleared, NOW);
        assert_eq!(track.runner_target, track.finish_at());
    }

    #[test]
    fn critical_message_drops_in_front_of_the_runner() {
        let older = vec![
            msg(1, "hello there", NOW - 3600),
            msg(2, "quick question about colors", NOW - 7200),
        ];
        let mut track = Track::new(older, NOW);
        let critical = msg(3, "urgent: service is down", NOW);
        track.add_message(critical, NOW);
        assert_eq!(track.next_hurdle().unwrap().message.id, 3);
        let front = slot_of(&track, 3);
        assert!(front < slot_of(&track, 1));
        assert!(front < slot_of(&track, 2));
        assert!((front - FIRST_HURDLE_AT.max(track.runner_at + DROP_LEAD)).abs() < 0.01);
    }

    #[test]
    fn older_hurdle_of_same_urgency_sits_nearer_the_runner() {
        let messages = vec![
            msg(1, "hello fresh", NOW - 60),
            msg(2, "hello waiting", NOW - 10 * 3600),
        ];
        let track = Track::new(messages, NOW);
        assert!(slot_of(&track, 2) < slot_of(&track, 1));
    }

    #[test]
    fn long_waiting_angry_normal_creeps_past_a_fresh_high() {
        let messages = vec![
            msg(1, "can't log in", NOW - 60), // high urgency, neutral mood
            msg(2, "still nothing?!!", NOW - 16 * 3600), // normal urgency, angry
        ];
        let track = Track::new(messages, NOW);
        assert_eq!(track.next_hurdle().unwrap().message.id, 2);
    }

    #[test]
    fn free_run_lays_the_course_out_by_arrival() {
        let messages = vec![
            msg(1, "hello there", NOW - 600),
            msg(2, "urgent: service is down", NOW - 60),
        ];
        let mut track = Track::new(messages, NOW);
        // Priority order puts the critical first...
        assert!(slot_of(&track, 2) < slot_of(&track, 1));
        // ...free-run restores arrival order.
        track.free_run = true;
        track.relayout(NOW);
        assert!(slot_of(&track, 1) < slot_of(&track, 2));
    }

    #[test]
    fn engaged_hurdle_stays_put_when_the_course_reshuffles() {
        let mut track = Track::new(vec![msg(1, "hello there", NOW - 600)], NOW);
        track.runner_at = FIRST_HURDLE_AT - 1.2; // mid-approach
        track.add_message(msg(2, "urgent: service is down", NOW), NOW);
        // The engaged hurdle didn't move; the critical queues behind it.
        assert!((slot_of(&track, 1) - FIRST_HURDLE_AT).abs() < 0.01);
        assert!(slot_of(&track, 2) > slot_of(&track, 1));
    }

    #[test]
    fn resolved_hurdles_freeze_in_place() {
        let mut track = Track::new(sample_messages(3), NOW);
        let cleared = track.next_hurdle().unwrap().message.id;
        track.resolve_next(MessageStatus::Cleared, NOW);
        let frozen_at = track
            .hurdles
            .iter()
            .find(|h| h.message.id == cleared)
            .unwrap()
            .at;
        track.relayout(NOW + 12 * 3600);
        track.tick(1.0, NOW + 12 * 3600);
        let after = track
            .hurdles
            .iter()
            .find(|h| h.message.id == cleared)
            .unwrap()
            .at;
        assert_eq!(frozen_at, after);
    }

    #[test]
    fn runner_target_never_overshoots_a_hurdle_that_crept_closer() {
        let mut track = Track::new(vec![msg(1, "hello there", NOW - 600)], NOW);
        track.runner_target = track.hurdles[0].at + 2.0; // stale, beyond the hurdle
        track.tick(0.016, NOW);
        assert!(track.runner_target <= track.hurdles[0].at - 1.2 + 1e-4);
        assert_eq!(track.next_hurdle().unwrap().message.id, 1);
    }

    #[test]
    fn tick_eases_hurdles_toward_their_slots() {
        let messages = vec![
            msg(1, "hello a", NOW - 60),
            msg(2, "hello b", NOW - 120),
        ];
        let mut track = Track::new(messages, NOW);
        // A critical reshuffles the course: hurdle 2 must fall back a slot.
        track.add_message(msg(3, "urgent: service is down", NOW), NOW);
        let h2 = |t: &Track| t.hurdles.iter().find(|h| h.message.id == 2).unwrap().at;
        let before = h2(&track);
        let slot = slot_of(&track, 2);
        assert!(slot > before);
        track.tick(0.1, NOW);
        let after = h2(&track);
        // Moved toward the slot, but only a creeping step, not a snap.
        assert!(after > before);
        assert!(after < slot);
        assert!(after - before <= 3.0 * 0.1 + 1e-4);
    }
}
