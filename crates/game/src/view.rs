//! Drawing helpers: palette, primitive-built channel icons, hurdles, HUD
//! widgets. Everything is drawn from shapes — no image assets besides the
//! runner sprite strips.

use macroquad::prelude::*;
use shared::Channel;

pub const SKY: Color = Color::new(0.086, 0.086, 0.20, 1.0);
pub const TRACK: Color = Color::new(0.16, 0.16, 0.32, 1.0);
pub const TRACK_EDGE: Color = Color::new(0.24, 0.24, 0.45, 1.0);
pub const PANEL: Color = Color::new(0.115, 0.115, 0.26, 1.0);
pub const INK: Color = Color::new(0.93, 0.93, 0.98, 1.0);
pub const INK_DIM: Color = Color::new(0.62, 0.62, 0.75, 1.0);
pub const ACCENT: Color = Color::new(0.96, 0.15, 0.52, 1.0);
pub const GOLD: Color = Color::new(1.0, 0.82, 0.29, 1.0);

pub fn channel_color(channel: Channel) -> Color {
    match channel {
        Channel::Email => Color::from_rgba(0x43, 0x61, 0xee, 255),
        Channel::WebForm => Color::from_rgba(0x2e, 0xc4, 0xb6, 255),
        Channel::Review => Color::from_rgba(0xff, 0xb7, 0x03, 255),
        Channel::Ticket => Color::from_rgba(0x9d, 0x4e, 0xdd, 255),
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

pub struct HurdleStyle {
    pub color: Color,
    pub faded: bool,
    /// Knocked flat (cleared) instead of standing.
    pub down: bool,
    /// Show a "come back later" marker (skipped).
    pub marked: bool,
}

/// Draw a hurdle whose feet stand at (x, ground_y). `unit` is pixels per
/// track unit; hurdles are ~1.3 units tall.
pub fn hurdle(x: f32, ground_y: f32, unit: f32, channel: Channel, style: &HurdleStyle) {
    let mut color = style.color;
    let mut post = Color::new(0.55, 0.58, 0.70, 1.0);
    if style.faded {
        color.a = 0.35;
        post.a = 0.35;
    }
    let bar_h = unit * 1.3;
    let half_w = unit * 0.55;
    let t = (unit * 0.09).max(2.0);

    if style.down {
        // Cleared: crossbar tipped onto the track behind the posts.
        draw_line(x - half_w, ground_y, x + half_w, ground_y - unit * 0.18, t * 1.4, post);
        return;
    }

    draw_line(x - half_w, ground_y, x - half_w, ground_y - bar_h, t, post);
    draw_line(x + half_w, ground_y, x + half_w, ground_y - bar_h, t, post);
    draw_line(
        x - half_w - t,
        ground_y - bar_h,
        x + half_w + t,
        ground_y - bar_h,
        t * 1.8,
        color,
    );

    // Channel sign floating above the crossbar.
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
    channel_icon(x, sign_cy, sign * 0.8, channel, color);

    if style.marked {
        let mut mark = ACCENT;
        mark.a = if style.faded { 0.6 } else { 1.0 };
        draw_circle(x + sign * 0.62, sign_cy - sign * 0.62, sign * 0.22, mark);
        let fs = sign * 0.38;
        draw_text("!", x + sign * 0.62 - fs * 0.14, sign_cy - sign * 0.62 + fs * 0.36, fs, INK);
    }
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
