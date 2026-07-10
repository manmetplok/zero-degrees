//! League leaderboard (story 017): period math and rank logic (pure, unit
//! tested) plus the leaderboard screen with animated rank swaps and the
//! team-total banner that keeps the framing collaborative.

use std::collections::HashMap;

use macroquad::prelude::*;

use crate::screens;
use crate::team::{self, BotSpec, RunnerId};
use crate::view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    Week,
    AllTime,
}

impl Period {
    pub const ALL: [Period; 3] = [Period::Today, Period::Week, Period::AllTime];

    pub fn label(self) -> &'static str {
        match self {
            Period::Today => "today",
            Period::Week => "this week",
            Period::AllTime => "all time",
        }
    }
}

/// One leaderboard row before ranking.
pub struct Entry {
    pub runner: RunnerId,
    pub xp: u64,
    pub streak_days: u32,
    pub badges: u32,
    /// Negative = no data yet ("—").
    pub avg_resp_secs: i64,
}

/// A bot's XP for a period: its baseline when the session started plus the
/// clears it landed live (live clears count toward every period).
pub fn bot_xp(spec: &BotSpec, landed_clears: u32, period: Period) -> u64 {
    let base = match period {
        Period::Today => spec.xp_today,
        Period::Week => spec.xp_week,
        Period::AllTime => spec.xp_all,
    };
    base + u64::from(landed_clears) * u64::from(spec.xp_per_clear)
}

/// Rank entries by XP, highest first. The sort is stable so ties keep the
/// caller's order.
pub fn standings(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by(|a, b| b.xp.cmp(&a.xp));
    entries
}

/// Runners whose rank differs between two orderings. Empty when the shapes
/// don't match (e.g. first frame with no previous order).
pub fn swapped(prev: &[RunnerId], cur: &[RunnerId]) -> Vec<RunnerId> {
    if prev.len() != cur.len() {
        return Vec::new();
    }
    cur.iter()
        .enumerate()
        .filter(|(i, r)| prev.get(*i) != Some(*r))
        .map(|(_, r)| *r)
        .collect()
}

/// Team-over-individual banner data.
pub struct Banner {
    pub team_xp_today: u64,
    pub team_cleared: u32,
    pub incoming: u32,
}

/// Leaderboard screen state: selected period plus the animation bookkeeping
/// for rank swaps.
pub struct Board {
    pub period: Period,
    /// Animated row slot per runner (0.0 = first place row).
    slot: HashMap<RunnerId, f32>,
    prev_order: Vec<RunnerId>,
    /// Real time of each runner's latest rank change, for the flash.
    flash: HashMap<RunnerId, f64>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            period: Period::Today,
            slot: HashMap::new(),
            prev_order: Vec::new(),
            flash: HashMap::new(),
        }
    }

    /// Draw the leaderboard below the header and handle taps on the period
    /// tabs. `rows` must already be ranked for `self.period`.
    pub fn frame(
        &mut self,
        tap: Option<Vec2>,
        dt: f32,
        now: f64,
        w: f32,
        h: f32,
        banner: &Banner,
        rows: Vec<Entry>,
    ) {
        let pad = w * 0.05;
        let fs = h * 0.022;

        // Team total above individuals (story: team over individual).
        let by = h * 0.115;
        let bh = h * 0.1;
        view::rounded_rect(pad, by, w - 2.0 * pad, bh, w * 0.02, view::PANEL);
        draw_rectangle(pad, by, w * 0.012, bh, view::GOLD);
        draw_text(
            &format!("Team today: {} XP", screens::fmt_xp(banner.team_xp_today)),
            pad + fs * 1.4,
            by + fs * 1.9,
            fs * 1.35,
            view::INK,
        );
        draw_text(
            &format!("{} of {} incoming cleared", banner.team_cleared, banner.incoming),
            pad + fs * 1.4,
            by + fs * 3.4,
            fs,
            view::INK_DIM,
        );
        let bar_y = by + bh - fs * 1.1;
        let bar_w = w - 2.0 * pad - fs * 2.8;
        let frac = if banner.incoming > 0 {
            (banner.team_cleared as f32 / banner.incoming as f32).min(1.0)
        } else {
            0.0
        };
        view::rounded_rect(pad + fs * 1.4, bar_y, bar_w, fs * 0.5, fs * 0.25, view::TRACK_EDGE);
        view::rounded_rect(pad + fs * 1.4, bar_y, bar_w * frac, fs * 0.5, fs * 0.25, view::GOLD);

        // Period tabs.
        let ty = by + bh + h * 0.02;
        let th = h * 0.042;
        let tw = (w - 2.0 * pad - w * 0.02 * 2.0) / 3.0;
        for (i, period) in Period::ALL.iter().enumerate() {
            let r = Rect::new(pad + i as f32 * (tw + w * 0.02), ty, tw, th);
            if tap.map_or(false, |p| r.contains(p)) && self.period != *period {
                self.period = *period;
                // New period, new ranking: don't flash the reshuffle.
                self.prev_order.clear();
                self.flash.clear();
            }
            let selected = self.period == *period;
            view::rounded_rect(r.x, r.y, r.w, r.h, th * 0.5, view::PANEL);
            if selected {
                view::rounded_rect(r.x, r.y + r.h - th * 0.14, r.w, th * 0.14, th * 0.07, view::GOLD);
            }
            let dims = measure_text(period.label(), None, fs as u16, 1.0);
            draw_text(
                period.label(),
                r.x + (r.w - dims.width) / 2.0,
                r.y + th * 0.65,
                fs,
                if selected { view::GOLD } else { view::INK_DIM },
            );
        }

        // Rank-change detection drives the swap animation.
        let order: Vec<RunnerId> = rows.iter().map(|e| e.runner).collect();
        if order != self.prev_order {
            for r in swapped(&self.prev_order, &order) {
                self.flash.insert(r, now);
            }
            self.prev_order = order;
        }

        // Rows, each easing toward its rank slot.
        let rows_y = ty + th + h * 0.022;
        let slot_h = h * 0.105;
        let ease = 1.0 - (-9.0 * dt).exp();
        for (i, e) in rows.iter().enumerate() {
            let target = i as f32;
            let s = self.slot.entry(e.runner).or_insert(target);
            *s += (target - *s) * ease;
            let y = rows_y + *s * slot_h;
            let r = Rect::new(pad, y, w - 2.0 * pad, slot_h - h * 0.012);

            let flashing = self
                .flash
                .get(&e.runner)
                .map_or(false, |at| now - at < 0.9);
            view::rounded_rect(r.x, r.y, r.w, r.h, w * 0.02, view::PANEL);
            if flashing {
                let mut c = view::GOLD;
                c.a = 0.25;
                view::rounded_rect(r.x, r.y, r.w, r.h, w * 0.02, c);
            }

            let rank_color = if i == 0 { view::GOLD } else { view::INK_DIM };
            draw_text(&format!("{}", i + 1), r.x + fs * 0.9, r.y + r.h * 0.62, fs * 1.6, rank_color);
            team::avatar(r.x + fs * 3.2, r.y + r.h * 0.5, r.h * 0.26, e.runner);
            draw_text(
                team::runner_name(e.runner),
                r.x + fs * 4.6,
                r.y + r.h * 0.45,
                fs * 1.15,
                view::INK,
            );
            draw_text(
                &format!(
                    "streak {}d  ·  {} badges  ·  avg {}",
                    e.streak_days,
                    e.badges,
                    screens::fmt_dur(e.avg_resp_secs)
                ),
                r.x + fs * 4.6,
                r.y + r.h * 0.8,
                fs * 0.85,
                view::INK_DIM,
            );
            let xp_text = format!("{} XP", screens::fmt_xp(e.xp));
            let dims = measure_text(&xp_text, None, (fs * 1.2) as u16, 1.0);
            draw_text(&xp_text, r.x + r.w - dims.width - fs, r.y + r.h * 0.6, fs * 1.2, view::GOLD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(runner: RunnerId, xp: u64) -> Entry {
        Entry {
            runner,
            xp,
            streak_days: 0,
            badges: 0,
            avg_resp_secs: -1,
        }
    }

    #[test]
    fn standings_rank_by_xp_descending_and_ties_stay_stable() {
        let ranked = standings(vec![
            entry(RunnerId::Me, 100),
            entry(RunnerId::Bot(0), 300),
            entry(RunnerId::Bot(1), 100),
        ]);
        let order: Vec<RunnerId> = ranked.iter().map(|e| e.runner).collect();
        assert_eq!(order, vec![RunnerId::Bot(0), RunnerId::Me, RunnerId::Bot(1)]);
    }

    #[test]
    fn period_switch_recomputes_from_that_periods_baseline() {
        let spec = &team::BOTS[0];
        assert_eq!(bot_xp(spec, 0, Period::Today), spec.xp_today);
        assert_eq!(bot_xp(spec, 0, Period::Week), spec.xp_week);
        assert_eq!(bot_xp(spec, 0, Period::AllTime), spec.xp_all);
        // Live clears count toward every period.
        let live = 2 * u64::from(spec.xp_per_clear);
        assert_eq!(bot_xp(spec, 2, Period::Today), spec.xp_today + live);
        assert_eq!(bot_xp(spec, 2, Period::AllTime), spec.xp_all + live);
    }

    #[test]
    fn rank_swaps_are_detected_for_every_runner_that_moved() {
        let prev = vec![RunnerId::Bot(0), RunnerId::Me, RunnerId::Bot(1)];
        let cur = vec![RunnerId::Me, RunnerId::Bot(0), RunnerId::Bot(1)];
        let moved = swapped(&prev, &cur);
        assert!(moved.contains(&RunnerId::Me));
        assert!(moved.contains(&RunnerId::Bot(0)));
        assert!(!moved.contains(&RunnerId::Bot(1)));
    }

    #[test]
    fn no_swaps_without_a_previous_order() {
        assert!(swapped(&[], &[RunnerId::Me]).is_empty());
    }
}
