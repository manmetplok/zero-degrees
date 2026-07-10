//! Player profile: owns the persisted progression (daily run, streak,
//! trophies) and draws its UI — the streak-flame pill on the home screen,
//! the trophy room overlay, goal/streak banners, and the full-screen trophy
//! celebration. Pure rules live in progress.rs / trophies.rs; this module
//! is the glue and the pixels. Layout is resolution-independent and drawn
//! from primitives only, matching view.rs.

use std::collections::VecDeque;
use std::path::PathBuf;

use macroquad::prelude::*;

use crate::progress::{ClearEvent, Day, DayEvent, GOAL_CLEARS, GOAL_XP};
use crate::save::{self, SaveData};
use crate::trophies::{Tier, TrophyAward, TrophyId};
use crate::view;

/// Seconds a full-screen trophy celebration stays before auto-advancing.
const CELEBRATION_SECS: f64 = 4.0;
/// Seconds a goal/streak banner stays on screen.
const BANNER_SECS: f64 = 3.0;

const FLAME: Color = Color::new(1.0, 0.55, 0.18, 1.0);
const FLAME_DIM: Color = Color::new(0.45, 0.45, 0.58, 1.0);
const SHIELD_BLUE: Color = Color::new(0.35, 0.75, 1.0, 1.0);
const BRONZE: Color = Color::new(0.80, 0.52, 0.28, 1.0);
const SILVER: Color = Color::new(0.78, 0.81, 0.88, 1.0);
const OVERLAY: Color = Color::new(0.02, 0.02, 0.08, 0.78);

fn tier_color(tier: Tier) -> Color {
    match tier {
        Tier::Bronze => BRONZE,
        Tier::Silver => SILVER,
        Tier::Gold => view::GOLD,
    }
}

struct Banner {
    text: String,
    at: f64,
}

pub struct Profile {
    pub data: SaveData,
    path: PathBuf,
    room_open: bool,
    /// Trophy tiers earned but not yet celebrated; front is on screen.
    celebrations: VecDeque<TrophyAward>,
    celebration_since: f64,
    banner: Option<Banner>,
}

impl Profile {
    /// Load the save (or start fresh at `today`); rollover for days passed
    /// while the app was closed settles on the first `tick`.
    pub fn load(today: Day) -> Self {
        let path = save::default_path();
        let data = save::load(&path).unwrap_or_else(|| SaveData::new(today));
        Self {
            data,
            path,
            room_open: false,
            celebrations: VecDeque::new(),
            celebration_since: 0.0,
            banner: None,
        }
    }

    fn store(&self) {
        if let Err(e) = save::store(&self.path, &self.data) {
            eprintln!("save failed ({}): {e}", self.path.display());
        }
    }

    fn on_day_event(&mut self, ev: &DayEvent, now: f64) {
        let text = match ev {
            DayEvent::GoalMet {
                streak,
                shield_earned: false,
            } => format!("Daily goal met!  Streak {streak}"),
            DayEvent::GoalMet {
                streak,
                shield_earned: true,
            } => format!("Daily goal met!  Streak {streak}  ·  shield earned"),
            DayEvent::ShieldUsed { streak } => {
                format!("Streak shield used — streak {streak} is safe")
            }
            DayEvent::StreakBroken { lost, best } => {
                format!("Streak ended at {lost}  ·  best {best}")
            }
        };
        self.banner = Some(Banner { text, at: now });
    }

    /// Once per frame: settle day rollovers, expire banners, advance the
    /// celebration queue. `today` comes from the world clock, `now` from the
    /// frame clock (same convention as score.rs).
    pub fn tick(&mut self, today: Day, now: f64) {
        if let Some(ev) = self.data.daily.tick(today) {
            self.on_day_event(&ev, now);
            self.store();
        }
        if let Some(b) = &self.banner {
            if now - b.at > BANNER_SECS {
                self.banner = None;
            }
        }
        if !self.celebrations.is_empty() && now - self.celebration_since > CELEBRATION_SECS {
            self.advance_celebration(now);
        }
    }

    /// Feed one cleared hurdle into streaks and trophies, then persist.
    pub fn on_clear(&mut self, ev: &ClearEvent, xp: u32, now: f64) {
        let today = Day::from_unix(ev.at);
        for day_ev in self.data.daily.on_clear(xp, today) {
            self.on_day_event(&day_ev, now);
        }
        for award in self.data.trophies.on_clear(ev) {
            if self.celebrations.is_empty() {
                self.celebration_since = now;
            }
            self.celebrations.push_back(award);
        }
        self.store();
    }

    /// Is a full-screen view (trophy room or celebration) swallowing input?
    pub fn overlay_active(&self) -> bool {
        self.room_open || !self.celebrations.is_empty()
    }

    /// A tap/keypress while an overlay is up: dismiss the celebration first,
    /// then the trophy room.
    pub fn dismiss(&mut self, now: f64) {
        if !self.celebrations.is_empty() {
            self.advance_celebration(now);
        } else {
            self.room_open = false;
        }
    }

    fn advance_celebration(&mut self, now: f64) {
        self.celebrations.pop_front();
        self.celebration_since = now;
    }

    pub fn toggle_room(&mut self) {
        self.room_open = !self.room_open;
    }

    /// A tap on the home screen; returns true when the profile UI took it.
    pub fn handle_tap(&mut self, pos: Vec2, w: f32, h: f32) -> bool {
        if pill_rect(w, h).contains(pos) {
            self.toggle_room();
            return true;
        }
        false
    }

    /// Demo-harness hook: show a celebration without touching real counts.
    pub fn demo_celebrate(&mut self, now: f64) {
        if self.celebrations.is_empty() {
            self.celebration_since = now;
        }
        self.celebrations.push_back(TrophyAward {
            id: TrophyId::Firefighter,
            tier: Tier::Bronze,
        });
    }

    // ---- drawing ----

    /// Streak flame pill on the home screen (also the profile entry point).
    pub fn draw_hud(&self, w: f32, h: f32, now: f64) {
        let r = pill_rect(w, h);
        view::rounded_rect(r.x, r.y, r.w, r.h, r.h / 2.0, view::PANEL);
        let d = &self.data.daily;
        // Flame pulses while today's goal is met.
        let pulse = if d.goal_met {
            1.0 + 0.08 * (now * 6.0).sin() as f32
        } else {
            1.0
        };
        flame(
            r.x + r.h * 0.62,
            r.y + r.h * 0.78,
            r.h * 0.62 * pulse,
            d.streak > 0 || d.goal_met,
        );
        let fs = r.h * 0.62;
        draw_text(
            &format!("{}", d.streak),
            r.x + r.h * 1.15,
            r.y + r.h * 0.70,
            fs,
            view::INK,
        );
        if d.shields > 0 {
            shield(r.x + r.w - r.h * 0.58, r.y + r.h * 0.5, r.h * 0.46);
        }
        // Thin bar along the bottom: today's goal progress.
        let bar_h = r.h * 0.10;
        let inset = r.h * 0.30;
        let bw = r.w - 2.0 * inset;
        view::rounded_rect(r.x + inset, r.y + r.h - bar_h * 1.9, bw, bar_h, bar_h / 2.0, view::TRACK_EDGE);
        view::rounded_rect(
            r.x + inset,
            r.y + r.h - bar_h * 1.9,
            bw * d.goal_progress(),
            bar_h,
            bar_h / 2.0,
            if d.goal_met { view::GOLD } else { FLAME },
        );
    }

    /// Banners, trophy room, celebration — call last so they sit on top.
    pub fn draw_overlays(&self, w: f32, h: f32, now: f64) {
        if let Some(b) = &self.banner {
            draw_banner(w, h, &b.text, ((now - b.at) / BANNER_SECS) as f32);
        }
        if self.room_open {
            self.draw_trophy_room(w, h);
        }
        if let Some(award) = self.celebrations.front() {
            draw_celebration(w, h, award, (now - self.celebration_since) as f32);
        }
    }

    fn draw_trophy_room(&self, w: f32, h: f32) {
        draw_rectangle(0.0, 0.0, w, h, OVERLAY);
        let pad = w * 0.06;
        let panel = Rect::new(pad, h * 0.07, w - 2.0 * pad, h * 0.86);
        view::rounded_rect(panel.x, panel.y, panel.w, panel.h, w * 0.04, view::PANEL);

        let fs = h * 0.030;
        let x = panel.x + w * 0.05;
        let mut y = panel.y + fs * 2.2;
        draw_text("Trophy Room", x, y, fs * 1.5, view::GOLD);

        // Streak summary block.
        let d = &self.data.daily;
        y += fs * 2.4;
        flame(x + fs * 0.6, y + fs * 0.25, fs * 1.25, d.streak > 0 || d.goal_met);
        draw_text(
            &format!("{} day streak  ·  best {}", d.streak, d.best_streak),
            x + fs * 1.8,
            y,
            fs,
            view::INK,
        );
        if d.shields > 0 {
            shield(x + fs * 1.8 + measure_text(
                &format!("{} day streak  ·  best {}", d.streak, d.best_streak),
                None,
                fs as u16,
                1.0,
            ).width + fs * 0.9, y - fs * 0.32, fs * 0.9);
        }
        y += fs * 1.6;
        let (goal_line, goal_ink) = if d.goal_met {
            (
                format!("today: goal met!  ·  {} clears  ·  {} XP", d.clears_today, d.xp_today),
                view::GOLD,
            )
        } else {
            (
                format!(
                    "today: {}/{} clears  ·  {}/{} XP",
                    d.clears_today, GOAL_CLEARS, d.xp_today, GOAL_XP
                ),
                view::INK_DIM,
            )
        };
        draw_text(&goal_line, x, y, fs * 0.85, goal_ink);

        // Trophy list.
        y += fs * 1.2;
        let row_h = (panel.y + panel.h - y - fs * 2.2) / TrophyId::ALL.len() as f32;
        for id in TrophyId::ALL {
            self.draw_trophy_row(id, panel.x, y, panel.w, row_h, fs);
            y += row_h;
        }
        let hint = "tap anywhere to close";
        let dims = measure_text(hint, None, (fs * 0.8) as u16, 1.0);
        draw_text(
            hint,
            panel.x + (panel.w - dims.width) / 2.0,
            panel.y + panel.h - fs * 0.9,
            fs * 0.8,
            view::INK_DIM,
        );
    }

    fn draw_trophy_row(&self, id: TrophyId, px: f32, y: f32, pw: f32, row_h: f32, fs: f32) {
        let case = &self.data.trophies;
        let tier = case.tier(id);
        let cx = px + pw * 0.11;
        let cy = y + row_h * 0.48;
        let r = (row_h * 0.30).min(pw * 0.07);
        // Medal disc with the cup glyph; dim until bronze is earned.
        let (disc, glyph) = match tier {
            Some(t) => (tier_color(t), view::PANEL),
            None => (view::TRACK_EDGE, view::INK_DIM),
        };
        draw_circle(cx, cy, r, disc);
        trophy_cup(cx, cy, r * 1.15, glyph);

        let tx = px + pw * 0.20;
        let right = px + pw * 0.94;
        draw_text(id.label(), tx, cy - fs * 0.55, fs, view::INK);
        draw_text(id.describe(), tx, cy + fs * 0.35, fs * 0.72, view::INK_DIM);

        // Right column: tier label and count, clear of the text on the left.
        let tier_label = tier.map_or("Locked", Tier::label);
        let dims = measure_text(tier_label, None, (fs * 0.8) as u16, 1.0);
        draw_text(
            tier_label,
            right - dims.width,
            cy - fs * 0.55,
            fs * 0.8,
            tier.map_or(view::INK_DIM, tier_color),
        );
        if let Some((have, need)) = case.next_tier_progress(id) {
            let count = format!("{have}/{need}");
            let cd = measure_text(&count, None, (fs * 0.7) as u16, 1.0);
            draw_text(&count, right - cd.width, cy + fs * 0.35, fs * 0.7, view::INK_DIM);
            // Progress toward the next tier along the bottom of the row.
            let bh = fs * 0.24;
            let by = cy + fs * 0.85;
            view::rounded_rect(tx, by, right - tx, bh, bh / 2.0, view::TRACK_EDGE);
            view::rounded_rect(
                tx,
                by,
                (right - tx) * (have as f32 / need as f32).min(1.0),
                bh,
                bh / 2.0,
                view::GOLD,
            );
        }
    }
}

/// Where the streak pill sits: top-left, under the hurdles/XP HUD block.
pub fn pill_rect(w: f32, h: f32) -> Rect {
    Rect::new(w * 0.04, h * 0.135, w * 0.24, h * 0.048)
}

/// Goal/streak banner: fades out over its lifetime `t` in [0, 1].
fn draw_banner(w: f32, h: f32, text: &str, t: f32) {
    // Shrink to fit long messages inside the portrait width.
    let mut fs = h * 0.030;
    let mut dims = measure_text(text, None, fs as u16, 1.0);
    if dims.width > w * 0.84 {
        fs *= w * 0.84 / dims.width;
        dims = measure_text(text, None, fs as u16, 1.0);
    }
    let bw = dims.width + fs * 2.0;
    let bh = fs * 2.0;
    let x = (w - bw) / 2.0;
    let y = h * 0.24 - fs * (t * t) * 1.5; // drifts up as it fades
    let fade = (1.0 - t * t).clamp(0.0, 1.0);
    let mut panel = view::PANEL;
    panel.a = 0.92 * fade;
    let mut ink = view::GOLD;
    ink.a = fade;
    view::rounded_rect(x, y, bw, bh, bh / 2.0, panel);
    draw_text(text, x + fs, y + fs * 1.35, fs, ink);
}

/// Full-screen trophy celebration; `t` is seconds since it appeared.
fn draw_celebration(w: f32, h: f32, award: &TrophyAward, t: f32) {
    draw_rectangle(0.0, 0.0, w, h, OVERLAY);
    let cx = w / 2.0;
    let cy = h * 0.40;
    // Pop-in with a slight overshoot, then a gentle idle wobble.
    let grow = (t * 3.5).min(1.0);
    let scale = grow * (1.0 + 0.25 * (1.0 - grow)) + 0.02 * (t * 3.0).sin();
    let color = tier_color(award.tier);

    // Radial rays behind the cup.
    let rays = 12;
    let mut ray = color;
    ray.a = 0.16;
    for i in 0..rays {
        let a = t * 0.35 + i as f32 * std::f32::consts::TAU / rays as f32;
        let (r1, r2) = (h * 0.055, h * 0.30 * scale);
        let spread = 0.16;
        draw_triangle(
            vec2(cx + r1 * a.cos(), cy + r1 * a.sin()),
            vec2(cx + r2 * (a - spread).cos(), cy + r2 * (a - spread).sin()),
            vec2(cx + r2 * (a + spread).cos(), cy + r2 * (a + spread).sin()),
            ray,
        );
    }
    trophy_cup(cx, cy, h * 0.16 * scale, color);

    let fs = h * 0.030;
    let title = award.id.label();
    let dims = measure_text(title, None, (fs * 1.8) as u16, 1.0);
    draw_text(title, cx - dims.width / 2.0, h * 0.58, fs * 1.8, view::INK);
    let sub = format!("{} trophy earned!", award.tier.label());
    let dims = measure_text(&sub, None, (fs * 1.1) as u16, 1.0);
    draw_text(&sub, cx - dims.width / 2.0, h * 0.63, fs * 1.1, color);
    let hint = "tap to continue";
    let dims = measure_text(hint, None, (fs * 0.85) as u16, 1.0);
    draw_text(hint, cx - dims.width / 2.0, h * 0.72, fs * 0.85, view::INK_DIM);
}

/// Streak flame: teardrop from a triangle over a circle, with a gold core.
/// `s` is the flame height; its base center sits at (cx, base_y).
fn flame(cx: f32, base_y: f32, s: f32, lit: bool) {
    let outer = if lit { FLAME } else { FLAME_DIM };
    let r = s * 0.34;
    let by = base_y - r;
    draw_circle(cx, by, r, outer);
    draw_triangle(
        vec2(cx - r, by - r * 0.10),
        vec2(cx + r, by - r * 0.10),
        vec2(cx + s * 0.08, base_y - s),
        outer,
    );
    // Inner core: gold when lit, hollow-dark when the streak is cold, so the
    // silhouette still reads as a flame rather than a droplet.
    let core = if lit { view::GOLD } else { view::PANEL };
    let cr = r * 0.55;
    let cy2 = by + r * 0.25;
    draw_circle(cx, cy2, cr, core);
    draw_triangle(
        vec2(cx - cr, cy2 - cr * 0.2),
        vec2(cx + cr, cy2 - cr * 0.2),
        vec2(cx, cy2 - s * 0.45),
        core,
    );
}

/// Streak-shield token: a small kite shape in cool blue.
fn shield(cx: f32, cy: f32, s: f32) {
    let w2 = s * 0.42;
    let top = cy - s * 0.5;
    draw_rectangle(cx - w2, top, w2 * 2.0, s * 0.55, SHIELD_BLUE);
    draw_triangle(
        vec2(cx - w2, top + s * 0.55),
        vec2(cx + w2, top + s * 0.55),
        vec2(cx, cy + s * 0.5),
        SHIELD_BLUE,
    );
    draw_circle(cx, top + s * 0.42, s * 0.16, view::PANEL);
}

/// Trophy cup drawn from primitives; `s` is roughly the cup height and the
/// cup is centered on (cx, cy).
fn trophy_cup(cx: f32, cy: f32, s: f32, color: Color) {
    let bw = s * 0.62;
    let top = cy - s * 0.48;
    // Bowl: rounded slab with a round belly.
    view::rounded_rect(cx - bw / 2.0, top, bw, s * 0.30, s * 0.06, color);
    draw_circle(cx, top + s * 0.28, bw * 0.42, color);
    // Handles.
    let t = (s * 0.07).max(1.5);
    draw_circle_lines(cx - bw * 0.62, top + s * 0.16, s * 0.14, t, color);
    draw_circle_lines(cx + bw * 0.62, top + s * 0.16, s * 0.14, t, color);
    // Stem and base.
    draw_rectangle(cx - s * 0.05, cy + s * 0.10, s * 0.10, s * 0.20, color);
    view::rounded_rect(cx - s * 0.20, cy + s * 0.28, s * 0.40, s * 0.12, s * 0.04, color);
}
