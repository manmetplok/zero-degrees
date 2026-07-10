//! Race control (story 011): pure inbox statistics plus the team-lead
//! overview screen. Every segment is tappable and emits a `FilterRequest`
//! the track view consumes (story 006's filter overlay can take those over
//! unchanged).
//!
//! Also hosts the stand-in message enrichment (category / mood / urgency)
//! until meta.rs's `enrich` lands: swap `category_of`, `mood_of` and
//! `urgency_of` for the real classifier and everything downstream follows.

use macroquad::prelude::*;
use shared::{Channel, Message, MessageStatus};

use crate::screens::FilterRequest;
use crate::team::{self, RunnerId};
use crate::view;

/// Shared "overdue" color for burning counts, hazard strips and the enraged
/// boss.
pub const FIRE: Color = Color::new(1.0, 0.47, 0.15, 1.0);

// ---- stand-in enrichment (to be replaced by meta::enrich) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Billing,
    Account,
    Technical,
    Shipping,
    Feedback,
    Other,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Billing,
        Category::Account,
        Category::Technical,
        Category::Shipping,
        Category::Feedback,
        Category::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Billing => "billing",
            Category::Account => "account",
            Category::Technical => "technical",
            Category::Shipping => "shipping",
            Category::Feedback => "feedback",
            Category::Other => "other",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Category::Billing => view::GOLD,
            Category::Account => Color::new(0.62, 0.45, 0.92, 1.0),
            Category::Technical => Color::new(0.36, 0.56, 0.95, 1.0),
            Category::Shipping => Color::new(0.24, 0.78, 0.72, 1.0),
            Category::Feedback => Color::new(1.0, 0.58, 0.35, 1.0),
            Category::Other => view::INK_DIM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Positive,
    Neutral,
    Negative,
}

impl Mood {
    pub const ALL: [Mood; 3] = [Mood::Positive, Mood::Neutral, Mood::Negative];

    pub fn label(self) -> &'static str {
        match self {
            Mood::Positive => "positive",
            Mood::Neutral => "neutral",
            Mood::Negative => "negative",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Mood::Positive => Color::new(0.38, 0.82, 0.47, 1.0),
            Mood::Neutral => view::INK_DIM,
            Mood::Negative => Color::new(0.95, 0.33, 0.33, 1.0),
        }
    }
}

fn haystack(msg: &Message) -> String {
    format!("{} {}", msg.subject, msg.body).to_lowercase()
}

fn any_in(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

/// Keyword classifier standing in for meta::enrich's category.
pub fn category_of(msg: &Message) -> Category {
    if msg.channel == Channel::Review {
        return Category::Feedback;
    }
    let text = haystack(msg);
    if any_in(&text, &["invoice", "charge", "billed", "refund", "pricing", "subscription"]) {
        Category::Billing
    } else if any_in(&text, &["password", "login", "locked", "sso", "account"]) {
        Category::Account
    } else if any_in(&text, &["500", "api", "export", "webhook", "error", "fail", "bug"]) {
        Category::Technical
    } else if any_in(&text, &["shipment", "delivery", "address", "arrive"]) {
        Category::Shipping
    } else {
        Category::Other
    }
}

/// Keyword classifier standing in for meta::enrich's sentiment.
pub fn mood_of(msg: &Message) -> Mood {
    let text = haystack(msg);
    if any_in(
        &text,
        &["twice", "can't", "fail", "locked", "urgent", "asap", "flood", "hammer", "slow", "☆", "block", "nobody"],
    ) {
        Mood::Negative
    } else if any_in(&text, &["great", "fixed", "thank", "keep customers", "★★★★★"]) {
        Mood::Positive
    } else {
        Mood::Neutral
    }
}

/// Urgency 1..=3, standing in for meta::enrich (story 004's hurdle height).
pub fn urgency_of(msg: &Message) -> u8 {
    let text = haystack(msg);
    if any_in(&text, &["urgent", "asap", "block", "locked"]) {
        3
    } else if any_in(&text, &["500", "fail", "error", "twice", "flood", "hammer", "can't", "expired"]) {
        2
    } else {
        1
    }
}

// ---- pure statistics ----

/// Everything race control shows, computed fresh each frame so live clears
/// and arrivals reflect immediately. "Open" includes skipped hurdles: they
/// still need work, the lead cares about the whole queue.
pub struct Stats {
    pub open: usize,
    pub cleared: usize,
    /// Open messages per channel, in `Channel::ALL` order.
    pub per_channel: [usize; 4],
    /// Open messages per category, in `Category::ALL` order.
    pub per_category: [usize; 6],
    /// Open messages per mood, in `Mood::ALL` order.
    pub per_mood: [usize; 3],
    pub burning: usize,
    pub hazard_zones: usize,
}

/// `ordered` must iterate messages in track order (the track keeps hurdles
/// sorted by position), so hazard zones — runs of hot hurdles — make sense.
pub fn compute<'a>(ordered: impl Iterator<Item = &'a Message>, sim_now: i64) -> Stats {
    let mut stats = Stats {
        open: 0,
        cleared: 0,
        per_channel: [0; 4],
        per_category: [0; 6],
        per_mood: [0; 3],
        burning: 0,
        hazard_zones: 0,
    };
    let mut hot = Vec::new();
    for msg in ordered {
        if msg.status == MessageStatus::Cleared {
            stats.cleared += 1;
            continue;
        }
        stats.open += 1;
        let ch = Channel::ALL.iter().position(|c| *c == msg.channel).unwrap_or(0);
        stats.per_channel[ch] += 1;
        let cat = Category::ALL.iter().position(|c| *c == category_of(msg)).unwrap_or(0);
        stats.per_category[cat] += 1;
        let mood = Mood::ALL.iter().position(|m| *m == mood_of(msg)).unwrap_or(0);
        stats.per_mood[mood] += 1;
        let burning = team::is_burning(msg, sim_now);
        if burning {
            stats.burning += 1;
        }
        hot.push(burning || urgency_of(msg) == 3);
    }
    stats.hazard_zones = hazard_zone_count(&hot);
    stats
}

/// A hazard zone is a stretch of two or more consecutive hot (burning or
/// max-urgency) open hurdles. Stand-in for story 010's real zones.
pub fn hazard_zone_count(hot: &[bool]) -> usize {
    let mut zones = 0;
    let mut run = 0;
    for &h in hot {
        if h {
            run += 1;
            if run == 2 {
                zones += 1;
            }
        } else {
            run = 0;
        }
    }
    zones
}

/// Per-runner progress row for the dashboard.
pub struct RunnerProgress {
    pub runner: RunnerId,
    pub clears: u32,
    /// Real time of the latest clear, for the live pulse.
    pub last_clear: Option<f64>,
}

// ---- drawing ----

/// Draw the dashboard below the screen header and hit-test `tap` against
/// every segment. Returns the filter request of the tapped segment, if any.
pub fn draw(
    tap: Option<Vec2>,
    w: f32,
    h: f32,
    stats: &Stats,
    runners: &[RunnerProgress],
    now: f64,
) -> Option<FilterRequest> {
    let mut picked = None;
    let pad = w * 0.05;
    let fs = h * 0.021;
    let tapped = |r: Rect| tap.map_or(false, |p| r.contains(p));

    // Open / cleared tiles.
    let mut y = h * 0.115;
    let tile_h = h * 0.07;
    let tile_w = (w - 2.0 * pad - w * 0.03) / 2.0;
    for (i, (label, value, color)) in [
        ("OPEN", stats.open, view::ACCENT),
        ("CLEARED", stats.cleared, view::GOLD),
    ]
    .iter()
    .enumerate()
    {
        let x = pad + i as f32 * (tile_w + w * 0.03);
        view::rounded_rect(x, y, tile_w, tile_h, w * 0.02, view::PANEL);
        draw_rectangle(x, y, w * 0.012, tile_h, *color);
        draw_text(label, x + fs * 1.4, y + tile_h / 2.0 + fs * 0.35, fs, view::INK_DIM);
        let num = value.to_string();
        let dims = measure_text(&num, None, (fs * 2.1) as u16, 1.0);
        draw_text(&num, x + tile_w - dims.width - fs * 1.2, y + tile_h / 2.0 + fs * 0.8, fs * 2.1, view::INK);
    }
    y += tile_h + h * 0.022;

    // Volume per channel: one bar per channel, tap filters the track.
    draw_text("VOLUME PER CHANNEL", pad, y + fs, fs, view::INK_DIM);
    y += fs * 1.8;
    let max_ch = stats.per_channel.iter().copied().max().max(Some(1)).unwrap() as f32;
    let row_h = h * 0.031;
    for (i, channel) in Channel::ALL.iter().enumerate() {
        let count = stats.per_channel[i];
        let row = Rect::new(pad, y, w - 2.0 * pad, row_h);
        if tapped(row) {
            picked = Some(FilterRequest::Channel(*channel));
        }
        let color = view::channel_color(*channel);
        view::channel_icon(pad + row_h * 0.45, y + row_h * 0.45, row_h * 0.62, *channel, color);
        draw_text(channel.label(), pad + row_h * 1.3, y + row_h * 0.72, fs, view::INK);
        let bar_x = pad + w * 0.28;
        let bar_w = (w - pad - bar_x - fs * 2.2) * (count as f32 / max_ch);
        view::rounded_rect(bar_x, y + row_h * 0.22, bar_w.max(row_h * 0.2), row_h * 0.5, row_h * 0.25, color);
        draw_text(&count.to_string(), w - pad - fs * 1.2, y + row_h * 0.72, fs, view::INK_DIM);
        y += row_h + h * 0.006;
    }
    y += h * 0.014;

    // Hurdle types (category distribution): stacked bar + tappable chips.
    draw_text("HURDLE TYPES", pad, y + fs, fs, view::INK_DIM);
    y += fs * 1.6;
    y = stacked_bar(
        w,
        y,
        Category::ALL
            .iter()
            .enumerate()
            .map(|(i, c)| (stats.per_category[i], c.color()))
            .collect(),
    );
    let chip_w = (w - 2.0 * pad - w * 0.02 * 2.0) / 3.0;
    let chip_h = h * 0.031;
    let mut chip_i = 0;
    for (i, cat) in Category::ALL.iter().enumerate() {
        if stats.per_category[i] == 0 {
            continue;
        }
        let cx = pad + (chip_i % 3) as f32 * (chip_w + w * 0.02);
        let cy = y + (chip_i / 3) as f32 * (chip_h + h * 0.008);
        let chip = Rect::new(cx, cy, chip_w, chip_h);
        if tapped(chip) {
            picked = Some(FilterRequest::Category(*cat));
        }
        view::rounded_rect(cx, cy, chip_w, chip_h, chip_h * 0.5, view::PANEL);
        draw_circle(cx + chip_h * 0.55, cy + chip_h * 0.5, chip_h * 0.22, cat.color());
        draw_text(
            &format!("{} {}", cat.label(), stats.per_category[i]),
            cx + chip_h * 1.1,
            cy + chip_h * 0.7,
            fs,
            view::INK,
        );
        chip_i += 1;
    }
    y += ((chip_i + 2) / 3) as f32 * (chip_h + h * 0.008) + h * 0.018;

    // Mood breakdown: stacked bar + tappable legend.
    draw_text("MOOD", pad, y + fs, fs, view::INK_DIM);
    y += fs * 1.6;
    y = stacked_bar(
        w,
        y,
        Mood::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| (stats.per_mood[i], m.color()))
            .collect(),
    );
    let third = (w - 2.0 * pad) / 3.0;
    for (i, mood) in Mood::ALL.iter().enumerate() {
        let cell = Rect::new(pad + i as f32 * third, y, third, chip_h);
        if tapped(cell) {
            picked = Some(FilterRequest::Mood(*mood));
        }
        draw_circle(cell.x + chip_h * 0.4, cell.y + chip_h * 0.5, chip_h * 0.22, mood.color());
        draw_text(
            &format!("{} {}", mood.label(), stats.per_mood[i]),
            cell.x + chip_h * 0.9,
            cell.y + chip_h * 0.7,
            fs,
            view::INK,
        );
    }
    y += chip_h + h * 0.018;

    // Burning / hazard zones strip.
    let strip = Rect::new(pad, y, w - 2.0 * pad, h * 0.045);
    if tapped(strip) {
        picked = Some(FilterRequest::Burning);
    }
    view::rounded_rect(strip.x, strip.y, strip.w, strip.h, w * 0.02, view::PANEL);
    flame(strip.x + strip.h * 0.55, strip.y + strip.h * 0.52, strip.h * 0.6);
    let hazard_text = format!(
        "{} burning  ·  {} hazard zone{}",
        stats.burning,
        stats.hazard_zones,
        if stats.hazard_zones == 1 { "" } else { "s" }
    );
    draw_text(&hazard_text, strip.x + strip.h * 1.2, strip.y + strip.h * 0.65, fs * 1.1, if stats.burning > 0 { FIRE } else { view::INK_DIM });
    y += strip.h + h * 0.02;

    // Per-runner progress.
    draw_text("RUNNERS", pad, y + fs, fs, view::INK_DIM);
    y += fs * 1.7;
    let max_clears = runners.iter().map(|r| r.clears).max().unwrap_or(0).max(1) as f32;
    let rrow = h * 0.045;
    for rp in runners {
        let color = team::runner_color(rp.runner);
        team::avatar(pad + rrow * 0.42, y + rrow * 0.45, rrow * 0.34, rp.runner);
        // Live pulse ring right after a clear (near-real-time demo feel).
        if let Some(at) = rp.last_clear {
            let t = ((now - at) / 1.0) as f32;
            if t < 1.0 {
                let mut c = color;
                c.a = 1.0 - t;
                draw_circle_lines(pad + rrow * 0.42, y + rrow * 0.45, rrow * (0.36 + t * 0.5), 2.0, c);
            }
        }
        draw_text(team::runner_name(rp.runner), pad + rrow * 1.05, y + rrow * 0.58, fs * 1.05, view::INK);
        let bar_x = pad + w * 0.26;
        let bar_w = (w - pad - bar_x - fs * 2.6) * (rp.clears as f32 / max_clears);
        view::rounded_rect(bar_x, y + rrow * 0.24, bar_w.max(2.0), rrow * 0.4, rrow * 0.2, color);
        draw_text(&rp.clears.to_string(), w - pad - fs * 1.4, y + rrow * 0.58, fs * 1.05, view::INK_DIM);
        y += rrow + h * 0.006;
    }

    if picked.is_some() {
        return picked;
    }
    None
}

/// Full-width stacked distribution bar; returns the y below it.
fn stacked_bar(w: f32, y: f32, parts: Vec<(usize, Color)>) -> f32 {
    let pad = w * 0.05;
    let bar_h = w * 0.035;
    let total: usize = parts.iter().map(|(n, _)| n).sum();
    view::rounded_rect(pad, y, w - 2.0 * pad, bar_h, bar_h * 0.5, view::TRACK_EDGE);
    if total > 0 {
        let mut x = pad;
        for (n, color) in parts {
            if n == 0 {
                continue;
            }
            let seg = (w - 2.0 * pad) * n as f32 / total as f32;
            view::rounded_rect(x, y, seg, bar_h, bar_h * 0.5, color);
            x += seg;
        }
    }
    y + bar_h + w * 0.02
}

/// Small primitive flame glyph for burning indicators.
pub fn flame(cx: f32, cy: f32, s: f32) {
    draw_triangle(
        vec2(cx, cy - s * 0.62),
        vec2(cx - s * 0.38, cy + s * 0.3),
        vec2(cx + s * 0.38, cy + s * 0.3),
        FIRE,
    );
    draw_circle(cx, cy + s * 0.12, s * 0.38, FIRE);
    draw_circle(cx, cy + s * 0.16, s * 0.2, view::GOLD);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: u64, channel: Channel, subject: &str, body: &str, received_at: i64, status: MessageStatus) -> Message {
        Message {
            id,
            channel,
            sender: "t@example.com".into(),
            subject: subject.into(),
            body: body.into(),
            received_at,
            status,
        }
    }

    #[test]
    fn classifier_buckets_the_obvious_cases() {
        let billing = msg(1, Channel::Email, "Invoice #1 charged twice", "refund please", 0, MessageStatus::Open);
        assert_eq!(category_of(&billing), Category::Billing);
        assert_eq!(mood_of(&billing), Mood::Negative);
        assert_eq!(urgency_of(&billing), 2);

        let urgent = msg(2, Channel::Ticket, "Urgent: locked out", "demo ASAP", 0, MessageStatus::Open);
        assert_eq!(category_of(&urgent), Category::Account);
        assert_eq!(urgency_of(&urgent), 3);

        let review = msg(3, Channel::Review, "★★★★★ great", "support fixed it, great!", 0, MessageStatus::Open);
        assert_eq!(category_of(&review), Category::Feedback, "reviews are always feedback");
        assert_eq!(mood_of(&review), Mood::Positive);

        let neutral = msg(4, Channel::WebForm, "Change of address", "new delivery address", 0, MessageStatus::Open);
        assert_eq!(category_of(&neutral), Category::Shipping);
        assert_eq!(mood_of(&neutral), Mood::Neutral);
        assert_eq!(urgency_of(&neutral), 1);
    }

    #[test]
    fn compute_counts_open_cleared_burning_and_distributions() {
        let now = 100_000;
        let msgs = vec![
            // Burning + urgent → hot.
            msg(1, Channel::Email, "Urgent: locked out", "asap", now - 8_000, MessageStatus::Open),
            // Burning only → hot.
            msg(2, Channel::Ticket, "API 500 fail", "error", now - 7_000, MessageStatus::Open),
            // Fresh and calm → not hot, breaks the run.
            msg(3, Channel::WebForm, "Question", "family plan?", now - 10, MessageStatus::Open),
            msg(4, Channel::Review, "★★★★★", "great", now - 10, MessageStatus::Open),
            msg(5, Channel::Email, "done", "done", now - 9_000, MessageStatus::Cleared),
        ];
        let stats = compute(msgs.iter(), now);
        assert_eq!(stats.open, 4);
        assert_eq!(stats.cleared, 1);
        assert_eq!(stats.burning, 2);
        assert_eq!(stats.hazard_zones, 1, "two adjacent hot hurdles form one zone");
        assert_eq!(stats.per_channel[0], 1, "one open email");
        assert_eq!(stats.per_mood[0], 1, "one positive review");
        let feedback_idx = Category::ALL.iter().position(|c| *c == Category::Feedback).unwrap();
        assert_eq!(stats.per_category[feedback_idx], 1);
    }

    #[test]
    fn skipped_messages_still_count_as_open_work() {
        let m = msg(1, Channel::Email, "hi", "hi", 0, MessageStatus::Skipped);
        let stats = compute([&m].into_iter().map(|m| m), 10);
        assert_eq!(stats.open, 1);
        assert_eq!(stats.cleared, 0);
    }

    #[test]
    fn hazard_zones_are_runs_of_two_or_more_hot_hurdles() {
        assert_eq!(hazard_zone_count(&[]), 0);
        assert_eq!(hazard_zone_count(&[true, false, true]), 0);
        assert_eq!(hazard_zone_count(&[true, true, false, true, true, true]), 2);
        assert_eq!(hazard_zone_count(&[true; 5]), 1);
    }
}
