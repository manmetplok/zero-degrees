//! The playing state: wires gestures to the track model and draws the run.
//! Layout is resolution-independent: everything scales from screen size, per
//! macroquad's logical-pixel coordinates (`high_dpi` handles physical pixels).

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::MessageStatus;

use crate::assets::{Assets, GROUND_FRAC};
use crate::binoculars::{self, Binoculars, Viewport};
use crate::card::{CardAction, CardHost};
use crate::hub::{Hub, TrackGeom};
use crate::inbox;
use crate::input::{Gesture, GestureDetector};
use crate::meta;
use crate::profile::Profile;
use crate::progress::{self, Day};
use crate::score::Score;
use crate::team;
use crate::track::Track;
use crate::view;

const JUMP_TIME: f32 = 0.55;
const JUMP_SPAN: f32 = 2.4; // track units covered by one jump
const APPROACH_GAP: f32 = 1.2; // where the runner stops before a hurdle
const POPUP_LIFE: f64 = 1.1; // seconds a score pop-up stays on screen

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
    score: Score,
    popups: Vec<Popup>,
    /// Manual category overrides (story 003), persisted locally.
    overrides: meta::Overrides,
    /// Response times of cleared hurdles (story 014); read by race control.
    pub responses: meta::ResponseLog,
    /// Hurdle detail overlay + AI coach feedback (stories 006-008, 020).
    cards: CardHost,
    /// A tapped hurdle opens its card once the runner arrives (story 006).
    card_pending: bool,
    /// Persisted progression (streaks, trophies) and its UI overlays.
    profile: Profile,
    /// Team screens, navigation, and the mock team simulation (hub.rs).
    hub: Hub,
    /// Which demo course is laid out (seed + difficulty preset), story 002.
    course: inbox::CourseSpec,
    /// Search/filter overlay and hazard zones (stories 010 and 012).
    lens: Binoculars,
}

/// Floating "+XP" text rising from a cleared hurdle.
struct Popup {
    text: String,
    /// Track position the pop-up rises from.
    world_x: f32,
    /// get_time() when spawned.
    at: f64,
}

/// Mock world clock: fake unix time advancing with the session, anchored
/// just past the newest sample message. Progression derives "today" from
/// this instead of the wall clock (see progress.rs).
fn world_now() -> i64 {
    progress::MOCK_WORLD_EPOCH + get_time() as i64
}

/// "2" for whole multipliers, "1.5" otherwise.
fn mult_label(m: f32) -> String {
    if m.fract() == 0.0 {
        format!("{}", m as u32)
    } else {
        format!("{m:.1}")
    }
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
        let course = inbox::CourseSpec::from_env();
        let track = Track::new(inbox::generate_course(&course), meta::demo_now(0.0));
        let lens = Binoculars::new(&track);
        Self {
            assets,
            track,
            phase: Phase::Waiting,
            gestures: GestureDetector::new(),
            cam_x: 0.0,
            scroll_hold: 0.0,
            facing_left: false,
            spawned_at: HashMap::new(),
            cleared: 0,
            score: Score::new(),
            popups: Vec::new(),
            overrides: meta::Overrides::load_default(),
            responses: meta::ResponseLog::default(),
            cards: CardHost::new(),
            card_pending: false,
            profile: Profile::load(Day::from_unix(world_now())),
            hub: Hub::new(),
            course,
            lens,
        }
    }

    /// AI-coach feedback ratios for race control (story 020).
    #[allow(dead_code)] // read by the race-control screen (story 011)
    pub fn feedback(&self) -> &crate::feedback::FeedbackStore {
        self.cards.feedback()
    }

    fn viewport(&self, lay: &Layout) -> Viewport {
        Viewport {
            w: lay.w,
            h: lay.h,
            unit: lay.unit,
            ground_y: lay.ground_y,
            anchor_x: lay.anchor_x,
            cam_x: self.cam_x,
        }
    }

    /// Apply a binoculars interaction (camera jump, course relay/preset).
    fn apply_lens(&mut self, action: binoculars::Action) {
        match action {
            binoculars::Action::JumpTo(x) => {
                self.cam_x = x.clamp(-1.0, self.track.finish_at());
                self.scroll_hold = 2.5;
            }
            binoculars::Action::Relay => {
                self.course = self.course.reseeded();
                self.rebuild_course();
            }
            binoculars::Action::CycleDifficulty => {
                self.course.difficulty = self.course.difficulty.next();
                self.rebuild_course();
            }
        }
    }

    /// The story-002 reset flag: clear the old course and lay out a new one.
    fn rebuild_course(&mut self) {
        self.track = Track::new(inbox::generate_course(&self.course), meta::demo_now(get_time()));
        self.phase = Phase::Waiting;
        self.cam_x = 0.0;
        self.scroll_hold = 0.0;
        self.spawned_at.clear();
        self.popups.clear();
        self.cleared = 0;
        self.score = Score::new();
        self.lens.refresh(&self.track);
    }

    /// Scripted actions for the ZD_DEMO dev harness (see main.rs): walks the
    /// full card flow — open card, power-up draft, recharge with a steering
    /// note, swipe-up send — then the progression overlays (trophy
    /// celebration, trophy room), without real input. Frame numbers leave
    /// slack for the wall-clock walk-up and charge animations at 60 or 120
    /// fps, so the shots land on the scout card, the revealed draft, the
    /// jump, the celebration, and the trophy room.
    pub fn demo_tick(&mut self, frame: u32) {
        match frame {
            20 => self.clear_hurdle(), // now opens the review card on arrival
            165 => self.cards.demo_power_up(get_time()),
            330 => self.cards.demo_recharge("keep it short", get_time()),
            480 => {
                let action = self.cards.demo_send(get_time());
                self.apply_card_action(action);
            }
            560 => self.skip_hurdle(),
            575 => self.simulate_incoming(),
            640 => self.profile.demo_celebrate(get_time()),
            780 => {
                self.profile.dismiss(get_time());
                self.profile.toggle_room();
            }
            _ => {}
        }
    }

    pub fn frame(&mut self) {
        let dt = get_frame_time();
        let lay = Layout::current();
        // Team simulation + screen routing (hub.rs). When a non-track screen
        // is active the hub owns the whole frame.
        if self.hub.frame(dt, &mut self.track, &self.score, &mut self.gestures) {
            return;
        }
        self.update(dt, &lay);
        self.draw(&lay);
        self.hub.track_overlay(lay.w, lay.h);
    }

    fn update(&mut self, dt: f32, lay: &Layout) {
        // Progression bookkeeping; while the trophy room or a celebration is
        // up, any tap or key dismisses it and gameplay input is swallowed.
        let now = get_time();
        self.profile.tick(Day::from_unix(world_now()), now);
        if self.profile.overlay_active() {
            if matches!(self.gestures.poll(), Gesture::Tap(_))
                || is_key_pressed(KeyCode::Space)
                || is_key_pressed(KeyCode::P)
                || is_key_pressed(KeyCode::Escape)
            {
                self.profile.dismiss(now);
            }
            self.follow_camera(dt);
            self.popups.retain(|p| now - p.at < POPUP_LIFE);
            return;
        }
        // While the detail card is open it captures all input and the run
        // idles underneath (stories 006-008). An approved send fires the
        // jump via apply_card_action; backing out changes nothing.
        if self.cards.is_open() {
            let action = self.cards.handle(self.gestures.poll(), lay.w, lay.h, get_time());
            self.cards.poll_keys(get_time());
            self.apply_card_action(action);
            self.follow_camera(dt);
            return;
        }
        // The binoculars get first pick of gestures and keys: overlay input,
        // zone-banner taps, and course controls happen there.
        let vp = self.viewport(lay);
        let gesture = match self.lens.intercept(self.gestures.poll(), &vp, &self.track) {
            binoculars::Outcome::Pass(g) => g,
            binoculars::Outcome::Handled(action) => {
                if let Some(action) = action {
                    self.apply_lens(action);
                }
                Gesture::None
            }
        };
        match gesture {
            Gesture::Tap(pos) => {
                let geom = TrackGeom {
                    w: lay.w,
                    h: lay.h,
                    unit: lay.unit,
                    anchor_x: lay.anchor_x,
                    ground_y: lay.ground_y,
                    cam_x: self.cam_x,
                };
                if self.hub.track_tap(pos, geom, &mut self.track) {
                    // Consumed by nav, filter chrome, or hurdle assignment.
                } else if self.profile.handle_tap(pos, lay.w, lay.h) {
                    // Streak pill tapped: profile opened its trophy room.
                } else if self.ingest_button_rect(lay).contains(pos) {
                    self.simulate_incoming();
                } else if self.retype_chip_rect(lay).is_some_and(|r| r.contains(pos)) {
                    self.retype_faced();
                } else {
                    // Story 006: tapping walks up to the faced hurdle and
                    // opens its detail card on arrival (try_open_card).
                    self.approach();
                    self.card_pending = true;
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
        // Desktop dev shortcuts mirroring the touch gestures. Disabled while
        // the binoculars overlay owns the keyboard for text search.
        if !self.lens.is_open() {
            if is_key_pressed(KeyCode::N) {
                self.simulate_incoming();
            }
            if is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Right) {
                self.approach();
            }
            if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Up) {
                self.clear_hurdle();
            }
            if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Left) {
                self.skip_hurdle();
            }
        }
        if is_key_pressed(KeyCode::T) {
            self.retype_faced();
        }
        if is_key_pressed(KeyCode::P) {
            self.profile.toggle_room();
        }
        if is_key_pressed(KeyCode::F) {
            self.track.free_run = !self.track.free_run;
        }

        // Priority layout: relayout as ages change, ease hurdles to slots.
        self.track.tick(dt, meta::demo_now(get_time()));

        // A queued detail card opens once the runner stands at the hurdle.
        if self.card_pending {
            self.try_open_card();
        }

        // Runner physics.
        match &mut self.phase {
            Phase::Jumping { t, from } => {
                *t += dt / JUMP_TIME;
                let (t, from) = (*t, *from);
                if t >= 1.0 {
                    self.track.runner_at = from + JUMP_SPAN;
                    self.award_clear();
                    self.phase = Phase::Running;
                } else {
                    self.track.runner_at = from + JUMP_SPAN * t;
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
        let now = get_time();
        self.popups.retain(|p| now - p.at < POPUP_LIFE);
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

    /// Story 008: clearing a hurdle means reviewing and sending a real
    /// reply, so a bare swipe-up no longer resolves anything — it opens the
    /// hurdle's card for review. The jump + score fire from
    /// `apply_card_action` once the card reports an approved send.
    fn clear_hurdle(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Running) {
            return;
        }
        self.scroll_hold = 0.0;
        if self.track.next_hurdle().is_some() {
            self.approach();
            self.card_pending = true;
        }
    }

    /// Open the queued detail card once the runner stands before the faced
    /// hurdle, walking it over first when it stopped elsewhere (story 006).
    fn try_open_card(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Running) {
            self.card_pending = false;
            return;
        }
        let Some(h) = self.track.next_hurdle() else {
            self.card_pending = false;
            return;
        };
        let stop_at = h.at - APPROACH_GAP;
        if (self.track.runner_at - stop_at).abs() < 0.05 {
            let message = h.message.clone();
            self.cards.open_for(&message, get_time());
            self.card_pending = false;
        } else if (self.track.runner_target - stop_at).abs() > 0.01 {
            // Not headed to the hurdle yet (e.g. stopped at an auto-run
            // point): walk over; the card opens on arrival.
            self.track.runner_target = stop_at;
            self.phase = Phase::Running;
        }
    }

    /// React to a card decision (story 008): an approved send resolves the
    /// hurdle through the normal jump, so animation and score happen only
    /// now — never when the draft was generated.
    fn apply_card_action(&mut self, action: CardAction) {
        if action == CardAction::Send {
            if let Some(h) = self.track.next_hurdle() {
                self.track.runner_target = h.at - APPROACH_GAP;
                self.phase = Phase::ApproachJump;
            }
        }
    }

    /// Resolve the faced hurdle as cleared and award XP: base scales with
    /// urgency (story 004), a speed bonus or late partial applies against the
    /// response target (story 014), then the combo rules (story 015).
    fn award_clear(&mut self) {
        let now = get_time();
        let now_ts = meta::demo_now(now);
        let Some(h) = self.track.next_hurdle() else {
            return;
        };
        let (hurdle_at, id, received_at) = (h.at, h.message.id, h.message.received_at);
        let m = meta::enriched(&h.message, &self.overrides);
        if self.track.resolve_next(MessageStatus::Cleared, now_ts).is_none() {
            return;
        }
        self.cleared += 1;
        self.lens.refresh(&self.track);
        let waited = now_ts - received_at;
        let on_time = !meta::is_overdue(m.urgency, waited);
        self.responses.record(meta::ResponseRecord {
            message_id: id,
            category: m.category,
            urgency: m.urgency,
            waited_secs: waited,
            on_time,
        });
        let reward = self.score.on_clear(meta::clear_xp(m.urgency, waited), now);
        // Progression (stories 016/018): burning state and response time come
        // from the real triage data (story 014).
        let event = progress::ClearEvent {
            message_id: id,
            urgency: m.urgency,
            sentiment: m.sentiment,
            was_burning: !on_time,
            response_seconds: waited as f64,
            track_cleared: self.track.remaining() == 0,
            at: world_now(),
        };
        self.profile.on_clear(&event, reward.xp, now);
        let mut text = if reward.multiplier > 1.0 {
            format!("+{} XP  x{}", reward.xp, mult_label(reward.multiplier))
        } else {
            format!("+{} XP", reward.xp)
        };
        text.push_str(if on_time { "  fast!" } else { "  fire out" });
        self.popups.push(Popup {
            text,
            world_x: hurdle_at,
            at: now,
        });
    }

    fn skip_hurdle(&mut self) {
        if !matches!(self.phase, Phase::Waiting | Phase::Running) {
            return;
        }
        self.scroll_hold = 0.0;
        self.track
            .resolve_next(MessageStatus::Skipped, meta::demo_now(get_time()));
        self.lens.refresh(&self.track);
        self.phase = Phase::Running;
    }

    /// Cycle the faced hurdle's category (story 003): the manual choice
    /// overrides the AI, re-skins the hurdle instantly, and persists locally.
    fn retype_faced(&mut self) {
        let Some(h) = self.track.next_hurdle() else {
            return;
        };
        let id = h.message.id;
        let current = meta::enriched(&h.message, &self.overrides).category;
        self.overrides.set(id, current.next());
    }

    /// Scenario: new message ingested mid-run drops onto the track ahead.
    fn simulate_incoming(&mut self) {
        let existing: Vec<_> = self.track.hurdles.iter().map(|h| h.message.clone()).collect();
        let msg = inbox::next_incoming(&existing);
        let id = msg.id;
        self.track.add_message(msg, meta::demo_now(get_time()));
        self.lens.refresh(&self.track);
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
        let vp = self.viewport(lay);
        clear_background(view::SKY);
        self.draw_track(lay);
        self.draw_hurdles(lay);
        self.lens.draw_world(&vp);
        self.draw_runner(lay);
        self.draw_popups(lay);
        self.draw_hud(lay);
        self.profile.draw_hud(lay.w, lay.h, get_time());
        self.draw_combo(lay);
        self.draw_card(lay);
        // Search overlay covers the HUD when open; the detail card sits on
        // top of that, and the full-screen progression celebration wins over
        // everything.
        self.lens.draw_ui(&vp, &self.track, &self.course);
        self.cards.draw();
        self.profile.draw_overlays(lay.w, lay.h, get_time());
    }

    fn draw_popups(&self, lay: &Layout) {
        let now = get_time();
        for p in &self.popups {
            let t = ((now - p.at) / POPUP_LIFE) as f32;
            let fs = lay.h * 0.034;
            let x = lay.x_of(p.world_x, self.cam_x);
            let y = lay.ground_y - lay.unit * 3.6 - t * lay.unit * 1.4;
            let dims = measure_text(&p.text, None, fs as u16, 1.0);
            let mut color = view::GOLD;
            color.a = 1.0 - t * t;
            draw_text(&p.text, x - dims.width / 2.0, y, fs, color);
        }
    }

    /// Combo meter: next multiplier plus a bar draining with the window.
    fn draw_combo(&self, lay: &Layout) {
        let Some(combo) = self.score.combo(get_time()) else {
            return;
        };
        let fs = lay.h * 0.028;
        let label = format!("combo x{}", mult_label(combo.next_multiplier));
        let dims = measure_text(&label, None, fs as u16, 1.0);
        let w = (lay.w * 0.28).max(dims.width + fs * 1.4);
        // Right-aligned under the ingest button, clear of the left HUD column.
        let btn = self.ingest_button_rect(lay);
        let x = lay.w - w - lay.w * 0.04;
        let y = btn.y + btn.h + lay.h * 0.015;
        view::rounded_rect(x, y, w, fs * 1.8, fs * 0.9, view::PANEL);
        draw_text(
            &label,
            x + (w - dims.width) / 2.0,
            y + fs * 1.25,
            fs,
            view::GOLD,
        );
        let bar_h = fs * 0.32;
        let bar_y = y + fs * 2.1;
        view::rounded_rect(x, bar_y, w, bar_h, bar_h / 2.0, view::TRACK_EDGE);
        view::rounded_rect(x, bar_y, w * combo.remaining, bar_h, bar_h / 2.0, view::GOLD);
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
        let now_ts = meta::demo_now(now);
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
            let m = meta::enriched(&h.message, &self.overrides);
            let open = h.message.status == MessageStatus::Open;
            let waited = now_ts - h.message.received_at;
            let target = m.urgency.response_target();
            let style = view::HurdleStyle {
                category: m.category,
                urgency: m.urgency,
                sentiment: m.sentiment,
                // Resolved hurdles fade, as do ones outside the hub filter
                // (story 013) or filtered out by the binoculars / an active
                // hazard zone (stories 005/010/012).
                faded: !open
                    || !self.hub.filter_matches(&h.message)
                    || !self.lens.passes(&h.message),
                down: h.message.status == MessageStatus::Cleared,
                marked: h.message.status == MessageStatus::Skipped,
                burning: open && meta::is_overdue(m.urgency, waited),
                wait: open.then(|| view::WaitLabel {
                    text: if waited > target {
                        format!("late {}", meta::format_wait(waited - target))
                    } else {
                        meta::format_wait(waited)
                    },
                    frac: waited as f32 / target as f32,
                }),
            };
            view::hurdle(x, lay.ground_y + drop, lay.unit, &style, now as f32);
            // Assignee avatar on the baton owner's hurdle (story 013).
            if h.message.status != MessageStatus::Cleared {
                if let Some(runner) = self.hub.assignee(h.message.id) {
                    team::hurdle_avatar(x, lay.ground_y + drop, lay.unit, runner);
                }
            }
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
            // Peak sits just above the tallest (critical) hurdle bar.
            y -= (std::f32::consts::PI * t).sin() * lay.unit * 1.9;
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

        // Cleared counter and XP total under the pill.
        let sub = format!("cleared {}  ·  {} XP", self.cleared, self.score.xp);
        draw_text(&sub, pad + fs * 0.2, lay.h * 0.03 + fs * 2.9, fs * 0.8, view::INK_DIM);
        if self.track.free_run {
            draw_text("free run", pad + fs * 0.2, lay.h * 0.03 + fs * 4.1, fs * 0.8, view::GOLD);
        }

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

    /// Card geometry shared by draw_card and the retype hit test.
    fn card_metrics(lay: &Layout) -> (f32, f32, f32, f32) {
        let pad = lay.w * 0.04;
        let card_y = lay.h * 0.74;
        let fs = lay.h * 0.026;
        (pad, card_y, fs, pad * 2.2)
    }

    /// Where the tappable category chip sits, when a hurdle is faced.
    fn retype_chip_rect(&self, lay: &Layout) -> Option<Rect> {
        let h = self.track.next_hurdle()?;
        let m = meta::enriched(&h.message, &self.overrides);
        let (_, card_y, fs, x) = Self::card_metrics(lay);
        Some(view::chip_rect(x, card_y + fs * 6.9, fs, m.category.label()))
    }

    fn draw_card(&self, lay: &Layout) {
        let (pad, card_y, fs, x) = Self::card_metrics(lay);
        let card_h = lay.h - card_y - pad;
        view::rounded_rect(pad, card_y, lay.w - 2.0 * pad, card_h, lay.w * 0.03, view::PANEL);

        match self.track.next_hurdle() {
            Some(h) => {
                let m = meta::enriched(&h.message, &self.overrides);
                let color = view::category_color(m.category);
                draw_rectangle(pad, card_y, lay.w * 0.012, card_h, color);
                let channel = view::channel_color(h.message.channel);
                view::channel_icon(x + fs * 0.6, card_y + fs * 1.9, fs * 1.3, h.message.channel, channel);
                draw_text(
                    format!("{}  ·  {}", h.message.channel.label(), h.message.sender),
                    x + fs * 1.8,
                    card_y + fs * 2.3,
                    fs * 0.9,
                    view::INK_DIM,
                );
                draw_text(&h.message.subject, x, card_y + fs * 4.1, fs * 1.15, view::INK);
                // Waiting time vs the urgency's response target (story 014).
                let now_ts = meta::demo_now(get_time());
                let waited = now_ts - h.message.received_at;
                let target = m.urgency.response_target();
                let (status, status_color) = if waited > target {
                    (
                        format!("ON FIRE · late {}", meta::format_wait(waited - target)),
                        view::FLAME,
                    )
                } else {
                    (
                        format!(
                            "waiting {} · target {}",
                            meta::format_wait(waited),
                            meta::format_wait(target)
                        ),
                        view::INK_DIM,
                    )
                };
                draw_text(&status, x, card_y + fs * 5.4, fs * 0.85, status_color);
                // Triage row: tappable category chip plus the honest read —
                // urgency with its why-it-matters signal, and sentiment.
                let chip_label = m.category.label();
                let chip = view::chip(x, card_y + fs * 6.9, fs, chip_label, color);
                let mut triage = format!("{} urgency · {}", m.urgency.label(), m.sentiment.label());
                if let Some(signal) = m.signal {
                    triage.push_str(&format!(" · \"{}\"", signal.trim()));
                }
                if m.overridden {
                    triage.push_str(" · retyped");
                }
                draw_text(&triage, chip.x + chip.w + fs * 0.6, card_y + fs * 6.9, fs * 0.85, view::INK_DIM);
                draw_text(
                    "tap: run · up: clear · left: skip · chip: retype",
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
