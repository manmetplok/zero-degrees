//! Drawing helpers: palette, primitive-built channel/category icons, hurdles
//! with their triage visuals (height, aura, burning state), HUD widgets.
//! Everything is drawn from shapes — no image assets besides the runner
//! sprite strips.

use macroquad::prelude::*;
use shared::Channel;

use crate::meta::{Category, Sentiment, Urgency};

pub const SKY: Color = Color::new(0.086, 0.086, 0.20, 1.0);
pub const TRACK: Color = Color::new(0.16, 0.16, 0.32, 1.0);
pub const TRACK_EDGE: Color = Color::new(0.24, 0.24, 0.45, 1.0);
pub const PANEL: Color = Color::new(0.115, 0.115, 0.26, 1.0);
pub const INK: Color = Color::new(0.93, 0.93, 0.98, 1.0);
pub const INK_DIM: Color = Color::new(0.62, 0.62, 0.75, 1.0);
pub const ACCENT: Color = Color::new(0.96, 0.15, 0.52, 1.0);
pub const GOLD: Color = Color::new(1.0, 0.82, 0.29, 1.0);
/// Fire and overdue accents (burning hurdles, late warnings).
pub const FLAME: Color = Color::new(1.0, 0.45, 0.13, 1.0);
const FLAME_CORE: Color = Color::new(1.0, 0.78, 0.25, 0.95);
const SMOKE: Color = Color::new(0.55, 0.55, 0.62, 1.0);
const RAIN_CLOUD: Color = Color::new(0.52, 0.55, 0.66, 1.0);
const RAIN_DROP: Color = Color::new(0.45, 0.65, 0.95, 1.0);

pub fn channel_color(channel: Channel) -> Color {
    match channel {
        Channel::Email => Color::from_rgba(0x43, 0x61, 0xee, 255),
        Channel::WebForm => Color::from_rgba(0x2e, 0xc4, 0xb6, 255),
        Channel::Review => Color::from_rgba(0xff, 0xb7, 0x03, 255),
        Channel::Ticket => Color::from_rgba(0x9d, 0x4e, 0xdd, 255),
    }
}

/// Category palette: the hurdle's primary read on the track (story 003).
pub fn category_color(category: Category) -> Color {
    match category {
        Category::Billing => Color::from_rgba(0xff, 0xca, 0x3a, 255),
        Category::Complaint => Color::from_rgba(0xff, 0x4d, 0x6d, 255),
        Category::Question => Color::from_rgba(0x4c, 0xc9, 0xf0, 255),
        Category::Feedback => Color::from_rgba(0x80, 0xed, 0x99, 255),
    }
}

/// Urgency accent for glows and warning text (story 004).
pub fn urgency_color(urgency: Urgency) -> Color {
    match urgency {
        Urgency::Critical => Color::new(0.98, 0.22, 0.35, 1.0),
        Urgency::High => Color::new(1.0, 0.62, 0.11, 1.0),
        Urgency::Normal => INK_DIM,
        Urgency::Low => Color::new(0.42, 0.80, 0.55, 1.0),
    }
}

/// Crossbar height in track units: urgency is literally the biggest thing on
/// screen (story 004). Kept under the runner's jump arc.
pub fn urgency_height(urgency: Urgency) -> f32 {
    match urgency {
        Urgency::Critical => 1.7,
        Urgency::High => 1.3,
        Urgency::Normal => 0.95,
        Urgency::Low => 0.55,
    }
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let r = r.min(w / 2.0).min(h / 2.0);
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, w, h - 2.0 * r, color);
    for (cx, cy) in [
        (x + r, y + r),
        (x + w - r, y + r),
        (x + r, y + h - r),
        (x + w - r, y + h - r),
    ] {
        draw_circle(cx, cy, r, color);
    }
}

/// Channel glyph centered on (cx, cy), `size` = glyph width.
pub fn channel_icon(cx: f32, cy: f32, size: f32, channel: Channel, color: Color) {
    let s = size;
    let t = (s * 0.09).max(1.5); // stroke width
    match channel {
        Channel::Email => {
            // Envelope: box + flap.
            let w = s;
            let h = s * 0.68;
            let (x, y) = (cx - w / 2.0, cy - h / 2.0);
            draw_rectangle_lines(x, y, w, h, t * 2.0, color);
            draw_line(x, y, cx, cy + h * 0.12, t, color);
            draw_line(x + w, y, cx, cy + h * 0.12, t, color);
        }
        Channel::WebForm => {
            // Form: page with entry lines.
            let w = s * 0.78;
            let h = s;
            let (x, y) = (cx - w / 2.0, cy - h / 2.0);
            draw_rectangle_lines(x, y, w, h, t * 2.0, color);
            for i in 0..3 {
                let ly = y + h * (0.28 + 0.22 * i as f32);
                draw_line(x + w * 0.2, ly, x + w * 0.8, ly, t, color);
            }
        }
        Channel::Review => {
            // Five-pointed star from a triangle fan.
            let outer = s * 0.55;
            let inner = outer * 0.45;
            let mut pts = Vec::with_capacity(10);
            for i in 0..10 {
                let r = if i % 2 == 0 { outer } else { inner };
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                pts.push(vec2(cx + r * a.cos(), cy + r * a.sin()));
            }
            let center = vec2(cx, cy);
            for i in 0..10 {
                draw_triangle(center, pts[i], pts[(i + 1) % 10], color);
            }
        }
        Channel::Ticket => {
            // Ticket stub: rounded slip with a perforation line.
            let w = s;
            let h = s * 0.6;
            let (x, y) = (cx - w / 2.0, cy - h / 2.0);
            rounded_rect(x, y, w, h, h * 0.22, color);
            let px = x + w * 0.68;
            let dash = h / 7.0;
            let mut dy = y + dash * 0.5;
            while dy < y + h - dash * 0.4 {
                draw_line(px, dy, px, dy + dash * 0.6, t, SKY);
                dy += dash * 1.4;
            }
        }
    }
}

/// Category glyph centered on (cx, cy), `size` = glyph width (story 003).
pub fn category_icon(cx: f32, cy: f32, size: f32, category: Category, color: Color) {
    let s = size;
    let t = (s * 0.1).max(1.5);
    match category {
        Category::Billing => {
            // Coin: ring with a value slot.
            draw_circle_lines(cx, cy, s * 0.48, t * 1.6, color);
            draw_line(cx - s * 0.2, cy, cx + s * 0.2, cy, t * 1.4, color);
        }
        Category::Complaint => {
            // Warning triangle with an exclamation mark.
            let h = s * 0.9;
            let (top, ly, lx, rx) = (cy - h * 0.55, cy + h * 0.45, cx - s * 0.55, cx + s * 0.55);
            draw_line(lx, ly, cx, top, t * 1.6, color);
            draw_line(rx, ly, cx, top, t * 1.6, color);
            draw_line(lx, ly, rx, ly, t * 1.6, color);
            draw_line(cx, cy - h * 0.18, cx, cy + h * 0.14, t * 1.4, color);
            draw_circle(cx, cy + h * 0.3, t * 0.8, color);
        }
        Category::Question => {
            // Speech bubble with a question mark.
            let w = s;
            let h = s * 0.78;
            let (x, y) = (cx - w / 2.0, cy - h * 0.62);
            rounded_rect(x, y, w, h, h * 0.3, color);
            draw_triangle(
                vec2(cx - w * 0.12, y + h),
                vec2(cx + w * 0.2, y + h),
                vec2(cx - w * 0.05, y + h + h * 0.32),
                color,
            );
            let fs = s * 0.8;
            let dims = measure_text("?", None, fs as u16, 1.0);
            draw_text("?", cx - dims.width / 2.0, y + h * 0.78, fs, SKY);
        }
        Category::Feedback => {
            // Heart: two lobes and a point.
            let r = s * 0.26;
            let (ly, py) = (cy - s * 0.12, cy + s * 0.42);
            draw_circle(cx - r * 0.85, ly, r, color);
            draw_circle(cx + r * 0.85, ly, r, color);
            draw_triangle(
                vec2(cx - r * 1.78, ly + r * 0.35),
                vec2(cx + r * 1.78, ly + r * 0.35),
                vec2(cx, py),
                color,
            );
        }
    }
}

/// Waiting-time tag drawn beside a hurdle. `frac` is waited / target: it
/// tints amber as the deadline nears and flame-red once overdue.
pub struct WaitLabel {
    pub text: String,
    pub frac: f32,
}

pub struct HurdleStyle {
    pub category: Category,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
    pub faded: bool,
    /// Knocked flat (cleared) instead of standing.
    pub down: bool,
    /// Show a "come back later" marker (skipped).
    pub marked: bool,
    /// Past the response target: the hurdle is on fire (story 014).
    pub burning: bool,
    pub wait: Option<WaitLabel>,
}

/// Draw a hurdle whose feet stand at (x, ground_y). `unit` is pixels per
/// track unit; `t` is the animation clock in seconds. Height comes from
/// urgency, color/shape/icon from category, the aura from sentiment.
pub fn hurdle(x: f32, ground_y: f32, unit: f32, style: &HurdleStyle, t: f32) {
    let mut color = category_color(style.category);
    let mut post = Color::new(0.55, 0.58, 0.70, 1.0);
    if style.faded {
        color.a = 0.35;
        post.a = 0.35;
    }
    let bar_h = unit * urgency_height(style.urgency);
    let half_w = unit * 0.55;
    let stroke = (unit * 0.09).max(2.0);

    if style.down {
        // Cleared: crossbar tipped onto the track behind the posts.
        draw_line(x - half_w, ground_y, x + half_w, ground_y - unit * 0.18, stroke * 1.4, post);
        return;
    }

    // Urgency intensity: a warning glow behind critical and high hurdles.
    if !style.faded {
        let glow = match style.urgency {
            Urgency::Critical => Some((unit * 1.3, 0.10 + 0.06 * (t * 3.0).sin())),
            Urgency::High => Some((unit * 0.9, 0.07)),
            _ => None,
        };
        if let Some((radius, alpha)) = glow {
            let mut c = urgency_color(style.urgency);
            let cy = ground_y - bar_h * 0.6;
            c.a = alpha;
            draw_circle(x, cy, radius, c);
            c.a = alpha * 0.8;
            draw_circle(x, cy, radius * 0.6, c);
        }
    }

    draw_line(x - half_w, ground_y, x - half_w, ground_y - bar_h, stroke, post);
    draw_line(x + half_w, ground_y, x + half_w, ground_y - bar_h, stroke, post);

    // Category shapes the barrier itself.
    let bar_y = ground_y - bar_h;
    match style.category {
        Category::Complaint => {
            // Solid panel: a barrier you don't argue with.
            let panel_h = (bar_h * 0.42).min(unit * 0.5);
            draw_rectangle(x - half_w - stroke, bar_y, 2.0 * (half_w + stroke), panel_h, color);
        }
        Category::Billing => {
            // Double rail, like a toll gate.
            draw_line(x - half_w - stroke, bar_y, x + half_w + stroke, bar_y, stroke * 1.8, color);
            let low = bar_y + (bar_h * 0.3).min(unit * 0.38);
            draw_line(x - half_w - stroke, low, x + half_w + stroke, low, stroke * 1.2, color);
        }
        Category::Question | Category::Feedback => {
            draw_line(x - half_w - stroke, bar_y, x + half_w + stroke, bar_y, stroke * 1.8, color);
        }
    }

    // Overdue: the hurdle is on fire — flames along the crossbar and posts,
    // smoke rising above (story 014).
    if style.burning && !style.faded {
        for i in 0..3 {
            let p = ((t * 0.45 + i as f32 * 0.33) % 1.0).max(0.0);
            let mut c = SMOKE;
            c.a = (1.0 - p) * 0.30;
            let sx = x + ((t * 0.9 + i as f32 * 2.1).sin()) * unit * 0.14;
            draw_circle(sx, bar_y - unit * 0.25 - p * unit * 1.3, unit * (0.10 + p * 0.14), c);
        }
        for i in 0..4 {
            let fx = x - half_w + (i as f32 + 0.5) * (2.0 * half_w / 4.0);
            flame(fx, bar_y + stroke, unit * 0.42, t, i as f32);
        }
        flame(x - half_w, ground_y - bar_h * 0.45, unit * 0.26, t, 4.2);
        flame(x + half_w, ground_y - bar_h * 0.55, unit * 0.26, t, 5.7);
    }

    // Category sign floating above the crossbar.
    let sign = unit * 0.9;
    let sign_cy = ground_y - bar_h - sign * 0.75;
    rounded_rect(
        x - sign * 0.62,
        sign_cy - sign * 0.62,
        sign * 1.24,
        sign * 1.24,
        sign * 0.2,
        Color::new(0.10, 0.10, 0.24, if style.faded { 0.35 } else { 0.92 }),
    );
    category_icon(x, sign_cy, sign * 0.72, style.category, color);

    // Mood aura from sentiment, visible from the track (story 005).
    if !style.faded {
        aura(x, sign_cy, sign, style.sentiment, t);
    }

    if style.marked {
        let mut mark = ACCENT;
        mark.a = if style.faded { 0.6 } else { 1.0 };
        draw_circle(x + sign * 0.62, sign_cy - sign * 0.62, sign * 0.22, mark);
        let fs = sign * 0.38;
        draw_text("!", x + sign * 0.62 - fs * 0.14, sign_cy - sign * 0.62 + fs * 0.36, fs, INK);
    }

    // Waiting-time countdown beside the hurdle (story 014).
    if let Some(wait) = &style.wait {
        let fs = unit * 0.32;
        let c = if wait.frac >= 1.0 {
            FLAME
        } else if wait.frac >= 0.6 {
            GOLD
        } else {
            INK_DIM
        };
        draw_text(&wait.text, x + half_w + unit * 0.12, ground_y - unit * 0.12, fs, c);
    }
}

/// One flickering flame with its base centered on (cx, base_y).
fn flame(cx: f32, base_y: f32, s: f32, t: f32, seed: f32) {
    let sway = (t * 6.0 + seed * 7.0).sin() * s * 0.18;
    let h = s * (1.0 + 0.25 * (t * 9.0 + seed * 3.0).sin());
    draw_triangle(
        vec2(cx - s * 0.38, base_y),
        vec2(cx + s * 0.38, base_y),
        vec2(cx + sway, base_y - h),
        Color::new(0.98, 0.45, 0.12, 0.9),
    );
    draw_triangle(
        vec2(cx - s * 0.20, base_y),
        vec2(cx + s * 0.20, base_y),
        vec2(cx + sway * 0.6, base_y - h * 0.62),
        FLAME_CORE,
    );
}

/// Sentiment aura around the hurdle sign at (x, sign_cy): flames for angry,
/// a rain cloud for sad, sparkles for happy (story 005).
fn aura(x: f32, sign_cy: f32, sign: f32, sentiment: Sentiment, t: f32) {
    let top = sign_cy - sign * 0.62;
    match sentiment {
        Sentiment::Angry => {
            for (i, dx) in [-0.42f32, 0.0, 0.42].iter().enumerate() {
                flame(x + dx * sign, top + sign * 0.06, sign * 0.30, t, i as f32 * 1.3);
            }
        }
        Sentiment::Negative => {
            let cloud_y = top - sign * 0.30;
            for (dx, dy, r) in [
                (-0.26f32, 0.06f32, 0.20f32),
                (0.0, -0.08, 0.24),
                (0.26, 0.06, 0.20),
            ] {
                draw_circle(x + dx * sign, cloud_y + dy * sign, sign * r, RAIN_CLOUD);
            }
            for i in 0..4 {
                let p = (t * 0.8 + i as f32 * 0.27) % 1.0;
                let dx = (-0.55 + 0.36 * i as f32) * sign;
                let mut c = RAIN_DROP;
                c.a = (1.0 - p) * 0.8;
                let dy = cloud_y + sign * 0.25 + p * sign * 1.5;
                draw_line(x + dx, dy, x + dx, dy + sign * 0.16, sign * 0.05, c);
            }
        }
        Sentiment::Positive => {
            for i in 0..5 {
                let a = t * 0.6 + i as f32 * std::f32::consts::TAU / 5.0;
                let (sx, sy) = (x + a.cos() * sign * 0.95, sign_cy + a.sin() * sign * 0.85);
                let twinkle = ((t * 3.0 + i as f32 * 2.1).sin() + 1.0) / 2.0;
                sparkle(sx, sy, sign * (0.08 + 0.08 * twinkle), 0.3 + 0.7 * twinkle);
            }
        }
        Sentiment::Neutral => {}
    }
}

/// Four-pointed twinkle.
fn sparkle(cx: f32, cy: f32, s: f32, alpha: f32) {
    let mut c = GOLD;
    c.a = alpha;
    let t = (s * 0.35).max(1.0);
    draw_line(cx - s, cy, cx + s, cy, t, c);
    draw_line(cx, cy - s, cx, cy + s, t, c);
}

/// Pill-shaped label; use `chip_rect` with the same arguments for hit
/// testing. `y` is the text baseline.
pub fn chip(x: f32, y: f32, fs: f32, label: &str, color: Color) -> Rect {
    let rect = chip_rect(x, y, fs, label);
    rounded_rect(rect.x, rect.y, rect.w, rect.h, rect.h / 2.0, color);
    draw_text(label, x + fs * 0.6, y, fs * 0.9, SKY);
    rect
}

/// The rect `chip` covers, for hit testing taps.
pub fn chip_rect(x: f32, y: f32, fs: f32, label: &str) -> Rect {
    let dims = measure_text(label, None, (fs * 0.9) as u16, 1.0);
    Rect::new(x, y - fs * 1.05, dims.width + fs * 1.2, fs * 1.45)
}

/// Checkered finish gate at x.
pub fn finish_line(x: f32, ground_y: f32, unit: f32) {
    let h = unit * 2.6;
    let post = Color::new(0.55, 0.58, 0.70, 1.0);
    let t = (unit * 0.1).max(2.0);
    draw_line(x, ground_y, x, ground_y - h, t, post);
    let sq = unit * 0.22;
    for row in 0..3 {
        for col in 0..5 {
            let c = if (row + col) % 2 == 0 { INK } else { SKY };
            draw_rectangle(
                x + t + col as f32 * sq,
                ground_y - h + row as f32 * sq,
                sq,
                sq,
                c,
            );
        }
    }
}
