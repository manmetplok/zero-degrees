//! Binoculars UI (stories 012, 010, 005): the one-thumb search/filter
//! overlay, hazard-zone banners along the track, and zone briefings. All
//! search/filter/zone state lives here; game.rs forwards gestures through
//! `intercept` and applies the returned camera/course actions, keeping the
//! shared file thin. Pure logic lives in filter.rs and hazards.rs.
//!
//! Dev harness: ZD_LENS=1 opens the overlay at startup (ZD_QUERY pre-fills
//! the search box) and ZD_ZONE=1 selects the first hazard zone — both exist
//! so screenshots can verify the visuals.

use macroquad::prelude::*;
use shared::{Channel, Message, MessageStatus};

use crate::filter::{self, Filter, MetaCache};
use crate::hazards::{self, HazardZone};
use crate::inbox::CourseSpec;
use crate::input::Gesture;
use crate::meta::{Category, Sentiment, Urgency};
use crate::track::Track;
use crate::view;

/// Screen mapping for the track, mirroring game.rs's layout values so this
/// module can place world-anchored widgets without owning the camera.
pub struct Viewport {
    pub w: f32,
    pub h: f32,
    /// Pixels per track unit.
    pub unit: f32,
    pub ground_y: f32,
    /// Screen x where the runner sits while the camera follows.
    pub anchor_x: f32,
    pub cam_x: f32,
}

impl Viewport {
    fn x_of(&self, track_x: f32) -> f32 {
        (track_x - self.cam_x) * self.unit + self.anchor_x
    }
}

/// What the game should do in response to a binoculars interaction.
pub enum Action {
    /// Move the camera to a track position (result tap, zone tap).
    JumpTo(f32),
    /// Relay a fresh course with a new seed (story 002's reset flag).
    Relay,
    /// Switch to the next difficulty preset and relay.
    CycleDifficulty,
}

/// Result of offering a gesture to the binoculars first.
pub enum Outcome {
    /// Not ours; the game handles it as usual.
    Pass(Gesture),
    /// Consumed, possibly with an action for the game to apply.
    Handled(Option<Action>),
}

/// Everything a chip in the overlay can toggle.
#[derive(Clone, Copy, PartialEq)]
enum Chip {
    Channel(Channel),
    Category(Category),
    Sentiment(Sentiment),
    Urgency(Urgency),
    Status(MessageStatus),
}

const MAX_QUERY: usize = 42;
const MAX_RESULTS: usize = 5;

pub struct Binoculars {
    open: bool,
    filter: Filter,
    cache: MetaCache,
    zones: Vec<HazardZone>,
    active_zone: Option<usize>,
    /// Ranked ids matching the current filter (and active zone).
    results: Vec<u64>,
    /// Startup action from the env harness, popped on the first frame.
    pending: Option<Action>,
}

impl Binoculars {
    pub fn new(track: &Track) -> Self {
        let mut lens = Self {
            open: false,
            filter: Filter::default(),
            cache: MetaCache::new(),
            zones: Vec::new(),
            active_zone: None,
            results: Vec::new(),
            pending: None,
        };
        lens.refresh(track);
        if std::env::var("ZD_LENS").is_ok() {
            lens.open = true;
            if let Ok(q) = std::env::var("ZD_QUERY") {
                lens.filter.query = q;
            }
        }
        if std::env::var("ZD_ZONE").is_ok() {
            if let Some(zone) = lens.zones.first() {
                lens.active_zone = Some(0);
                lens.pending = Some(Action::JumpTo(zone.start));
            }
        }
        lens.recompute(track);
        lens
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Rebuild the meta cache, re-detect hazard zones, and re-rank results.
    /// Call whenever the track changes (clear, skip, ingest, relay).
    pub fn refresh(&mut self, track: &Track) {
        self.cache.rebuild(track.hurdles.iter().map(|h| &h.message));
        let selected = self
            .active_zone
            .and_then(|i| self.zones.get(i))
            .map(|z| (z.title, z.start));
        self.zones = hazards::detect_zones(&track.hurdles);
        self.active_zone = selected.and_then(|(title, start)| {
            self.zones
                .iter()
                .position(|z| z.title == title && (z.start - start).abs() < 0.001)
        });
        self.recompute(track);
    }

    /// Should this hurdle stand out? False dims it on the track (story 012's
    /// filtered-out hurdles and story 010's zone focus).
    pub fn passes(&self, message: &Message) -> bool {
        if let Some(zone) = self.active_zone.and_then(|i| self.zones.get(i)) {
            if !zone.contains(message.id) {
                return false;
            }
        }
        match self.cache.get(message.id) {
            Some(meta) => self.filter.matches(message, meta),
            None => true,
        }
    }

    fn dimming(&self) -> bool {
        self.filter.is_active() || self.active_zone.is_some()
    }

    fn recompute(&mut self, track: &Track) {
        self.results = filter::search(
            track.hurdles.iter().map(|h| &h.message),
            &self.cache,
            &self.filter,
        );
        if let Some(zone) = self.active_zone.and_then(|i| self.zones.get(i)) {
            self.results.retain(|id| zone.contains(*id));
        }
    }

    // ---- input ----

    /// Offer a gesture (and this frame's keys) to the binoculars before the
    /// game acts on it.
    pub fn intercept(&mut self, gesture: Gesture, vp: &Viewport, track: &Track) -> Outcome {
        if let Some(action) = self.pending.take() {
            return Outcome::Handled(Some(action));
        }
        if self.open {
            return Outcome::Handled(self.overlay_input(gesture, vp, track));
        }
        if is_key_pressed(KeyCode::B) {
            self.open = true;
            self.recompute(track);
            return Outcome::Handled(None);
        }
        if is_key_pressed(KeyCode::R) {
            return Outcome::Handled(Some(Action::Relay));
        }
        if is_key_pressed(KeyCode::D) {
            return Outcome::Handled(Some(Action::CycleDifficulty));
        }
        if let Gesture::Tap(pos) = gesture {
            if self.button_rect(vp).contains(pos) {
                self.open = true;
                self.recompute(track);
                return Outcome::Handled(None);
            }
            for (rect, i) in self.banner_rects(vp) {
                if rect.contains(pos) {
                    return Outcome::Handled(self.toggle_zone(i, track));
                }
            }
            if let Some((rect, _)) = self.briefing_layout(vp) {
                if rect.contains(pos) {
                    self.active_zone = None;
                    self.recompute(track);
                    return Outcome::Handled(None);
                }
            }
        }
        Outcome::Pass(gesture)
    }

    /// Tap a hazard banner: select the zone (filtering the view to it and
    /// jumping the camera), or deselect when it is already active.
    fn toggle_zone(&mut self, i: usize, track: &Track) -> Option<Action> {
        if self.active_zone == Some(i) {
            self.active_zone = None;
            self.recompute(track);
            return None;
        }
        self.active_zone = Some(i);
        self.recompute(track);
        self.zones.get(i).map(|z| Action::JumpTo(z.start.max(0.0)))
    }

    fn overlay_input(&mut self, gesture: Gesture, vp: &Viewport, track: &Track) -> Option<Action> {
        let mut dirty = false;
        while let Some(c) = get_char_pressed() {
            if !c.is_control() && self.filter.query.chars().count() < MAX_QUERY {
                self.filter.query.push(c);
                dirty = true;
            }
        }
        if is_key_pressed(KeyCode::Backspace) && self.filter.query.pop().is_some() {
            dirty = true;
        }
        if is_key_pressed(KeyCode::Escape) {
            self.open = false;
        }
        let mut action = None;
        if is_key_pressed(KeyCode::Enter) {
            if let Some(&id) = self.results.first() {
                action = self.jump_to(id, track);
            }
        }
        if let Gesture::Tap(pos) = gesture {
            let geom = self.overlay_geom(vp);
            if geom.close.contains(pos) {
                self.open = false;
            } else if geom.reset.contains(pos) {
                self.filter.clear();
                self.active_zone = None;
                dirty = true;
            } else if pos.y < geom.panel.y {
                self.open = false;
            } else {
                for (rect, chip) in &geom.chips {
                    if rect.contains(pos) {
                        self.toggle_chip(*chip);
                        dirty = true;
                    }
                }
                for (i, rect) in geom.result_rows.iter().enumerate() {
                    if rect.contains(pos) {
                        if let Some(&id) = self.results.get(i) {
                            action = self.jump_to(id, track);
                        }
                    }
                }
            }
        }
        if dirty {
            self.recompute(track);
        }
        action
    }

    fn toggle_chip(&mut self, chip: Chip) {
        match chip {
            Chip::Channel(v) => filter::toggle(&mut self.filter.channels, v),
            Chip::Category(v) => filter::toggle(&mut self.filter.categories, v),
            Chip::Sentiment(v) => filter::toggle(&mut self.filter.sentiments, v),
            Chip::Urgency(v) => filter::toggle(&mut self.filter.urgencies, v),
            Chip::Status(v) => filter::toggle(&mut self.filter.statuses, v),
        }
    }

    fn chip_selected(&self, chip: Chip) -> bool {
        match chip {
            Chip::Channel(v) => self.filter.channels.contains(&v),
            Chip::Category(v) => self.filter.categories.contains(&v),
            Chip::Sentiment(v) => self.filter.sentiments.contains(&v),
            Chip::Urgency(v) => self.filter.urgencies.contains(&v),
            Chip::Status(v) => self.filter.statuses.contains(&v),
        }
    }

    /// Close the overlay and aim the camera at the hurdle. The filter stays
    /// active, so the rest of the track shows dimmed.
    fn jump_to(&mut self, id: u64, track: &Track) -> Option<Action> {
        self.open = false;
        track
            .hurdles
            .iter()
            .find(|h| h.message.id == id)
            .map(|h| Action::JumpTo(h.at))
    }

    // ---- geometry ----

    /// Round binoculars button: right edge, just above the track, in easy
    /// thumb reach.
    fn button_rect(&self, vp: &Viewport) -> Rect {
        let d = vp.w * 0.13;
        Rect::new(vp.w - d - vp.w * 0.04, vp.ground_y - d - vp.h * 0.05, d, d)
    }

    /// Screen rects of the visible zone banners, with lane stacking for
    /// overlapping zones.
    fn banner_rects(&self, vp: &Viewport) -> Vec<(Rect, usize)> {
        let bh = (vp.unit * 0.62).min(vp.h * 0.05);
        let base_y = vp.ground_y - vp.unit * 3.95;
        let mut lane_ends: Vec<f32> = Vec::new();
        let mut out = Vec::new();
        for (i, z) in self.zones.iter().enumerate() {
            let lane = lane_ends
                .iter()
                .position(|end| z.start >= *end)
                .unwrap_or_else(|| {
                    lane_ends.push(f32::NEG_INFINITY);
                    lane_ends.len() - 1
                });
            lane_ends[lane] = z.end;
            let x0 = vp.x_of(z.start);
            let x1 = vp.x_of(z.end);
            if x1 < -8.0 || x0 > vp.w + 8.0 {
                continue;
            }
            let y = base_y - lane as f32 * (bh + vp.h * 0.008);
            out.push((Rect::new(x0, y, x1 - x0, bh), i));
        }
        out
    }

    /// Briefing panel rect plus its word-wrapped lines, when a zone is active.
    fn briefing_layout(&self, vp: &Viewport) -> Option<(Rect, Vec<String>)> {
        let zone = self.active_zone.and_then(|i| self.zones.get(i))?;
        let pad = vp.w * 0.04;
        let fs = vp.h * 0.019;
        let inner_w = vp.w - 2.0 * pad - fs * 2.4;
        let mut lines = Vec::new();
        for text in zone.briefing() {
            lines.extend(wrap_text(&text, fs as u16, inner_w));
        }
        let h = fs * 2.4 + lines.len() as f32 * fs * 1.35 + fs * 2.0;
        // Sits over the HUD pill like a toast, clear of the zone banners.
        Some((Rect::new(pad, vp.h * 0.025, vp.w - 2.0 * pad, h), lines))
    }

    // ---- drawing ----

    /// World-anchored layer: hazard-zone bands along the track. Call after
    /// the hurdles are drawn.
    pub fn draw_world(&self, vp: &Viewport) {
        for (rect, i) in self.banner_rects(vp) {
            let zone = &self.zones[i];
            let selected = self.active_zone == Some(i);
            let mut band = view::ACCENT;
            band.a = if selected { 0.9 } else { 0.35 };
            view::rounded_rect(rect.x, rect.y, rect.w, rect.h, rect.h * 0.3, band);

            let label = zone.label();
            let vis_x0 = rect.x.max(0.0);
            let vis_x1 = (rect.x + rect.w).min(vp.w);
            // Fit the label to the visible part of the band, shrinking if needed.
            let mut fs = (rect.h * 0.62).max(vp.h * 0.015);
            let mut dims = measure_text(&label, None, fs as u16, 1.0);
            let avail = (vis_x1 - vis_x0 - fs).max(1.0);
            if dims.width + fs * 1.26 > avail {
                fs = (fs * avail / (dims.width + fs * 1.26)).max(vp.h * 0.012);
                dims = measure_text(&label, None, fs as u16, 1.0);
            }
            let tri = fs * 0.9;
            let total = tri * 1.4 + dims.width;
            let x0 = ((vis_x0 + vis_x1 - total) / 2.0)
                .clamp(vis_x0 + fs * 0.3, (vis_x1 - total).max(vis_x0 + fs * 0.3));
            warning_triangle(
                x0 + tri * 0.5,
                rect.y + rect.h * 0.5,
                tri,
                if selected { view::INK } else { view::GOLD },
            );
            draw_text(
                &label,
                x0 + tri * 1.4,
                rect.y + rect.h * 0.5 + dims.height * 0.5,
                fs,
                view::INK,
            );
        }
    }

    /// Screen-anchored layer: preset line, binoculars button, zone briefing,
    /// and the overlay. Call last so the overlay covers the HUD.
    pub fn draw_ui(&self, vp: &Viewport, track: &Track, spec: &CourseSpec) {
        self.draw_spec_line(vp, spec);
        self.draw_button(vp);
        self.draw_briefing(vp);
        if self.open {
            self.draw_overlay(vp, track);
        }
    }

    /// Course preset + dev controls, in the strip between track and card.
    fn draw_spec_line(&self, vp: &Viewport, spec: &CourseSpec) {
        let fs = vp.h * 0.0165;
        let text = format!(
            "{} · seed {} · R relay · D difficulty · B binoculars",
            spec.difficulty.label(),
            spec.seed
        );
        draw_text(&text, vp.w * 0.04, vp.ground_y + vp.h * 0.165, fs, view::INK_DIM);
    }

    fn draw_button(&self, vp: &Viewport) {
        let r = self.button_rect(vp);
        view::rounded_rect(r.x, r.y, r.w, r.h, r.w * 0.3, view::PANEL);
        let color = if self.dimming() { view::ACCENT } else { view::INK_DIM };
        let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.54);
        let lens_r = r.w * 0.16;
        let t = (r.w * 0.045).max(2.0);
        for dir in [-1.0f32, 1.0] {
            let lx = cx + dir * lens_r * 1.1;
            draw_circle_lines(lx, cy, lens_r, t, color);
            // Eyepiece stem rising from each lens.
            draw_line(lx, cy - lens_r, lx, cy - lens_r * 1.9, t, color);
        }
        draw_line(cx - lens_r * 0.35, cy - lens_r * 1.7, cx + lens_r * 0.35, cy - lens_r * 1.7, t, color);

        // Match count while a filter narrows the track.
        if self.dimming() && !self.open {
            let fs = vp.h * 0.016;
            let label = format!("{} in view", self.results.len());
            let dims = measure_text(&label, None, fs as u16, 1.0);
            draw_text(
                &label,
                r.x + (r.w - dims.width) / 2.0,
                r.y + r.h + fs * 1.2,
                fs,
                view::ACCENT,
            );
        }
    }

    fn draw_briefing(&self, vp: &Viewport) {
        if self.open {
            return;
        }
        let Some((rect, lines)) = self.briefing_layout(vp) else {
            return;
        };
        let Some(zone) = self.active_zone.and_then(|i| self.zones.get(i)) else {
            return;
        };
        let fs = vp.h * 0.019;
        view::rounded_rect(rect.x, rect.y, rect.w, rect.h, vp.w * 0.02, view::PANEL);
        draw_rectangle(rect.x, rect.y, vp.w * 0.012, rect.h, view::ACCENT);
        let x = rect.x + fs * 1.2;
        warning_triangle(x + fs * 0.45, rect.y + fs * 1.35, fs * 0.9, view::GOLD);
        draw_text(&zone.label(), x + fs * 1.3, rect.y + fs * 1.7, fs * 1.1, view::GOLD);
        let mut y = rect.y + fs * 3.2;
        for line in &lines {
            draw_text(line, x, y, fs, view::INK);
            y += fs * 1.35;
        }
        draw_text(
            "tap the banner again to dismiss",
            x,
            rect.y + rect.h - fs * 0.7,
            fs * 0.85,
            view::INK_DIM,
        );
    }

    fn overlay_geom(&self, vp: &Viewport) -> OverlayGeom {
        let panel = Rect::new(vp.w * 0.03, vp.h * 0.115, vp.w * 0.94, vp.h * 0.87);
        let pad = vp.w * 0.035;
        let x = panel.x + pad;
        let inner_w = panel.w - 2.0 * pad;

        let btn_h = vp.h * 0.042;
        let close = Rect::new(panel.x + panel.w - pad - vp.w * 0.16, panel.y + vp.h * 0.018, vp.w * 0.16, btn_h);
        let reset = Rect::new(close.x - vp.w * 0.17, close.y, vp.w * 0.16, btn_h);
        let query = Rect::new(x, panel.y + vp.h * 0.075, inner_w, vp.h * 0.048);

        let mut chips = Vec::new();
        let label_w = inner_w * 0.145;
        let rows: [(&str, Vec<Chip>); 5] = [
            ("channel", Channel::ALL.iter().map(|c| Chip::Channel(*c)).collect()),
            ("type", Category::ALL.iter().map(|c| Chip::Category(*c)).collect()),
            ("mood", Sentiment::ALL.iter().map(|s| Chip::Sentiment(*s)).collect()),
            ("height", Urgency::ALL.iter().map(|u| Chip::Urgency(*u)).collect()),
            (
                "status",
                vec![
                    Chip::Status(MessageStatus::Open),
                    Chip::Status(MessageStatus::Cleared),
                    Chip::Status(MessageStatus::Skipped),
                ],
            ),
        ];
        let row_h = vp.h * 0.047;
        let chip_h = vp.h * 0.036;
        let rows_y = query.y + query.h + vp.h * 0.018;
        let mut row_labels = Vec::new();
        for (r, (label, row)) in rows.into_iter().enumerate() {
            let y = rows_y + r as f32 * row_h;
            row_labels.push((label, x, y + chip_h * 0.7));
            let gap = vp.w * 0.008;
            let chip_w = (inner_w - label_w - gap * row.len() as f32) / row.len() as f32;
            for (c, chip) in row.into_iter().enumerate() {
                let cx = x + label_w + c as f32 * (chip_w + gap);
                chips.push((Rect::new(cx, y, chip_w, chip_h), chip));
            }
        }

        let results_y = rows_y + 5.0 * row_h + vp.h * 0.012;
        let row_h = vp.h * 0.082;
        let mut result_rows = Vec::new();
        for i in 0..MAX_RESULTS {
            let y = results_y + vp.h * 0.03 + i as f32 * (row_h + vp.h * 0.006);
            if y + row_h > panel.y + panel.h - vp.h * 0.012 {
                break;
            }
            result_rows.push(Rect::new(x, y, inner_w, row_h));
        }

        OverlayGeom {
            panel,
            close,
            reset,
            query,
            chips,
            row_labels,
            results_y,
            result_rows,
        }
    }

    fn draw_overlay(&self, vp: &Viewport, track: &Track) {
        let geom = self.overlay_geom(vp);
        draw_rectangle(0.0, 0.0, vp.w, vp.h, Color::new(0.0, 0.0, 0.0, 0.55));
        view::rounded_rect(geom.panel.x, geom.panel.y, geom.panel.w, geom.panel.h, vp.w * 0.03, view::PANEL);

        let fs = vp.h * 0.026;
        draw_text("Binoculars", geom.panel.x + vp.w * 0.035, geom.panel.y + fs * 1.6, fs, view::GOLD);
        for (rect, label) in [(&geom.reset, "reset"), (&geom.close, "close")] {
            view::rounded_rect(rect.x, rect.y, rect.w, rect.h, rect.h * 0.5, view::TRACK_EDGE);
            let cfs = vp.h * 0.018;
            let dims = measure_text(label, None, cfs as u16, 1.0);
            draw_text(
                label,
                rect.x + (rect.w - dims.width) / 2.0,
                rect.y + rect.h * 0.5 + dims.height * 0.5,
                cfs,
                view::INK,
            );
        }

        // Search box with a blinking caret.
        let q = &geom.query;
        view::rounded_rect(q.x, q.y, q.w, q.h, q.h * 0.3, view::SKY);
        let qfs = vp.h * 0.021;
        let caret = if get_time() % 1.0 < 0.5 { "|" } else { "" };
        if self.filter.query.is_empty() {
            draw_text(
                &format!("type to search sender or text{caret}"),
                q.x + qfs * 0.7,
                q.y + q.h * 0.5 + qfs * 0.4,
                qfs,
                view::INK_DIM,
            );
        } else {
            draw_text(
                &format!("{}{caret}", self.filter.query),
                q.x + qfs * 0.7,
                q.y + q.h * 0.5 + qfs * 0.4,
                qfs,
                view::INK,
            );
        }

        // Filter chips.
        let lfs = vp.h * 0.0165;
        for (label, x, y) in &geom.row_labels {
            draw_text(label, *x, *y, lfs, view::INK_DIM);
        }
        for (rect, chip) in &geom.chips {
            let selected = self.chip_selected(*chip);
            let bg = if selected { view::ACCENT } else { view::TRACK_EDGE };
            view::rounded_rect(rect.x, rect.y, rect.w, rect.h, rect.h * 0.5, bg);
            let label = chip_label(*chip);
            let dims = measure_text(label, None, lfs as u16, 1.0);
            draw_text(
                label,
                rect.x + (rect.w - dims.width) / 2.0,
                rect.y + rect.h * 0.5 + dims.height * 0.5,
                lfs,
                if selected { view::INK } else { view::INK_DIM },
            );
        }

        // Results header + scout-report rows.
        let hfs = vp.h * 0.019;
        let header = match self.results.len() {
            0 => "no matches · loosen the filters".to_string(),
            1 => "1 match · tap to jump".to_string(),
            n => format!("{n} matches · tap one to jump"),
        };
        let hx = geom.panel.x + vp.w * 0.035;
        draw_text(&header, hx, geom.results_y + hfs, hfs, view::INK);

        for (i, rect) in geom.result_rows.iter().enumerate() {
            let Some(&id) = self.results.get(i) else { break };
            let Some(hurdle) = track.hurdles.iter().find(|h| h.message.id == id) else {
                continue;
            };
            let m = &hurdle.message;
            view::rounded_rect(rect.x, rect.y, rect.w, rect.h, vp.w * 0.015, view::TRACK);
            let color = view::channel_color(m.channel);
            draw_rectangle(rect.x, rect.y, vp.w * 0.010, rect.h, color);
            let ifs = vp.h * 0.018;
            view::channel_icon(rect.x + ifs * 1.4, rect.y + rect.h * 0.32, ifs * 1.2, m.channel, color);
            let tx = rect.x + ifs * 2.5;
            let max_w = rect.w - ifs * 3.0;
            let head = match self.cache.get(id) {
                Some(meta) => format!(
                    "{} · {} · {}",
                    m.sender,
                    meta.category.label(),
                    meta.urgency.label()
                ),
                None => m.sender.clone(),
            };
            let status = match m.status {
                MessageStatus::Open => None,
                MessageStatus::Cleared => Some("cleared"),
                MessageStatus::Skipped => Some("skipped"),
            };
            draw_text(&fit_text(&head, ifs as u16, max_w), tx, rect.y + rect.h * 0.38, ifs, view::INK);
            if let Some(tag) = status {
                let tfs = ifs * 0.85;
                let dims = measure_text(tag, None, tfs as u16, 1.0);
                draw_text(tag, rect.x + rect.w - dims.width - ifs * 0.6, rect.y + rect.h * 0.38, tfs, view::GOLD);
            }
            // Scout report: the AI summary of the message (story 006 output).
            let summary = self.cache.summary(id).unwrap_or_default().to_string();
            let sfs = vp.h * 0.016;
            draw_text(
                &fit_text(&summary, sfs as u16, max_w),
                tx,
                rect.y + rect.h * 0.72,
                sfs,
                view::INK_DIM,
            );
        }
    }
}

struct OverlayGeom {
    panel: Rect,
    close: Rect,
    reset: Rect,
    query: Rect,
    chips: Vec<(Rect, Chip)>,
    row_labels: Vec<(&'static str, f32, f32)>,
    results_y: f32,
    result_rows: Vec<Rect>,
}

fn chip_label(chip: Chip) -> &'static str {
    match chip {
        Chip::Channel(c) => c.label(),
        Chip::Category(c) => c.label(),
        Chip::Sentiment(s) => s.label(),
        Chip::Urgency(u) => u.label(),
        Chip::Status(MessageStatus::Open) => "Open",
        Chip::Status(MessageStatus::Cleared) => "Cleared",
        Chip::Status(MessageStatus::Skipped) => "Skipped",
    }
}

/// Filled warning triangle with an exclamation mark, built from primitives.
fn warning_triangle(cx: f32, cy: f32, s: f32, color: Color) {
    let h = s * 0.95;
    draw_triangle(
        vec2(cx, cy - h * 0.55),
        vec2(cx - s * 0.55, cy + h * 0.45),
        vec2(cx + s * 0.55, cy + h * 0.45),
        color,
    );
    let fs = s * 0.85;
    draw_text("!", cx - fs * 0.13, cy + h * 0.38, fs, view::SKY);
}

/// Truncate `text` with an ellipsis so it fits `max_w` at font size `fs`.
fn fit_text(text: &str, fs: u16, max_w: f32) -> String {
    if measure_text(text, None, fs, 1.0).width <= max_w {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() {
        cut.pop();
        let candidate = format!("{}...", cut.trim_end());
        if measure_text(&candidate, None, fs, 1.0).width <= max_w {
            return candidate;
        }
    }
    String::new()
}

/// Greedy word wrap by measured width.
fn wrap_text(text: &str, fs: u16, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if measure_text(&candidate, None, fs, 1.0).width <= max_w || line.is_empty() {
            line = candidate;
        } else {
            lines.push(line);
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
