//! Screen navigation and shared UI chrome for the team screens (stories
//! 011/013/017/019): the `Screen` enum, the always-reachable one-thumb nav
//! button + menu, toast notifications, the filter requests the track view
//! consumes, and small text-format helpers. Pure rules (filter matching,
//! formatting) are separated from drawing and unit tested.

use macroquad::prelude::*;
use shared::{Channel, Message};

use crate::dashboard::{self, Category, Mood};
use crate::team::{self, RunnerId};
use crate::view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Track,
    RaceControl,
    Leaderboard,
    Boss,
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Track => "Track",
            Screen::RaceControl => "Race control",
            Screen::Leaderboard => "League",
            Screen::Boss => "Boss",
        }
    }
}

/// A request to show the track filtered to a subset of hurdles. Race control
/// segments and the "my lane" toggle emit these. Today the track dims
/// non-matching hurdles; story 006's filter overlay can consume the same
/// requests unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterRequest {
    Channel(Channel),
    Category(Category),
    Mood(Mood),
    Burning,
    Assignee(RunnerId),
}

impl FilterRequest {
    pub fn label(self) -> String {
        match self {
            FilterRequest::Channel(c) => c.label().to_lowercase(),
            FilterRequest::Category(c) => c.label().to_string(),
            FilterRequest::Mood(m) => format!("{} mood", m.label()),
            FilterRequest::Burning => "burning".to_string(),
            FilterRequest::Assignee(RunnerId::Me) => "my lane".to_string(),
            FilterRequest::Assignee(r) => format!("{}'s lane", team::runner_name(r)),
        }
    }

    pub fn matches(self, msg: &Message, assignee: Option<RunnerId>, sim_now: i64) -> bool {
        match self {
            FilterRequest::Channel(c) => msg.channel == c,
            FilterRequest::Category(c) => dashboard::category_of(msg) == c,
            FilterRequest::Mood(m) => dashboard::mood_of(msg) == m,
            FilterRequest::Burning => team::is_burning(msg, sim_now),
            FilterRequest::Assignee(r) => assignee == Some(r),
        }
    }
}

// ---- nav button + menu ----

pub enum NavAction {
    Goto(Screen),
    ToggleMyLane,
}

pub enum NavResponse {
    /// Tap was not on the nav; the caller may use it.
    Pass,
    /// Tap was swallowed by the nav (button toggle or scrim close).
    Consumed,
    Action(NavAction),
}

const NAV_SCREENS: [Screen; 4] = [
    Screen::Track,
    Screen::RaceControl,
    Screen::Leaderboard,
    Screen::Boss,
];

/// Corner nav: a round button in the bottom-right (thumb range) that fans a
/// vertical menu of screens plus the "my lane" toggle.
pub struct Nav {
    pub open: bool,
}

impl Nav {
    pub fn new() -> Self {
        // Dev harness: ZD_NAV=1 opens the menu at launch, for screenshots.
        Self {
            open: std::env::var("ZD_NAV").is_ok(),
        }
    }

    fn button_rect(w: f32, h: f32) -> Rect {
        let d = w * 0.125;
        Rect::new(w - d - w * 0.03, h - d - w * 0.03, d, d)
    }

    /// Menu row `i`, stacked upward from the button (0 = nearest).
    fn entry_rect(w: f32, h: f32, i: usize) -> Rect {
        let bw = w * 0.46;
        let bh = h * 0.052;
        let gap = h * 0.011;
        let btn = Self::button_rect(w, h);
        Rect::new(
            w - bw - w * 0.03,
            btn.y - (i as f32 + 1.0) * (bh + gap),
            bw,
            bh,
        )
    }

    pub fn handle_tap(&mut self, pos: Vec2, w: f32, h: f32) -> NavResponse {
        if Self::button_rect(w, h).contains(pos) {
            self.open = !self.open;
            return NavResponse::Consumed;
        }
        if !self.open {
            return NavResponse::Pass;
        }
        for (i, screen) in NAV_SCREENS.iter().enumerate() {
            if Self::entry_rect(w, h, i).contains(pos) {
                self.open = false;
                return NavResponse::Action(NavAction::Goto(*screen));
            }
        }
        if Self::entry_rect(w, h, NAV_SCREENS.len()).contains(pos) {
            self.open = false;
            return NavResponse::Action(NavAction::ToggleMyLane);
        }
        // Anywhere else closes the menu and swallows the tap.
        self.open = false;
        NavResponse::Consumed
    }

    pub fn draw(&self, w: f32, h: f32, current: Screen, my_lane: bool) {
        if self.open {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.05, 0.55));
            let fs = h * 0.024;
            let highlight = |on: bool| if on { view::GOLD } else { view::INK };
            for (i, screen) in NAV_SCREENS.iter().enumerate() {
                let r = Self::entry_rect(w, h, i);
                view::rounded_rect(r.x, r.y, r.w, r.h, r.h * 0.5, view::PANEL);
                if *screen == current {
                    draw_circle(r.x + r.h * 0.5, r.y + r.h * 0.5, r.h * 0.14, view::ACCENT);
                }
                draw_text(screen.label(), r.x + r.h * 0.9, r.y + r.h * 0.66, fs, highlight(*screen == current));
            }
            let r = Self::entry_rect(w, h, NAV_SCREENS.len());
            view::rounded_rect(r.x, r.y, r.w, r.h, r.h * 0.5, view::PANEL);
            if my_lane {
                draw_circle(r.x + r.h * 0.5, r.y + r.h * 0.5, r.h * 0.14, view::GOLD);
            }
            let label = if my_lane { "My lane: on" } else { "My lane" };
            draw_text(label, r.x + r.h * 0.9, r.y + r.h * 0.66, fs, highlight(my_lane));
        }

        // The button itself: 2x2 dot grid glyph.
        let btn = Self::button_rect(w, h);
        let (cx, cy) = (btn.x + btn.w / 2.0, btn.y + btn.h / 2.0);
        draw_circle(cx, cy, btn.w * 0.5, view::PANEL);
        draw_circle_lines(cx, cy, btn.w * 0.5, 2.0, view::TRACK_EDGE);
        let dot = btn.w * 0.09;
        let off = btn.w * 0.14;
        for (dx, dy) in [(-off, -off), (off, -off), (-off, off), (off, off)] {
            draw_circle(cx + dx, cy + dy, dot, if self.open { view::GOLD } else { view::ACCENT });
        }
    }
}

// ---- toasts ----

const TOAST_LIFE: f64 = 3.5;

/// Sliding in-game notifications ("Baton incoming!").
pub struct Toasts {
    items: Vec<(String, f64)>,
}

impl Toasts {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, text: impl Into<String>) {
        self.items.push((text.into(), get_time()));
    }

    pub fn draw(&mut self, w: f32, h: f32) {
        let now = get_time();
        self.items.retain(|(_, at)| now - at < TOAST_LIFE);
        let fs = h * 0.024;
        for (i, (text, at)) in self.items.iter().enumerate() {
            let t = ((now - at) / TOAST_LIFE) as f32;
            // Slide in from the top, fade out at the end.
            let slide = (1.0 - (t * 12.0).min(1.0)) * h * 0.04;
            let alpha = if t > 0.85 { (1.0 - t) / 0.15 } else { 1.0 };
            let dims = measure_text(text, None, fs as u16, 1.0);
            let bw = dims.width + fs * 2.2;
            let bh = fs * 1.9;
            let x = (w - bw) / 2.0;
            // Below the HUD pill / screen headers so nothing overlaps.
            let y = h * 0.155 + i as f32 * (bh + h * 0.01) - slide;
            let mut panel = view::PANEL;
            panel.a = 0.95 * alpha;
            view::rounded_rect(x, y, bw, bh, bh * 0.5, panel);
            let mut gold = view::GOLD;
            gold.a = alpha;
            draw_circle(x + bh * 0.5, y + bh * 0.5, fs * 0.28, gold);
            let mut ink = view::INK;
            ink.a = alpha;
            draw_text(text, x + bh * 0.9, y + fs * 1.3, fs, ink);
        }
    }
}

// ---- shared screen chrome ----

/// Screen header with a back-to-track chevron, title, and subtitle.
/// Returns true when the back button was tapped.
pub fn header(tap: Option<Vec2>, w: f32, h: f32, title: &str, subtitle: &str) -> bool {
    let d = w * 0.1;
    let back = Rect::new(w * 0.04, h * 0.025, d, d);
    view::rounded_rect(back.x, back.y, back.w, back.h, d * 0.3, view::PANEL);
    let fs = d * 0.62;
    draw_text("<", back.x + d * 0.32, back.y + d * 0.68, fs, view::INK);
    let tfs = h * 0.036;
    draw_text(title, back.x + d + w * 0.035, back.y + tfs * 0.95, tfs, view::INK);
    draw_text(subtitle, back.x + d + w * 0.035, back.y + tfs * 1.85, tfs * 0.55, view::INK_DIM);
    tap.map_or(false, |p| back.contains(p))
}

// ---- pure text helpers ----

/// 12345 -> "12,345".
pub fn fmt_xp(xp: u64) -> String {
    let digits = xp.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Compact duration: "45s", "14m", "1h05m". Negative means unknown: "-".
/// (ASCII only: macroquad's default font has no em-dash or ellipsis glyphs.)
pub fn fmt_dur(secs: i64) -> String {
    if secs < 0 {
        "-".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Char-safe truncation with an ASCII ellipsis (subjects contain multi-byte
/// ★, and the default font lacks the '…' glyph).
pub fn ellipsize(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::MessageStatus;

    fn msg(channel: Channel, subject: &str, received_at: i64) -> Message {
        Message {
            id: 1,
            channel,
            sender: "t".into(),
            subject: subject.into(),
            body: String::new(),
            received_at,
            status: MessageStatus::Open,
        }
    }

    #[test]
    fn filters_match_channel_assignee_and_burning() {
        let m = msg(Channel::Ticket, "API 500 fail", 1_000);
        assert!(FilterRequest::Channel(Channel::Ticket).matches(&m, None, 1_000));
        assert!(!FilterRequest::Channel(Channel::Email).matches(&m, None, 1_000));
        assert!(FilterRequest::Category(Category::Technical).matches(&m, None, 1_000));
        assert!(FilterRequest::Assignee(RunnerId::Me).matches(&m, Some(RunnerId::Me), 1_000));
        assert!(!FilterRequest::Assignee(RunnerId::Me).matches(&m, Some(RunnerId::Bot(0)), 1_000));
        assert!(!FilterRequest::Assignee(RunnerId::Me).matches(&m, None, 1_000));
        assert!(FilterRequest::Burning.matches(&m, None, 1_000 + team::BURNING_AGE_SECS));
        assert!(!FilterRequest::Burning.matches(&m, None, 1_000));
    }

    #[test]
    fn xp_formatting_groups_thousands() {
        assert_eq!(fmt_xp(0), "0");
        assert_eq!(fmt_xp(999), "999");
        assert_eq!(fmt_xp(1_000), "1,000");
        assert_eq!(fmt_xp(1_234_567), "1,234,567");
    }

    #[test]
    fn durations_format_compactly() {
        assert_eq!(fmt_dur(-1), "-");
        assert_eq!(fmt_dur(45), "45s");
        assert_eq!(fmt_dur(14 * 60), "14m");
        assert_eq!(fmt_dur(3900), "1h05m");
    }

    #[test]
    fn ellipsize_is_char_safe() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("★★★★★ Support turned it around", 8), "★★★★★ S...");
    }
}
