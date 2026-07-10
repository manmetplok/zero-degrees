//! Backlog boss battle (story 019): the open backlog rendered as a monster
//! whose health is the weighted priority of open messages. Pure battle state
//! (weighting, enrage, victory, respawn, attribution) is separated from the
//! drawing and unit tested.

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::{Message, MessageStatus};

use crate::dashboard::{self, FIRE};
use crate::screens;
use crate::team::{self, RunnerId};
use crate::view;

/// The boss enrages when this many hurdles are burning at once.
pub const ENRAGE_BURNING: usize = 4;

pub fn is_enraged(burning: usize) -> bool {
    burning >= ENRAGE_BURNING
}

/// Priority weight of one message: its urgency (1..=3) plus one when it is
/// burning. Boss health is the sum over the open queue.
pub fn msg_weight(msg: &Message, sim_now: i64) -> u32 {
    u32::from(dashboard::urgency_of(msg)) + u32::from(team::is_burning(msg, sim_now))
}

/// Total weighted priority of open (not yet cleared) messages.
pub fn open_weight<'a>(msgs: impl Iterator<Item = &'a Message>, sim_now: i64) -> u32 {
    msgs.filter(|m| m.status != MessageStatus::Cleared)
        .map(|m| msg_weight(m, sim_now))
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No backlog seen yet — the boss sleeps.
    Dormant,
    Alive,
    Victory,
}

/// An attributed hit, kept briefly for the floating hit animation.
pub struct Hit {
    pub runner: RunnerId,
    pub dmg: u32,
    pub at: f64,
}

pub struct Boss {
    pub phase: Phase,
    pub hp: u32,
    pub max_hp: u32,
    pub enraged: bool,
    pub bosses_defeated: u32,
    /// Damage per runner against the current boss (resets on respawn).
    pub current_dmg: HashMap<RunnerId, u32>,
    /// Damage per runner across all bosses (history survives respawns).
    pub total_dmg: HashMap<RunnerId, u32>,
    /// Final contribution stats of the last defeated boss, for the victory
    /// screen.
    pub last_battle: Vec<(RunnerId, u32)>,
    /// Recent hits for the floating animation.
    pub hits: Vec<Hit>,
    /// Real time the boss last grew (new arrival), for the flash ring.
    pub grew_at: f64,
}

impl Boss {
    pub fn new() -> Self {
        Self {
            phase: Phase::Dormant,
            hp: 0,
            max_hp: 0,
            enraged: false,
            bosses_defeated: 0,
            current_dmg: HashMap::new(),
            total_dmg: HashMap::new(),
            last_battle: Vec::new(),
            hits: Vec::new(),
            grew_at: f64::NEG_INFINITY,
        }
    }

    /// Credit a runner's clear as a hit on the boss.
    pub fn on_hit(&mut self, runner: RunnerId, dmg: u32, at: f64) {
        *self.current_dmg.entry(runner).or_insert(0) += dmg;
        *self.total_dmg.entry(runner).or_insert(0) += dmg;
        self.hits.push(Hit { runner, dmg, at });
        if self.hits.len() > 12 {
            self.hits.remove(0);
        }
    }

    /// Reconcile with the live queue once per frame. `weight` is the current
    /// open weighted priority; `burning` the number of burning hurdles.
    pub fn sync(&mut self, weight: u32, burning: usize, now: f64) {
        match self.phase {
            Phase::Dormant => {
                if weight > 0 {
                    self.phase = Phase::Alive;
                    self.hp = weight;
                    self.max_hp = weight;
                    self.enraged = is_enraged(burning);
                }
            }
            Phase::Alive => {
                if weight == 0 {
                    // Victory: the whole team gets the credit.
                    self.hp = 0;
                    self.phase = Phase::Victory;
                    self.bosses_defeated += 1;
                    self.enraged = false;
                    let mut battle: Vec<(RunnerId, u32)> =
                        self.current_dmg.iter().map(|(r, d)| (*r, *d)).collect();
                    battle.sort_by(|a, b| b.1.cmp(&a.1));
                    self.last_battle = battle;
                } else {
                    if weight > self.hp {
                        self.grew_at = now;
                    }
                    self.hp = weight;
                    self.max_hp = self.max_hp.max(weight);
                    self.enraged = is_enraged(burning);
                }
            }
            Phase::Victory => {
                if weight > 0 {
                    // Respawn, sized to the new backlog; history is kept.
                    self.phase = Phase::Alive;
                    self.current_dmg.clear();
                    self.hp = weight;
                    self.max_hp = weight;
                    self.enraged = is_enraged(burning);
                    self.grew_at = now;
                }
            }
        }
    }
}

// ---- drawing ----

/// Draw the boss screen below the header.
pub fn draw(boss: &Boss, w: f32, h: f32, now: f64) {
    match boss.phase {
        Phase::Dormant => {
            let fs = h * 0.026;
            draw_text("No backlog. The boss sleeps.", w * 0.1, h * 0.4, fs, view::INK_DIM);
        }
        Phase::Victory => draw_victory(boss, w, h, now),
        Phase::Alive => draw_alive(boss, w, h, now),
    }
}

fn draw_alive(boss: &Boss, w: f32, h: f32, now: f64) {
    let cx = w * 0.5;
    let cy = h * 0.34;
    let frac = if boss.max_hp > 0 {
        boss.hp as f32 / boss.max_hp as f32
    } else {
        0.0
    };
    let t = now as f32;
    let mut r = w * (0.13 + 0.15 * frac);
    r += (t * 2.1).sin() * r * 0.035; // idle breathing
    let (sx, sy) = if boss.enraged {
        ((t * 37.0).sin() * w * 0.008, (t * 29.0).cos() * w * 0.008)
    } else {
        (0.0, 0.0)
    };
    let (cx, cy) = (cx + sx, cy + sy);

    let body = if boss.enraged {
        Color::new(0.82, 0.20, 0.22, 1.0)
    } else {
        Color::new(0.45, 0.30, 0.75, 1.0)
    };
    let belly = if boss.enraged {
        Color::new(0.62, 0.13, 0.16, 1.0)
    } else {
        Color::new(0.34, 0.22, 0.58, 1.0)
    };

    // Enrage spikes around the body.
    if boss.enraged {
        for i in 0..10 {
            let a = t * 0.4 + i as f32 * std::f32::consts::TAU / 10.0;
            let tip = vec2(cx + a.cos() * r * 1.28, cy + a.sin() * r * 1.28);
            let b1 = vec2(cx + (a - 0.16).cos() * r, cy + (a - 0.16).sin() * r);
            let b2 = vec2(cx + (a + 0.16).cos() * r, cy + (a + 0.16).sin() * r);
            draw_triangle(tip, b1, b2, FIRE);
        }
    }

    draw_circle(cx, cy, r, body);
    draw_circle(cx, cy + r * 0.25, r * 0.72, belly);

    // Growth flash: expanding ring after a new arrival.
    let gt = ((now - boss.grew_at) / 0.6) as f32;
    if (0.0..1.0).contains(&gt) {
        let mut c = view::GOLD;
        c.a = 1.0 - gt;
        draw_circle_lines(cx, cy, r * (1.0 + gt * 0.5), 3.0, c);
    }

    // Eyes (angry brows when enraged).
    for side in [-1.0f32, 1.0] {
        let ex = cx + side * r * 0.38;
        let ey = cy - r * 0.25;
        draw_circle(ex, ey, r * 0.17, view::INK);
        let pupil_dy = if boss.enraged { r * 0.03 } else { (t * 1.3).sin() * r * 0.03 };
        draw_circle(ex, ey + pupil_dy, r * 0.08, view::SKY);
        if boss.enraged {
            // Angry brows: inner end low, outer end high.
            draw_line(
                ex - side * r * 0.2,
                ey - r * 0.14,
                ex + side * r * 0.3,
                ey - r * 0.32,
                r * 0.06,
                view::SKY,
            );
        }
    }
    // Mouth with teeth.
    let mw = r * 0.8;
    let my = cy + r * 0.38;
    view::rounded_rect(cx - mw / 2.0, my, mw, r * 0.22, r * 0.1, view::SKY);
    for i in 0..4 {
        let tx = cx - mw / 2.0 + mw * (0.14 + 0.24 * i as f32);
        draw_triangle(
            vec2(tx, my),
            vec2(tx + mw * 0.09, my),
            vec2(tx + mw * 0.045, my + r * 0.12),
            view::INK,
        );
    }

    // Floating attributed hits.
    for (i, hit) in boss.hits.iter().enumerate() {
        let ht = ((now - hit.at) / 1.2) as f32;
        if !(0.0..1.0).contains(&ht) {
            continue;
        }
        let off = ((hit.at * 7.3).sin() as f32) * w * 0.2;
        let fs = h * 0.028;
        let mut color = team::runner_color(hit.runner);
        color.a = 1.0 - ht * ht;
        draw_text(
            &format!("-{} {}", hit.dmg, team::runner_name(hit.runner)),
            cx + off - w * 0.06,
            cy - r - h * 0.02 - ht * h * 0.06 + (i as f32 * 0.0),
            fs,
            color,
        );
    }

    // Health bar.
    let pad = w * 0.1;
    let bar_y = h * 0.565;
    let bar_h = h * 0.026;
    let fs = h * 0.022;
    view::rounded_rect(pad, bar_y, w - 2.0 * pad, bar_h, bar_h * 0.5, view::TRACK_EDGE);
    let fill = if boss.enraged { FIRE } else { view::ACCENT };
    view::rounded_rect(pad, bar_y, (w - 2.0 * pad) * frac, bar_h, bar_h * 0.5, fill);
    draw_text(
        &format!("{} / {} priority", boss.hp, boss.max_hp),
        pad,
        bar_y + bar_h + fs * 1.3,
        fs,
        view::INK_DIM,
    );
    if boss.enraged {
        let blink = 0.6 + 0.4 * ((now * 6.0).sin() as f32).abs();
        let mut c = FIRE;
        c.a = blink;
        let label = "ENRAGED - swarm the queue!";
        let dims = measure_text(label, None, (fs * 1.25) as u16, 1.0);
        draw_text(label, (w - dims.width) / 2.0, bar_y + bar_h + fs * 2.9, fs * 1.25, c);
    }

    // Contribution to the current boss (below the enrage line).
    let mut y = h * 0.685;
    draw_text("DAMAGE THIS BOSS", pad * 0.5, y, fs, view::INK_DIM);
    y += fs * 0.9;
    let mut rows: Vec<(RunnerId, u32)> = boss.current_dmg.iter().map(|(r, d)| (*r, *d)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let max_dmg = rows.iter().map(|(_, d)| *d).max().unwrap_or(0).max(1) as f32;
    let rrow = h * 0.042;
    if rows.is_empty() {
        draw_text("No hits yet - clear a hurdle!", pad * 0.5, y + rrow * 0.7, fs, view::INK_DIM);
    }
    for (runner, dmg) in &rows {
        let color = team::runner_color(*runner);
        team::avatar(pad * 0.5 + rrow * 0.4, y + rrow * 0.45, rrow * 0.32, *runner);
        draw_text(team::runner_name(*runner), pad * 0.5 + rrow * 1.0, y + rrow * 0.58, fs, view::INK);
        let bar_x = w * 0.3;
        let bw = (w - pad * 0.5 - bar_x - fs * 2.4) * (*dmg as f32 / max_dmg);
        view::rounded_rect(bar_x, y + rrow * 0.22, bw.max(2.0), rrow * 0.42, rrow * 0.21, color);
        draw_text(&dmg.to_string(), w - pad * 0.5 - fs * 1.4, y + rrow * 0.58, fs, view::INK_DIM);
        y += rrow + h * 0.005;
    }

    draw_text(
        &format!("bosses defeated: {}", boss.bosses_defeated),
        pad * 0.5,
        h * 0.93,
        fs,
        view::INK_DIM,
    );
}

fn draw_victory(boss: &Boss, w: f32, h: f32, now: f64) {
    let pad = w * 0.08;
    let fs = h * 0.024;
    // Celebration rays.
    let cx = w * 0.5;
    let cy = h * 0.22;
    for i in 0..12 {
        let a = now as f32 * 0.5 + i as f32 * std::f32::consts::TAU / 12.0;
        let mut c = view::GOLD;
        c.a = 0.18;
        draw_triangle(
            vec2(cx, cy),
            vec2(cx + a.cos() * w * 0.7, cy + a.sin() * w * 0.7),
            vec2(cx + (a + 0.12).cos() * w * 0.7, cy + (a + 0.12).sin() * w * 0.7),
            c,
        );
    }
    let title = "BOSS DOWN!";
    let tfs = h * 0.056;
    let dims = measure_text(title, None, tfs as u16, 1.0);
    draw_text(title, (w - dims.width) / 2.0, cy + tfs * 0.3, tfs, view::GOLD);
    let sub = "Backlog cleared by the whole team";
    let sdims = measure_text(sub, None, fs as u16, 1.0);
    draw_text(sub, (w - sdims.width) / 2.0, cy + tfs * 1.1, fs, view::INK);

    // Per-runner contribution stats.
    let total: u32 = boss.last_battle.iter().map(|(_, d)| d).sum();
    let mut y = h * 0.36;
    draw_text("TEAM CONTRIBUTION", pad, y, fs, view::INK_DIM);
    y += fs * 1.2;
    let rrow = h * 0.06;
    let max_dmg = boss.last_battle.iter().map(|(_, d)| *d).max().unwrap_or(0).max(1) as f32;
    for (runner, dmg) in &boss.last_battle {
        let color = team::runner_color(*runner);
        team::avatar(pad + rrow * 0.4, y + rrow * 0.42, rrow * 0.3, *runner);
        let pct = if total > 0 { *dmg * 100 / total } else { 0 };
        draw_text(
            &format!("{}  ·  {} dmg  ·  {}%", team::runner_name(*runner), dmg, pct),
            pad + rrow * 1.0,
            y + rrow * 0.45,
            fs,
            view::INK,
        );
        let bw = (w - 2.0 * pad - rrow) * (*dmg as f32 / max_dmg);
        view::rounded_rect(pad + rrow, y + rrow * 0.6, bw.max(2.0), rrow * 0.22, rrow * 0.11, color);
        y += rrow + h * 0.008;
    }

    y = y.max(h * 0.72);
    draw_text(
        &format!(
            "bosses defeated: {}   ·   total damage: {}",
            boss.bosses_defeated,
            screens::fmt_xp(boss.total_dmg.values().map(|d| u64::from(*d)).sum())
        ),
        pad,
        y + fs,
        fs,
        view::INK_DIM,
    );
    draw_text(
        "New arrivals will respawn the boss - history is kept.",
        pad,
        y + fs * 2.4,
        fs * 0.9,
        view::INK_DIM,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Channel;

    fn msg(id: u64, subject: &str, received_at: i64, status: MessageStatus) -> Message {
        Message {
            id,
            channel: Channel::Email,
            sender: "t".into(),
            subject: subject.into(),
            body: String::new(),
            received_at,
            status,
        }
    }

    #[test]
    fn health_is_the_weighted_priority_of_open_messages() {
        let now = 100_000;
        let msgs = vec![
            msg(1, "Urgent: locked out asap", now, MessageStatus::Open), // urgency 3
            msg(2, "API 500 fail", now, MessageStatus::Open),            // urgency 2
            msg(3, "hello", now - 8_000, MessageStatus::Open),           // urgency 1 + burning
            msg(4, "Urgent: done", now, MessageStatus::Cleared),         // cleared: no weight
        ];
        assert_eq!(open_weight(msgs.iter(), now), 3 + 2 + 2);
    }

    #[test]
    fn hits_reduce_health_and_are_attributed() {
        let mut boss = Boss::new();
        boss.sync(6, 0, 0.0);
        assert_eq!(boss.phase, Phase::Alive);
        assert_eq!((boss.hp, boss.max_hp), (6, 6));
        boss.on_hit(RunnerId::Me, 2, 0.1);
        boss.sync(4, 0, 0.2);
        assert_eq!(boss.hp, 4);
        assert_eq!(boss.current_dmg[&RunnerId::Me], 2);
        assert_eq!(boss.hits.last().unwrap().runner, RunnerId::Me);
    }

    #[test]
    fn boss_grows_when_new_messages_arrive() {
        let mut boss = Boss::new();
        boss.sync(4, 0, 0.0);
        boss.sync(7, 0, 1.0);
        assert_eq!(boss.hp, 7);
        assert_eq!(boss.max_hp, 7);
        assert_eq!(boss.grew_at, 1.0);
    }

    #[test]
    fn enrage_tracks_the_burning_threshold() {
        assert!(!is_enraged(ENRAGE_BURNING - 1));
        assert!(is_enraged(ENRAGE_BURNING));
        let mut boss = Boss::new();
        boss.sync(5, ENRAGE_BURNING, 0.0);
        assert!(boss.enraged);
        boss.sync(5, ENRAGE_BURNING - 1, 1.0);
        assert!(!boss.enraged, "boss calms down when hurdles stop burning");
    }

    #[test]
    fn victory_snapshots_contributions_and_respawn_keeps_history() {
        let mut boss = Boss::new();
        boss.sync(5, 0, 0.0);
        boss.on_hit(RunnerId::Me, 3, 0.1);
        boss.on_hit(RunnerId::Bot(0), 2, 0.2);
        boss.sync(0, 0, 0.3);
        assert_eq!(boss.phase, Phase::Victory);
        assert_eq!(boss.bosses_defeated, 1);
        assert_eq!(boss.last_battle[0], (RunnerId::Me, 3), "biggest hitter first");

        // New arrivals respawn an appropriately-sized boss.
        boss.sync(2, 0, 1.0);
        assert_eq!(boss.phase, Phase::Alive);
        assert_eq!((boss.hp, boss.max_hp), (2, 2));
        assert!(boss.current_dmg.is_empty(), "fresh fight");
        assert_eq!(boss.total_dmg[&RunnerId::Me], 3, "history survives");
        assert_eq!(boss.bosses_defeated, 1);
    }

    #[test]
    fn dormant_boss_only_spawns_once_there_is_backlog() {
        let mut boss = Boss::new();
        boss.sync(0, 0, 0.0);
        assert_eq!(boss.phase, Phase::Dormant);
        boss.sync(3, 0, 1.0);
        assert_eq!(boss.phase, Phase::Alive);
    }
}
