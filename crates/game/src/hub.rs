//! The hub wires the team screens (stories 011/013/017/019) into the game
//! with a minimal surface in game.rs: `Game::frame` hands the frame to
//! `Hub::frame`, which runs the deterministic team simulation every frame
//! and owns update+draw whenever a non-track screen is active. On the track
//! it only contributes chrome (nav button, toasts, filter pill) and tap
//! handling for hurdle assignment.
//!
//! Dev harness: `ZD_SCREEN=race|league|boss|victory` opens a screen at
//! launch (for screenshots), `ZD_WARP=<secs>` pre-ages the simulated inbox
//! clock (burning/enrage states), `ZD_BATON=<secs>` times the scripted
//! incoming-baton demo (default 6s).

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::MessageStatus;

use crate::boss::{self, Boss};
use crate::dashboard;
use crate::input::{Gesture, GestureDetector};
use crate::leaderboard::{self, Banner, Board, Entry, Period};
use crate::score::Score;
use crate::screens::{self, FilterRequest, Nav, NavAction, NavResponse, Screen, Toasts};
use crate::team::{self, RunnerId, SimClock, Team};
use crate::track::Track;
use crate::view;

/// Mock leaderboard baselines for the local player's earlier periods; the
/// live session XP (from `Score`) is added on top and fully covers "today".
const ME_XP_WEEK0: u64 = 900;
const ME_XP_ALL0: u64 = 5200;
const ME_STREAK_DAYS: u32 = 3;
const ME_BADGES: u32 = 2;

/// Track geometry the hub needs for hit-testing hurdles; mirrors game.rs's
/// Layout without exporting it.
pub struct TrackGeom {
    pub w: f32,
    pub h: f32,
    pub unit: f32,
    pub anchor_x: f32,
    pub ground_y: f32,
    pub cam_x: f32,
}

impl TrackGeom {
    fn x_of(&self, track_x: f32) -> f32 {
        (track_x - self.cam_x) * self.unit + self.anchor_x
    }
}

pub struct Hub {
    pub screen: Screen,
    nav: Nav,
    toasts: Toasts,
    team: Team,
    boss: Boss,
    board: Board,
    filter: Option<FilterRequest>,
    my_lane: bool,
    clock: SimClock,
    sim_now: i64,
    start: f64,
    inited: bool,
    /// Last seen status per message id, to attribute newly cleared hurdles.
    known: HashMap<u64, MessageStatus>,
    last_xp: u64,
    my_clears: u32,
    /// Sum of simulated response times over my clears, for the average.
    my_resp_sum: i64,
    // Dev-harness knobs (see module docs).
    warp: f64,
    baton_at: f64,
    baton_done: bool,
    force_victory: bool,
}

impl Hub {
    pub fn new() -> Self {
        let mut force_victory = false;
        let screen = match std::env::var("ZD_SCREEN").ok().as_deref() {
            Some("race") | Some("control") | Some("dashboard") => Screen::RaceControl,
            Some("league") | Some("board") | Some("leaderboard") => Screen::Leaderboard,
            Some("boss") => Screen::Boss,
            Some("victory") => {
                force_victory = true;
                Screen::Boss
            }
            _ => Screen::Track,
        };
        let env_f64 = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        // Dev harness: ZD_FILTER presets a track filter, for screenshots.
        let filter = match std::env::var("ZD_FILTER").ok().as_deref() {
            Some("lane") => Some(FilterRequest::Assignee(RunnerId::Me)),
            Some("burning") => Some(FilterRequest::Burning),
            Some("email") => Some(FilterRequest::Channel(shared::Channel::Email)),
            _ => None,
        };
        Self {
            screen,
            nav: Nav::new(),
            toasts: Toasts::new(),
            team: Team::new(),
            boss: Boss::new(),
            board: Board::new(),
            my_lane: matches!(filter, Some(FilterRequest::Assignee(RunnerId::Me))),
            filter,
            clock: SimClock { base_ts: 0 },
            sim_now: 0,
            start: 0.0,
            inited: false,
            known: HashMap::new(),
            last_xp: 0,
            my_clears: 0,
            my_resp_sum: 0,
            warp: env_f64("ZD_WARP", 0.0),
            baton_at: env_f64("ZD_BATON", 6.0),
            baton_done: false,
            force_victory,
        }
    }

    /// Runs once per frame before the track updates. Returns true when a
    /// non-track screen owned the whole frame (update + draw).
    pub fn frame(&mut self, dt: f32, track: &mut Track, score: &Score, gestures: &mut GestureDetector) -> bool {
        let now = get_time();
        self.init(track, now);
        let elapsed = now - self.start;
        self.sim_now = self.clock.now(elapsed + self.warp);

        // Desktop dev shortcuts for screen switching.
        for (key, screen) in [
            (KeyCode::Key1, Screen::Track),
            (KeyCode::Key2, Screen::RaceControl),
            (KeyCode::Key3, Screen::Leaderboard),
            (KeyCode::Key4, Screen::Boss),
        ] {
            if is_key_pressed(key) {
                self.screen = screen;
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            self.screen = Screen::Track;
        }

        self.demo_baton(track, elapsed, now);
        self.bot_clears(track, elapsed, now);
        self.observe(track, score, now);

        let weight = boss::open_weight(track.hurdles.iter().map(|h| &h.message), self.sim_now);
        let burning = track
            .hurdles
            .iter()
            .filter(|h| h.message.status != MessageStatus::Cleared)
            .filter(|h| team::is_burning(&h.message, self.sim_now))
            .count();
        self.boss.sync(weight, burning, now);

        if self.screen == Screen::Track {
            return false;
        }
        self.screen_frame(dt, track, score, gestures, now);
        true
    }

    fn init(&mut self, track: &mut Track, now: f64) {
        if self.inited {
            return;
        }
        self.inited = true;
        self.start = now;
        self.clock = SimClock::from_messages(track.hurdles.iter().map(|h| h.message.received_at));
        // Seed a few assignments so avatars and lanes exist right away; the
        // first hurdle shows the mechanic on screen from the start.
        let ids: Vec<u64> = track.hurdles.iter().map(|h| h.message.id).collect();
        for (idx, bot) in [(0usize, 2usize), (2, 0), (5, 1)] {
            if let Some(id) = ids.get(idx) {
                self.team.assign(*id, RunnerId::Bot(bot));
            }
        }
        if self.force_victory {
            // Dev shot: fight the whole battle through the real code path.
            let weight = boss::open_weight(track.hurdles.iter().map(|h| &h.message), self.clock.base_ts);
            self.boss.sync(weight, 0, now);
            let runners = [RunnerId::Me, RunnerId::Bot(0), RunnerId::Bot(1), RunnerId::Bot(2)];
            for (i, h) in track.hurdles.iter_mut().enumerate() {
                if h.message.status == MessageStatus::Cleared {
                    continue;
                }
                let dmg = boss::msg_weight(&h.message, self.clock.base_ts);
                h.message.status = MessageStatus::Cleared;
                self.boss.on_hit(runners[i % runners.len()], dmg, now);
            }
        }
        for h in &track.hurdles {
            self.known.insert(h.message.id, h.message.status);
        }
    }

    /// Scripted relay handoff: a teammate passes the local player a baton a
    /// few seconds into the session ("Baton incoming!").
    fn demo_baton(&mut self, track: &mut Track, elapsed: f64, now: f64) {
        if self.baton_done || elapsed < self.baton_at {
            return;
        }
        self.baton_done = true;
        let next_id = track.next_hurdle().map(|h| h.message.id);
        let pick = track.hurdles.iter().find(|h| {
            h.message.status == MessageStatus::Open
                && self.team.assignee(h.message.id).is_none()
                && Some(h.message.id) != next_id
        });
        if let Some(h) = pick {
            self.team.assign(h.message.id, RunnerId::Me);
            self.team.last_clear_at.entry(RunnerId::Me).or_insert(now - 100.0);
            self.toasts.push(format!(
                "Baton incoming! {} -> you: {}",
                team::BOTS[0].name,
                screens::ellipsize(&h.message.subject, 22)
            ));
        }
    }

    /// Apply due bot clears to real hurdles so every screen (track, race
    /// control, boss) reflects teammate progress consistently.
    fn bot_clears(&mut self, track: &mut Track, elapsed: f64, now: f64) {
        for bot in 0..team::BOTS.len() {
            if !self.team.clear_due(bot, elapsed) {
                continue;
            }
            match pick_bot_hurdle(track, &self.team, bot) {
                Some(id) => {
                    let hurdle = track
                        .hurdles
                        .iter_mut()
                        .find(|h| h.message.id == id)
                        .expect("id came from pick_bot_hurdle");
                    let dmg = boss::msg_weight(&hurdle.message, self.sim_now);
                    hurdle.message.status = MessageStatus::Cleared;
                    self.known.insert(id, MessageStatus::Cleared);
                    self.team.record_landed(bot, now);
                    self.boss.on_hit(RunnerId::Bot(bot), dmg, now);
                }
                None => self.team.record_skipped(bot),
            }
        }
    }

    /// Diff the queue against the last frame: new ids are arrivals (the boss
    /// sync picks up the growth), newly cleared hurdles that the bots didn't
    /// clear are the local player's hits.
    fn observe(&mut self, track: &Track, score: &Score, now: f64) {
        for h in &track.hurdles {
            let (id, status) = (h.message.id, h.message.status);
            match self.known.get(&id) {
                None => {
                    self.known.insert(id, status);
                }
                Some(&prev) if prev != status => {
                    if status == MessageStatus::Cleared {
                        self.my_clears += 1;
                        self.my_resp_sum += (self.sim_now - h.message.received_at).max(0);
                        let dmg = boss::msg_weight(&h.message, self.sim_now);
                        self.boss.on_hit(RunnerId::Me, dmg, now);
                        self.team.last_clear_at.insert(RunnerId::Me, now);
                    }
                    self.known.insert(id, status);
                }
                _ => {}
            }
        }
        self.last_xp = self.last_xp.max(score.xp);
    }

    // ---- non-track screens ----

    fn screen_frame(&mut self, dt: f32, track: &Track, score: &Score, gestures: &mut GestureDetector, now: f64) {
        let (w, h) = (screen_width(), screen_height());
        let mut tap = match gestures.poll() {
            Gesture::Tap(pos) => Some(pos),
            _ => None,
        };

        // The nav draws on top, so it gets the tap first.
        if let Some(pos) = tap {
            match self.nav.handle_tap(pos, w, h) {
                NavResponse::Action(action) => {
                    self.apply_nav(action);
                    tap = None;
                }
                NavResponse::Consumed => tap = None,
                NavResponse::Pass => {}
            }
        }

        clear_background(view::SKY);
        let (title, subtitle) = match self.screen {
            Screen::RaceControl => ("Race control", "the whole course at a glance"),
            Screen::Leaderboard => ("League", "clearing the backlog together"),
            Screen::Boss => ("Backlog boss", "every clear is a hit"),
            Screen::Track => unreachable!("track frames are owned by Game"),
        };
        if screens::header(tap, w, h, title, subtitle) {
            self.screen = Screen::Track;
            tap = None;
        }

        match self.screen {
            Screen::RaceControl => {
                let stats = dashboard::compute(track.hurdles.iter().map(|h| &h.message), self.sim_now);
                let runners = self.runner_progress();
                if let Some(filter) = dashboard::draw(tap, w, h, &stats, &runners, now) {
                    // Drill-down: open the track filtered to the segment.
                    self.set_filter(Some(filter));
                    self.screen = Screen::Track;
                    self.toasts.push(format!("Track filtered: {}", filter.label()));
                }
            }
            Screen::Leaderboard => {
                let rows = leaderboard::standings(self.entries(self.board.period, score));
                let banner = self.banner(score);
                self.board.frame(tap, dt, now, w, h, &banner, rows);
            }
            Screen::Boss => boss::draw(&self.boss, w, h, now),
            Screen::Track => {}
        }

        self.nav.draw(w, h, self.screen, self.my_lane);
        self.toasts.draw(w, h);
    }

    fn apply_nav(&mut self, action: NavAction) {
        match action {
            NavAction::Goto(screen) => self.screen = screen,
            NavAction::ToggleMyLane => {
                let on = !self.my_lane;
                self.set_filter(on.then_some(FilterRequest::Assignee(RunnerId::Me)));
                self.screen = Screen::Track;
            }
        }
    }

    fn set_filter(&mut self, filter: Option<FilterRequest>) {
        self.filter = filter;
        self.my_lane = matches!(filter, Some(FilterRequest::Assignee(RunnerId::Me)));
    }

    fn runner_progress(&self) -> Vec<dashboard::RunnerProgress> {
        let mut rows = vec![dashboard::RunnerProgress {
            runner: RunnerId::Me,
            clears: self.my_clears,
            last_clear: self.team.last_clear_at.get(&RunnerId::Me).copied(),
        }];
        for i in 0..team::BOTS.len() {
            rows.push(dashboard::RunnerProgress {
                runner: RunnerId::Bot(i),
                clears: self.team.bot_clears[i],
                last_clear: self.team.last_clear_at.get(&RunnerId::Bot(i)).copied(),
            });
        }
        rows
    }

    fn entries(&self, period: Period, score: &Score) -> Vec<Entry> {
        let me_xp = score.xp
            + match period {
                Period::Today => 0,
                Period::Week => ME_XP_WEEK0,
                Period::AllTime => ME_XP_ALL0,
            };
        let me_avg = if self.my_clears > 0 {
            self.my_resp_sum / i64::from(self.my_clears)
        } else {
            -1
        };
        let mut entries = vec![Entry {
            runner: RunnerId::Me,
            xp: me_xp,
            streak_days: ME_STREAK_DAYS,
            badges: ME_BADGES,
            avg_resp_secs: me_avg,
        }];
        for (i, spec) in team::BOTS.iter().enumerate() {
            entries.push(Entry {
                runner: RunnerId::Bot(i),
                xp: leaderboard::bot_xp(spec, self.team.bot_clears[i], period),
                streak_days: spec.streak_days,
                badges: spec.badges,
                avg_resp_secs: spec.avg_response_secs,
            });
        }
        entries
    }

    fn banner(&self, score: &Score) -> Banner {
        let team_xp_today = self.entries(Period::Today, score).iter().map(|e| e.xp).sum();
        let team_cleared = self.my_clears + self.team.bot_clears.iter().sum::<u32>();
        Banner {
            team_xp_today,
            team_cleared,
            incoming: self.known.len() as u32,
        }
    }

    // ---- track-screen chrome & taps ----

    /// Handle a tap while the track screen is active. Returns true when the
    /// hub consumed it (nav, filter pill, or hurdle assignment).
    pub fn track_tap(&mut self, pos: Vec2, geom: TrackGeom, track: &mut Track) -> bool {
        match self.nav.handle_tap(pos, geom.w, geom.h) {
            NavResponse::Action(action) => {
                self.apply_nav(action);
                return true;
            }
            NavResponse::Consumed => return true,
            NavResponse::Pass => {}
        }

        if self.filter.is_some() && filter_pill_rect(geom.w, geom.h).contains(pos) {
            self.set_filter(None);
            return true;
        }

        // A tap on a hurdle's sign cycles its baton: claim it, pass it to
        // each teammate, release it (story 013).
        for h in &track.hurdles {
            if h.message.status == MessageStatus::Cleared {
                continue;
            }
            let x = geom.x_of(h.at);
            let sign = geom.unit * 0.9;
            let sign_cy = geom.ground_y - geom.unit * 1.3 - sign * 0.75;
            let hit = Rect::new(x - sign * 0.72, sign_cy - sign * 0.72, sign * 1.44, sign * 1.44);
            if hit.contains(pos) {
                let subject = screens::ellipsize(&h.message.subject, 20);
                let toast = match self.team.cycle(h.message.id) {
                    Some(RunnerId::Me) => format!("You claimed: {subject}"),
                    Some(bot) => format!("Baton passed to {}", team::runner_name(bot)),
                    None => format!("Unassigned: {subject}"),
                };
                self.toasts.push(toast);
                return true;
            }
        }
        false
    }

    /// Chrome drawn over the track: filter pill, nav button/menu, toasts.
    pub fn track_overlay(&mut self, w: f32, h: f32) {
        if let Some(filter) = self.filter {
            let pill = filter_pill_rect(w, h);
            view::rounded_rect(pill.x, pill.y, pill.w, pill.h, pill.h * 0.5, view::PANEL);
            let fs = pill.h * 0.52;
            let label = format!("filter: {}   x", filter.label());
            let dims = measure_text(&label, None, fs as u16, 1.0);
            draw_text(
                &label,
                pill.x + (pill.w - dims.width) / 2.0,
                pill.y + pill.h * 0.66,
                fs,
                view::GOLD,
            );
        }
        self.nav.draw(w, h, self.screen, self.my_lane);
        self.toasts.draw(w, h);
    }

    /// Assignee of a hurdle, for the avatar on the track (story 013).
    pub fn assignee(&self, id: u64) -> Option<RunnerId> {
        self.team.assignee(id)
    }

    /// Whether a hurdle passes the active filter (no filter = everything).
    /// The track dims non-matching hurdles; story 006's overlay can take
    /// this over.
    pub fn filter_matches(&self, msg: &shared::Message) -> bool {
        self.filter
            .map_or(true, |f| f.matches(msg, self.team.assignee(msg.id), self.sim_now))
    }
}

fn filter_pill_rect(w: f32, h: f32) -> Rect {
    // Centered in the empty sky band, clear of the HUD, combo, and toasts.
    let pw = w * 0.5;
    let ph = h * 0.038;
    Rect::new((w - pw) / 2.0, h * 0.215, pw, ph)
}

/// The hurdle a bot's scheduled clear lands on: prefer its own lane, then
/// any unassigned open hurdle, never the one the player is facing and never
/// another runner's baton.
fn pick_bot_hurdle(track: &Track, team: &Team, bot: usize) -> Option<u64> {
    let next_id = track.next_hurdle().map(|h| h.message.id);
    let eligible = |id: u64, status: MessageStatus| status != MessageStatus::Cleared && Some(id) != next_id;
    track
        .hurdles
        .iter()
        .find(|h| {
            eligible(h.message.id, h.message.status)
                && team.assignee(h.message.id) == Some(RunnerId::Bot(bot))
        })
        .or_else(|| {
            track.hurdles.iter().find(|h| {
                eligible(h.message.id, h.message.status) && team.assignee(h.message.id).is_none()
            })
        })
        .map(|h| h.message.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::sample_messages;

    #[test]
    fn bots_prefer_their_own_lane_and_never_take_the_faced_hurdle() {
        let mut track = Track::new(sample_messages(4), crate::meta::demo_now(0.0));
        let mut team = Team::new();
        let ids: Vec<u64> = track.hurdles.iter().map(|h| h.message.id).collect();

        // The player faces the first hurdle; a bot must not steal it even
        // when it is the only unassigned one left.
        team.assign(ids[1], RunnerId::Me);
        team.assign(ids[2], RunnerId::Bot(1));
        team.assign(ids[3], RunnerId::Bot(0));
        assert_eq!(pick_bot_hurdle(&track, &team, 0), Some(ids[3]), "own lane first");

        team.unassign(ids[3]);
        assert_eq!(
            pick_bot_hurdle(&track, &team, 0),
            Some(ids[3]),
            "falls back to unassigned hurdles"
        );

        team.assign(ids[3], RunnerId::Bot(1));
        assert_eq!(
            pick_bot_hurdle(&track, &team, 0),
            None,
            "never the faced hurdle, never another runner's baton"
        );

        // Once the faced hurdle is resolved the next one frees up.
        track.resolve_next(MessageStatus::Cleared, crate::meta::demo_now(0.0));
        team.unassign(ids[1]);
        assert_eq!(pick_bot_hurdle(&track, &team, 0), None, "ids[1] is now faced");
    }
}
