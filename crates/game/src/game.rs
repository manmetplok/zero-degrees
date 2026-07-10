//! The playing state: wires gestures to the track model and draws the run.
//! Layout is resolution-independent: everything scales from screen size, per
//! macroquad's logical-pixel coordinates (`high_dpi` handles physical pixels).

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::MessageStatus;

use crate::assets::{Assets, GROUND_FRAC};
use crate::inbox;
use crate::input::{Gesture, GestureDetector};
use crate::track::Track;
use crate::view;

const JUMP_TIME: f32 = 0.55;
const JUMP_SPAN: f32 = 2.4; // track units covered by one jump
const APPROACH_GAP: f32 = 1.2; // where the runner stops before a hurdle

enum Phase {
    Waiting,
    Running,
    /// Running to the next hurdle with a jump queued on arrival.
    ApproachJump,
    Jumping {
        t: f32,
        from: f32,
    },
    Celebrating,
}

pub struct Game {
    assets: Assets,
    track: Track,
    phase: Phase,
    gestures: GestureDetector,
    cam_x: f32,
    /// Seconds left before the camera snaps back to the runner after a drag.
    scroll_hold: f32,
    facing_left: bool,
    /// Drop-in animation start time per hurdle (message id -> get_time()).
    spawned_at: HashMap<u64, f64>,
    cleared: u32,
}

struct Layout {
    w: f32,
    h: f32,
    /// Pixels per track unit.
    unit: f32,
    ground_y: f32,
    /// Screen x where the runner sits while the camera follows.
    anchor_x: f32,
}

impl Layout {
    fn current() -> Self {
        let w = screen_width();
        let h = screen_height();
        Self {
            w,
            h,
            unit: w / 7.2,
            ground_y: h * 0.60,
            anchor_x: w * 0.30,
        }
    }

    fn x_of(&self, track_x: f32, cam_x: f32) -> f32 {
        (track_x - cam_x) * self.unit + self.anchor_x
    }
}

impl Game {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            track: Track::new(inbox::sample_messages(8)),
            phase: Phase::Waiting,
            gestures: GestureDetector::new(),
            cam_x: 0.0,
            scroll_hold: 0.0,
            facing_left: false,
            spawned_at: HashMap::new(),
            cleared: 0,
        }
    }

    pub fn frame(&mut self) {
        let dt = get_frame_time();
        let lay = Layout::current();
        self.update(dt, &lay);
        self.draw(&lay);
    }

    fn update(&mut self, dt: f32, lay: &Layout) {
        match self.gestures.poll() {
            Gesture::Tap(pos) => {
                if self.ingest_button_rect(lay).contains(pos) {
                    self.simulate_incoming();
                } else {
                    self.approach();
                }
            }
            Gesture::SwipeUp => self.clear_hurdle(),
            Gesture::SwipeLeft => self.skip_hurdle(),
            Gesture::SwipeRight => self.approach(),
            Gesture::Drag(dx) => {
                self.cam_x = (self.cam_x - dx / lay.unit).clamp(-1.0, self.track.finish_at());
                self.scroll_hold = 1.5;
            }
            Gesture::None => {}
        }
        if is_key_pressed(KeyCode::N) {
            self.simulate_incoming();
        }

        // Runner physics.
        match &mut self.phase {
            Phase::Jumping { t, from } => {
                *t += dt / JUMP_TIME;
                let from = *from;
                if *t >= 1.0 {
                    self.track.runner_at = from + JUMP_SPAN;
                    self.track.resolve_next(MessageStatus::Cleared);
                    self.cleared += 1;
                    self.phase = Phase::Running;
                } else {
                    self.track.runner_at = from + JUMP_SPAN * *t;
                }
            }
            _ => {
                let target = self.track.runner_target;
                if target < self.track.runner_at - 0.01 {
                    self.facing_left = true;
                } else if target > self.track.runner_at + 0.01 {
                    self.facing_left = false;
                }
                let moving = self.track.advance(dt);
                if !moving {
                    self.phase = match self.phase {
                        Phase::ApproachJump => Phase::Jumping {
                            t: 0.0,
                            from: self.track.runner_at,
                        },
                        _ if self.track.remaining() == 0
                            && self.track.runner_at >= self.track.finish_at() - 0.01 =>
                        {
                            Phase::Celebrating
                        }
                        Phase::Running => Phase::Waiting,
                        _ => return self.follow_camera(dt),
                    };
                }
            }
        }
        self.follow_camera(dt);
    }

    fn follow_camera(&mut self, dt: f32) {
        if self.scroll_hold > 0.0 {
            self.scroll_hold -= dt;
            return;
        }
        let goal = self.track.runner_at;
        self.cam_x += (goal - self.cam_x) * (1.0 - (-6.0 * dt).exp());
    }

    fn approach(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Celebrating) {
            return;
        }
        self.scroll_hold = 0.0;
        if let Some(h) = self.track.next_hurdle() {
            self.track.runner_target = h.at - APPROACH_GAP;
            self.phase = Phase::Running;
        }
    }

    fn clear_hurdle(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Running) {
            return;
        }
        self.scroll_hold = 0.0;
        if let Some(h) = self.track.next_hurdle() {
            self.track.runner_target = h.at - APPROACH_GAP;
            self.phase = Phase::ApproachJump;
        }
    }

    fn skip_hurdle(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Running) {
            return;
        }
        self.scroll_hold = 0.0;
        self.track.resolve_next(MessageStatus::Skipped);
        self.phase = Phase::Running;
    }

    /// Scenario: new message ingested mid-run drops onto the track ahead.
    fn simulate_incoming(&mut self) {
        let existing: Vec<_> = self.track.hurdles.iter().map(|h| h.message.clone()).collect();
        let msg = inbox::next_incoming(&existing);
        let id = msg.id;
        self.track.add_message(msg);
        self.spawned_at.insert(id, get_time());
        if matches!(self.phase, Phase::Celebrating) {
            self.phase = Phase::Waiting;
        }
    }

    fn ingest_button_rect(&self, lay: &Layout) -> Rect {
        let d = lay.w * 0.13;
        Rect::new(lay.w - d - lay.w * 0.04, lay.h * 0.03, d, d)
    }

    // ---- drawing ----

    fn draw(&self, lay: &Layout) {
        clear_background(view::SKY);
        self.draw_track(lay);
        self.draw_hurdles(lay);
        self.draw_runner(lay);
        self.draw_hud(lay);
        self.draw_card(lay);
    }

    fn draw_track(&self, lay: &Layout) {
        let track_h = lay.h * 0.14;
        draw_rectangle(0.0, lay.ground_y, lay.w, track_h, view::TRACK);
        draw_rectangle(0.0, lay.ground_y, lay.w, lay.unit * 0.06, view::TRACK_EDGE);
        draw_rectangle(
            0.0,
            lay.ground_y + track_h,
            lay.w,
            lay.h - lay.ground_y - track_h,
            view::PANEL,
        );
        // Lane dashes scroll with the world.
        let dash_w = lay.unit * 0.9;
        let period = lay.unit * 2.0;
        let offset = (self.cam_x * lay.unit) % period;
        let y = lay.ground_y + track_h * 0.55;
        let mut x = -offset - period;
        while x < lay.w + period {
            draw_rectangle(x, y, dash_w, lay.unit * 0.05, view::TRACK_EDGE);
            x += period;
        }
        view::finish_line(
            lay.x_of(self.track.finish_at(), self.cam_x),
            lay.ground_y,
            lay.unit,
        );
    }

    fn draw_hurdles(&self, lay: &Layout) {
        let now = get_time();
        for h in &self.track.hurdles {
            let mut x = lay.x_of(h.at, self.cam_x);
            if x < -lay.unit * 2.0 || x > lay.w + lay.unit * 2.0 {
                continue;
            }
            let mut drop = 0.0;
            if let Some(t0) = self.spawned_at.get(&h.message.id) {
                let p = ((now - t0) / 0.7).clamp(0.0, 1.0) as f32;
                drop = -(1.0 - p) * (1.0 - p) * lay.h * 0.5;
                x += (1.0 - p) * lay.unit * 0.3;
            }
            let style = view::HurdleStyle {
                color: view::channel_color(h.message.channel),
                faded: h.message.status != MessageStatus::Open,
                down: h.message.status == MessageStatus::Cleared,
                marked: h.message.status == MessageStatus::Skipped,
            };
            view::hurdle(x, lay.ground_y + drop, lay.unit, h.message.channel, &style);
        }
    }

    fn draw_runner(&self, lay: &Layout) {
        let (strip, src) = match &self.phase {
            Phase::Jumping { t, .. } => (&self.assets.jump, self.assets.jump.source_at(*t)),
            Phase::Celebrating => (&self.assets.wave, self.assets.wave.source(get_time() as f32)),
            Phase::Waiting => (&self.assets.idle, self.assets.idle.source(get_time() as f32)),
            _ => (&self.assets.run, self.assets.run.source(get_time() as f32)),
        };
        let quad = lay.unit * 3.2;
        let x = lay.x_of(self.track.runner_at, self.cam_x) - quad / 2.0;
        let mut y = lay.ground_y - quad * (1.0 - GROUND_FRAC);
        if let Phase::Jumping { t, .. } = self.phase {
            y -= (std::f32::consts::PI * t).sin() * lay.unit * 1.6;
        }
        draw_texture_ex(
            &strip.texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(quad, quad)),
                source: Some(src),
                flip_x: self.facing_left,
                ..Default::default()
            },
        );
    }

    fn draw_hud(&self, lay: &Layout) {
        let fs = lay.h * 0.032;
        let remaining = self.track.remaining();
        let label = match remaining {
            0 => "Inbox zero!".to_string(),
            1 => "1 hurdle to go".to_string(),
            n => format!("{n} hurdles to go"),
        };
        let pad = lay.w * 0.04;
        let dims = measure_text(&label, None, fs as u16, 1.0);
        view::rounded_rect(
            pad,
            lay.h * 0.03,
            dims.width + fs * 1.6,
            fs * 1.8,
            fs * 0.9,
            view::PANEL,
        );
        draw_text(&label, pad + fs * 0.8, lay.h * 0.03 + fs * 1.25, fs, view::INK);

        // Cleared counter under the pill.
        let sub = format!("cleared {}", self.cleared);
        draw_text(&sub, pad + fs * 0.2, lay.h * 0.03 + fs * 2.9, fs * 0.8, view::INK_DIM);

        // Simulate-incoming button (demo control, doubles as the ingest hook).
        let btn = self.ingest_button_rect(lay);
        view::rounded_rect(btn.x, btn.y, btn.w, btn.h, btn.w * 0.3, view::PANEL);
        view::channel_icon(
            btn.x + btn.w * 0.44,
            btn.y + btn.h * 0.5,
            btn.w * 0.42,
            shared::Channel::Email,
            view::INK_DIM,
        );
        let plus = btn.w * 0.30;
        draw_text(
            "+",
            btn.x + btn.w * 0.70,
            btn.y + btn.h * 0.44,
            plus,
            view::ACCENT,
        );
    }

    fn draw_card(&self, lay: &Layout) {
        let pad = lay.w * 0.04;
        let card_y = lay.h * 0.78;
        let card_h = lay.h - card_y - pad;
        view::rounded_rect(pad, card_y, lay.w - 2.0 * pad, card_h, lay.w * 0.03, view::PANEL);

        let fs = lay.h * 0.026;
        let x = pad * 2.2;
        match self.track.next_hurdle() {
            Some(h) => {
                let color = view::channel_color(h.message.channel);
                draw_rectangle(pad, card_y, lay.w * 0.012, card_h, color);
                view::channel_icon(x + fs * 0.6, card_y + fs * 1.9, fs * 1.3, h.message.channel, color);
                draw_text(
                    &format!("{}  ·  {}", h.message.channel.label(), h.message.sender),
                    x + fs * 1.8,
                    card_y + fs * 2.3,
                    fs * 0.9,
                    view::INK_DIM,
                );
                draw_text(&h.message.subject, x, card_y + fs * 4.3, fs * 1.15, view::INK);
                draw_text(
                    "tap: approach    swipe up: clear    swipe left: skip",
                    x,
                    card_y + card_h - fs * 0.9,
                    fs * 0.8,
                    view::INK_DIM,
                );
            }
            None => {
                draw_text("Inbox zero — run for the finish!", x, card_y + fs * 3.0, fs * 1.2, view::INK);
                draw_text(
                    "tap the mail button to simulate a new message",
                    x,
                    card_y + fs * 5.0,
                    fs * 0.85,
                    view::INK_DIM,
                );
            }
        }
    }
}
