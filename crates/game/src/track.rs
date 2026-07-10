//! Track model: pure game state, no rendering. One open message = one hurdle
//! placed along a 1-D track; the runner advances hurdle by hurdle toward the
//! finish line. Rendering maps track distance to screen space elsewhere.

use shared::{Message, MessageStatus};

/// Distance between consecutive hurdles, in track units (1 unit ≈ 1 meter).
pub const HURDLE_SPACING: f32 = 6.0;
/// Where the first hurdle stands, giving the runner a short lead-in.
pub const FIRST_HURDLE_AT: f32 = 4.0;
/// Extra track after the last hurdle before the finish line.
pub const FINISH_MARGIN: f32 = 5.0;

pub struct Hurdle {
    pub message: Message,
    /// Position along the track, in track units.
    pub at: f32,
}

pub struct Track {
    pub hurdles: Vec<Hurdle>,
    /// Runner position along the track, in track units.
    pub runner_at: f32,
    /// Where the runner is headed; `runner_at` eases toward this each frame.
    pub runner_target: f32,
    next_slot: f32,
}

impl Track {
    pub fn new(messages: Vec<Message>) -> Self {
        let mut next_slot = FIRST_HURDLE_AT;
        let hurdles = messages
            .into_iter()
            .map(|message| {
                let at = next_slot;
                next_slot += HURDLE_SPACING;
                Hurdle { message, at }
            })
            .collect();
        Self {
            hurdles,
            runner_at: 0.0,
            runner_target: 0.0,
            next_slot,
        }
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
        self.hurdles
            .iter()
            .filter(|h| h.message.status == MessageStatus::Open)
            .find(|h| h.at + 0.01 >= self.runner_target)
            .or_else(|| {
                // Skipped hurdles behind us may still be open; face the first of those.
                self.hurdles
                    .iter()
                    .find(|h| h.message.status == MessageStatus::Open)
            })
    }

    pub fn finish_at(&self) -> f32 {
        self.hurdles
            .iter()
            .map(|h| h.at)
            .fold(FIRST_HURDLE_AT, f32::max)
            + FINISH_MARGIN
    }

    /// Resolve the hurdle the runner is facing and move on to the next one.
    pub fn resolve_next(&mut self, status: MessageStatus) -> Option<u64> {
        let id = self.next_hurdle().map(|h| h.message.id)?;
        let hurdle = self
            .hurdles
            .iter_mut()
            .find(|h| h.message.id == id)
            .expect("id came from next_hurdle");
        hurdle.message.status = status;
        let at = hurdle.at;
        self.runner_target = match self.next_hurdle() {
            // Stop just short of the next hurdle; approach happens there.
            Some(next) => next.at - 1.5,
            None => self.finish_at(),
        }
        .max(at);
        Some(id)
    }

    /// A new message arrives mid-run: drop its hurdle onto the track ahead.
    pub fn add_message(&mut self, message: Message) -> &Hurdle {
        let at = self.next_slot;
        self.next_slot += HURDLE_SPACING;
        self.hurdles.push(Hurdle { message, at });
        self.hurdles.last().expect("just pushed")
    }

    /// Ease the runner toward its target. Returns true while moving.
    pub fn advance(&mut self, dt: f32) -> bool {
        let gap = self.runner_target - self.runner_at;
        if gap.abs() < 0.01 {
            self.runner_at = self.runner_target;
            return false;
        }
        // Constant running speed with a soft arrival.
        let speed = (gap.abs() * 2.5).clamp(1.0, 8.0);
        self.runner_at += gap.signum() * (speed * dt).min(gap.abs());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::sample_messages;

    #[test]
    fn clearing_advances_runner_and_decrements_remaining() {
        let mut track = Track::new(sample_messages(3));
        assert_eq!(track.remaining(), 3);
        let first = track.next_hurdle().unwrap().message.id;
        track.resolve_next(MessageStatus::Cleared);
        assert_eq!(track.remaining(), 2);
        assert_ne!(track.next_hurdle().unwrap().message.id, first);
        assert!(track.runner_target > 0.0);
    }

    #[test]
    fn skipped_hurdles_stay_open_and_get_revisited() {
        let mut track = Track::new(sample_messages(2));
        track.resolve_next(MessageStatus::Skipped);
        track.resolve_next(MessageStatus::Cleared);
        assert_eq!(track.remaining(), 0);
    }

    #[test]
    fn mid_run_message_lands_ahead_of_existing_hurdles() {
        let mut track = Track::new(sample_messages(2));
        let last_at = track.hurdles.last().unwrap().at;
        let msg = crate::inbox::next_incoming(
            &track.hurdles.iter().map(|h| h.message.clone()).collect::<Vec<_>>(),
        );
        let hurdle = track.add_message(msg);
        assert!(hurdle.at > last_at);
    }

    #[test]
    fn finishing_all_hurdles_targets_finish_line() {
        let mut track = Track::new(sample_messages(1));
        track.resolve_next(MessageStatus::Cleared);
        assert_eq!(track.runner_target, track.finish_at());
    }
}
