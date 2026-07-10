//! Hurdle detail card (stories 006, 007, 008 + thumbs from 020): the overlay
//! that opens when the runner stands at a hurdle. Leads with the AI scout
//! report and honest urgency, one tap deeper shows the full message, the
//! reply power-up charges an editable AI draft, and a swipe up — and nothing
//! else — sends it and clears the hurdle. Backing out keeps the draft.
//!
//! Pure state (stage machine, draft editing, charge timing) lives in `Card`
//! and `ReplyState` and is unit-tested headless; `CardHost` adds screen
//! layout, tap hit-testing, and primitives-only drawing in view.rs style.

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::{AiFeature, FeedbackRating, Message};

use crate::feedback::FeedbackStore;
use crate::input::Gesture;
use crate::meta::{self, MessageMeta, Urgency};
use crate::reply;
use crate::view;

/// Seconds the power-up "charges" before the draft starts to appear.
pub const CHARGE_TIME: f64 = 0.5;
/// Seconds the typewriter reveal takes after charging.
pub const REVEAL_TIME: f64 = 0.7;
/// Longest steering note the field accepts.
const NOTE_MAX: usize = 60;

const CHIP_LABELS: [&str; 3] = ["shorter", "more formal", "offer refund"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Scout report + urgency; the card's landing view (story 006).
    Scout,
    /// The complete original message, one tap deeper (story 006).
    FullMessage,
    /// Editable AI draft + steering note + recharge (stories 007, 008).
    Reply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Draft,
    Note,
}

/// Draft-in-progress state; survives backing out of the card (story 008).
#[derive(Debug, Clone)]
pub struct ReplyState {
    /// The pristine AI output, kept for feedback (story 020).
    generated: Option<String>,
    /// The player's editable copy; what a send actually sends.
    draft: Option<String>,
    note: String,
    /// Recharge count; feeds the mock generator's variation.
    variant: u32,
    /// When the last (re)charge started, for the charge/reveal animation.
    charged_at: f64,
    focus: Focus,
}

impl ReplyState {
    fn new() -> Self {
        Self {
            generated: None,
            draft: None,
            note: String::new(),
            variant: 0,
            charged_at: f64::NEG_INFINITY,
            focus: Focus::Draft,
        }
    }
}

/// One open hurdle card: pure state, no rendering.
pub struct Card {
    pub message: Message,
    pub meta: MessageMeta,
    /// AI scout report shown at the top of the card (story 006).
    pub summary: String,
    /// Why the urgency is what it is — "height is honest" (story 004).
    pub why: String,
    pub stage: Stage,
    reply: ReplyState,
}

impl Card {
    pub fn new(message: &Message) -> Self {
        let meta = meta::enrich(message);
        Self {
            message: message.clone(),
            summary: meta::summarize(message),
            why: meta::why_it_matters(&meta),
            meta,
            stage: Stage::Scout,
            reply: ReplyState::new(),
        }
    }

    /// The reply power-up (story 007): jump to the reply stage and charge a
    /// draft if none exists yet. A kept draft from an earlier visit is shown
    /// as-is instead of being regenerated.
    pub fn power_up(&mut self, now: f64) {
        self.stage = Stage::Reply;
        if self.reply.generated.is_none() {
            self.generate(now);
        }
    }

    /// Regenerate the draft taking the steering note into account (story 007).
    /// Discards the player's edits — that's the point of a recharge.
    pub fn recharge(&mut self, now: f64) {
        if self.stage != Stage::Reply || self.charging(now) {
            return;
        }
        self.reply.variant += 1;
        self.generate(now);
    }

    fn generate(&mut self, now: f64) {
        let text = reply::generate_draft(&self.message, &self.meta, &self.reply.note, self.reply.variant);
        self.reply.generated = Some(text.clone());
        self.reply.draft = Some(text);
        self.reply.charged_at = now;
    }

    pub fn charging(&self, now: f64) -> bool {
        self.reply.draft.is_some() && now < self.reply.charged_at + CHARGE_TIME
    }

    /// 0..=1 typewriter progress of the draft text after charging.
    pub fn reveal_progress(&self, now: f64) -> f32 {
        if self.reply.draft.is_none() {
            return 0.0;
        }
        (((now - self.reply.charged_at - CHARGE_TIME) / REVEAL_TIME) as f32).clamp(0.0, 1.0)
    }

    /// A fully revealed draft is editable and sendable (story 008: the player
    /// must be able to review everything before approving).
    pub fn ready(&self, now: f64) -> bool {
        self.reply.draft.is_some() && self.reveal_progress(now) >= 1.0
    }

    pub fn draft(&self) -> Option<&str> {
        self.reply.draft.as_deref()
    }

    pub fn note(&self) -> &str {
        &self.reply.note
    }

    pub fn set_note(&mut self, note: &str) {
        self.reply.note = note.chars().take(NOTE_MAX).collect();
        self.reply.focus = Focus::Note;
    }

    /// Keyboard editing of the focused field. Minimal by design: characters
    /// append, backspace deletes — a pragmatic mobile-feel affordance until a
    /// platform keyboard/IME story exists.
    pub fn type_char(&mut self, c: char, now: f64) {
        match self.reply.focus {
            Focus::Note => {
                if c != '\n' && self.reply.note.chars().count() < NOTE_MAX {
                    self.reply.note.push(c);
                }
            }
            Focus::Draft => {
                if self.ready(now) {
                    if let Some(draft) = &mut self.reply.draft {
                        draft.push(c);
                    }
                }
            }
        }
    }

    pub fn backspace(&mut self, now: f64) {
        match self.reply.focus {
            Focus::Note => {
                self.reply.note.pop();
            }
            Focus::Draft => {
                if self.ready(now) {
                    if let Some(draft) = &mut self.reply.draft {
                        draft.pop();
                    }
                }
            }
        }
    }
}

/// What the card asks the game to do after handling input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    None,
    /// Backed out; message stays open, draft kept, no penalty (story 008).
    Closed,
    /// The player approved the reply: it is "sent" (simulated for now — see
    /// api-changerequests/card-flow.md) and the hurdle may be jumped.
    Send,
}

/// Owns the open card, kept drafts per message, and the feedback store.
/// The game holds exactly one of these.
pub struct CardHost {
    open: Option<Card>,
    /// Reply state stashed when a card is closed without sending.
    saved: HashMap<u64, ReplyState>,
    feedback: FeedbackStore,
    opened_at: f64,
}

impl CardHost {
    pub fn new() -> Self {
        Self {
            open: None,
            saved: HashMap::new(),
            feedback: FeedbackStore::new(),
            opened_at: 0.0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// AI-coach feedback, for race control's aggregate ratios (story 020).
    #[allow(dead_code)] // read by the race-control screen (story 011)
    pub fn feedback(&self) -> &FeedbackStore {
        &self.feedback
    }

    /// Open the card for a message, restoring any kept draft (story 008).
    pub fn open_for(&mut self, message: &Message, now: f64) {
        let mut card = Card::new(message);
        if let Some(kept) = self.saved.get(&message.id) {
            card.reply = kept.clone();
        }
        self.open = Some(card);
        self.opened_at = now;
    }

    fn close(&mut self) -> CardAction {
        if let Some(card) = self.open.take() {
            self.saved.insert(card.message.id, card.reply);
        }
        CardAction::Closed
    }

    fn try_send(&mut self, now: f64) -> CardAction {
        let ready = self
            .open
            .as_ref()
            .is_some_and(|c| c.stage == Stage::Reply && c.ready(now));
        if !ready {
            return CardAction::None;
        }
        let card = self.open.take().expect("checked above");
        let sent = card.reply.draft.clone().unwrap_or_default();
        // Story 020: the sent version is stored next to the draft rating.
        self.feedback.set_final_value(card.message.id, &sent);
        self.saved.remove(&card.message.id);
        CardAction::Send
    }

    fn rate(&mut self, feature: AiFeature, rating: FeedbackRating) {
        let Some(card) = &self.open else { return };
        let ai_output = match feature {
            AiFeature::Category => card.meta.category.label().to_string(),
            AiFeature::Urgency => card.meta.urgency.label().to_string(),
            AiFeature::Summary => card.summary.clone(),
            AiFeature::DraftReply => match &card.reply.generated {
                Some(text) => text.clone(),
                None => return,
            },
        };
        self.feedback.rate(card.message.id, feature, &ai_output, rating);
    }

    /// Route one gesture into the card. `w`/`h` are the screen size the card
    /// was drawn at (hit-testing must match drawing); `now` in seconds.
    pub fn handle(&mut self, gesture: Gesture, w: f32, h: f32, now: f64) -> CardAction {
        if self.open.is_none() {
            return CardAction::None;
        }
        match gesture {
            // Story 008: swipe up is the explicit approval that sends.
            Gesture::SwipeUp => self.try_send(now),
            Gesture::Tap(pos) => self.tap(pos, w, h, now),
            _ => CardAction::None,
        }
    }

    fn tap(&mut self, pos: Vec2, w: f32, h: f32, now: f64) -> CardAction {
        let ly = lay(w, h);
        if ly.close.contains(pos) || !ly.panel.contains(pos) {
            return self.close();
        }
        let stage = self.open.as_ref().expect("checked in handle").stage;
        match stage {
            Stage::Scout => {
                for (up, down, feature) in [
                    (ly.cat_up, ly.cat_down, AiFeature::Category),
                    (ly.urg_up, ly.urg_down, AiFeature::Urgency),
                    (ly.sum_up, ly.sum_down, AiFeature::Summary),
                ] {
                    if up.contains(pos) {
                        self.rate(feature, FeedbackRating::Helpful);
                        return CardAction::None;
                    }
                    if down.contains(pos) {
                        self.rate(feature, FeedbackRating::Unhelpful);
                        return CardAction::None;
                    }
                }
                let card = self.open.as_mut().expect("checked in handle");
                if ly.read_full.contains(pos) {
                    card.stage = Stage::FullMessage;
                } else if ly.power_up.contains(pos) {
                    card.power_up(now);
                }
            }
            Stage::FullMessage => {
                let card = self.open.as_mut().expect("checked in handle");
                if ly.back.contains(pos) {
                    card.stage = Stage::Scout;
                }
            }
            Stage::Reply => {
                if ly.draft_up.contains(pos) {
                    self.rate(AiFeature::DraftReply, FeedbackRating::Helpful);
                    return CardAction::None;
                }
                if ly.draft_down.contains(pos) {
                    self.rate(AiFeature::DraftReply, FeedbackRating::Unhelpful);
                    return CardAction::None;
                }
                let card = self.open.as_mut().expect("checked in handle");
                if ly.draft_panel.contains(pos) {
                    card.reply.focus = Focus::Draft;
                } else if ly.note_field.contains(pos) {
                    card.reply.focus = Focus::Note;
                } else if ly.recharge.contains(pos) {
                    card.recharge(now);
                } else if ly.back.contains(pos) {
                    card.stage = Stage::Scout;
                } else {
                    for (chip, label) in ly.chips.iter().zip(CHIP_LABELS) {
                        if chip.contains(pos) {
                            card.set_note(label);
                        }
                    }
                }
            }
        }
        CardAction::None
    }

    /// Feed keyboard input to the focused text field (desktop dev; on device
    /// the steering chips carry the flow). Runtime-only: touches macroquad.
    pub fn poll_keys(&mut self, now: f64) {
        let Some(card) = self.open.as_mut() else { return };
        while let Some(c) = get_char_pressed() {
            if c == '\r' || c == '\n' {
                card.type_char('\n', now);
            } else if !c.is_control() {
                card.type_char(c, now);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            card.backspace(now);
        }
    }

    // ---- dev-harness hooks (ZD_DEMO, see main.rs) ----

    pub fn demo_power_up(&mut self, now: f64) {
        if let Some(card) = self.open.as_mut() {
            card.power_up(now);
        }
    }

    pub fn demo_recharge(&mut self, note: &str, now: f64) {
        if let Some(card) = self.open.as_mut() {
            card.set_note(note);
            card.recharge(now);
        }
    }

    pub fn demo_send(&mut self, now: f64) -> CardAction {
        self.try_send(now)
    }

    // ---- drawing ----

    pub fn draw(&self) {
        let Some(card) = &self.open else { return };
        let (w, h) = (screen_width(), screen_height());
        let now = get_time();

        // Short slide-and-fade entrance.
        let t = (((now - self.opened_at) / 0.22) as f32).clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.02, 0.02, 0.08, 0.62 * ease));
        let ly = lay(w, h).shifted((1.0 - ease) * h * 0.05);

        view::rounded_rect(ly.panel.x, ly.panel.y, ly.panel.w, ly.panel.h, w * 0.03, view::PANEL);
        let channel = card.message.channel;
        draw_rectangle(ly.panel.x, ly.panel.y + w * 0.03, w * 0.012, ly.panel.h - w * 0.06, view::channel_color(channel));

        // Close button, all stages.
        view::rounded_rect(ly.close.x, ly.close.y, ly.close.w, ly.close.h, ly.close.w * 0.3, view::TRACK);
        let xfs = ly.close.h * 0.75;
        let xd = measure_text("x", None, xfs as u16, 1.0);
        draw_text("x", ly.close.x + (ly.close.w - xd.width) / 2.0, ly.close.y + ly.close.h * 0.68, xfs, view::INK_DIM);

        match card.stage {
            Stage::Scout => self.draw_scout(card, &ly, now),
            Stage::FullMessage => self.draw_full_message(card, &ly),
            Stage::Reply => self.draw_reply(card, &ly, now),
        }
    }

    fn draw_header(&self, card: &Card, ly: &Lay, subject_lines: usize) {
        let fs = ly.panel.h * 0.026;
        let x = ly.panel.x + ly.pad;
        let color = view::channel_color(card.message.channel);
        view::channel_icon(x + fs * 0.7, ly.py(0.055), fs * 1.4, card.message.channel, color);
        draw_text(
            &format!("{}  ·  {}", card.message.channel.label(), card.message.sender),
            x + fs * 1.9,
            ly.py(0.055) + fs * 0.45,
            fs,
            view::INK_DIM,
        );
        let sfs = ly.panel.h * 0.032;
        draw_wrapped(&card.message.subject, x, ly.py(0.095), ly.text_w(), sfs, view::INK, subject_lines);
    }

    fn draw_scout(&self, card: &Card, ly: &Lay, now: f64) {
        self.draw_header(card, ly, 2);
        let x = ly.panel.x + ly.pad;
        let fs = ly.panel.h * 0.024;
        let id = card.message.id;

        // Hurdle typing + height, with the honest why (stories 003/004).
        draw_text("TYPE", x, ly.py(0.225) + fs, fs * 0.8, view::INK_DIM);
        draw_text(card.meta.category.label(), x + ly.panel.w * 0.24, ly.py(0.225) + fs, fs * 1.05, view::INK);
        self.thumbs(ly.cat_up, ly.cat_down, id, AiFeature::Category);

        draw_text("URGENCY", x, ly.py(0.29) + fs, fs * 0.8, view::INK_DIM);
        draw_text(
            card.meta.urgency.label(),
            x + ly.panel.w * 0.24,
            ly.py(0.29) + fs,
            fs * 1.05,
            urgency_color(card.meta.urgency),
        );
        self.thumbs(ly.urg_up, ly.urg_down, id, AiFeature::Urgency);
        draw_wrapped(&card.why, x, ly.py(0.335), ly.text_w(), fs * 0.9, view::INK_DIM, 2);

        draw_line(x, ly.py(0.425), ly.panel.x + ly.panel.w - ly.pad, ly.py(0.425), 1.5, view::TRACK_EDGE);

        // Scout report (story 006); short messages are the original verbatim
        // and get no thumbs — there was no AI output to rate.
        let verbatim = card.message.body.len() <= meta::SUMMARY_THRESHOLD;
        let caption = if verbatim { "MESSAGE (short - shown in full)" } else { "SCOUT REPORT" };
        draw_text(caption, x, ly.py(0.465) + fs, fs * 0.8, view::GOLD);
        if !verbatim {
            self.thumbs(ly.sum_up, ly.sum_down, id, AiFeature::Summary);
        }
        draw_wrapped(&card.summary, x, ly.py(0.51), ly.text_w(), fs * 1.05, view::INK, 7);

        button(ly.read_full, "read full message", view::TRACK, view::INK, fs);
        button(ly.power_up, "draft reply", view::ACCENT, view::INK, fs);
        bolt(ly.power_up.x + ly.power_up.w * 0.12, ly.power_up.y + ly.power_up.h * 0.5, ly.power_up.h * 0.55, view::GOLD);

        hint(ly, "tap x or outside to back out - nothing is sent", view::INK_DIM);
        let _ = now;
    }

    fn draw_full_message(&self, card: &Card, ly: &Lay) {
        self.draw_header(card, ly, 2);
        let x = ly.panel.x + ly.pad;
        let fs = ly.panel.h * 0.024;
        draw_text("FULL MESSAGE", x, ly.py(0.225) + fs, fs * 0.8, view::GOLD);
        draw_wrapped(&card.message.body, x, ly.py(0.27), ly.text_w(), fs * 1.05, view::INK, 15);
        button(ly.back, "back", view::TRACK, view::INK, fs);
        hint(ly, "the scout report is the AI's read - this is the source", view::INK_DIM);
    }

    fn draw_reply(&self, card: &Card, ly: &Lay, now: f64) {
        let fs = ly.panel.h * 0.024;
        let x = ly.panel.x + ly.pad;
        let color = view::channel_color(card.message.channel);
        view::channel_icon(x + fs * 0.7, ly.py(0.055), fs * 1.4, card.message.channel, color);
        draw_wrapped(&card.message.subject, x + fs * 1.9, ly.py(0.03), ly.text_w() - fs * 1.9, fs, view::INK_DIM, 1);

        draw_text("AI DRAFT - YOURS TO EDIT", x, ly.py(0.115) + fs, fs * 0.8, view::GOLD);
        if card.reply.generated.is_some() {
            self.thumbs(ly.draft_up, ly.draft_down, card.message.id, AiFeature::DraftReply);
        }

        // Draft panel: charge animation, then typewriter reveal (story 007).
        let dp = ly.draft_panel;
        view::rounded_rect(dp.x, dp.y, dp.w, dp.h, fs, view::SKY);
        if card.reply.focus == Focus::Draft {
            draw_rectangle(dp.x, dp.y + dp.h - 3.0, dp.w, 3.0, view::ACCENT);
        }
        let inset = fs;
        if card.charging(now) {
            let cx = dp.x + dp.w / 2.0;
            let cy = dp.y + dp.h * 0.42;
            bolt(cx, cy, fs * 3.2, view::GOLD);
            let label = "charging power-up...";
            let d = measure_text(label, None, fs as u16, 1.0);
            draw_text(label, cx - d.width / 2.0, cy + fs * 3.0, fs, view::INK_DIM);
            let frac = (((now - card.reply.charged_at) / CHARGE_TIME) as f32).clamp(0.0, 1.0);
            let bar = Rect::new(dp.x + dp.w * 0.2, cy + fs * 4.0, dp.w * 0.6, fs * 0.5);
            view::rounded_rect(bar.x, bar.y, bar.w, bar.h, bar.h / 2.0, view::TRACK_EDGE);
            view::rounded_rect(bar.x, bar.y, bar.w * frac, bar.h, bar.h / 2.0, view::GOLD);
        } else if let Some(draft) = card.draft() {
            let mut shown = reveal_prefix(draft, card.reveal_progress(now)).to_string();
            if card.ready(now) && card.reply.focus == Focus::Draft && blink(now) {
                shown.push('_');
            }
            draw_wrapped(&shown, dp.x + inset, dp.y + inset * 0.6, dp.w - 2.0 * inset, fs * 0.88, view::INK, 14);
        }

        draw_text("STEERING NOTE FOR RECHARGE", x, ly.py(0.62) + fs * 0.8, fs * 0.8, view::INK_DIM);
        let nf = ly.note_field;
        view::rounded_rect(nf.x, nf.y, nf.w, nf.h, fs * 0.6, view::SKY);
        if card.reply.focus == Focus::Note {
            draw_rectangle(nf.x, nf.y + nf.h - 3.0, nf.w, 3.0, view::ACCENT);
        }
        let mut note = card.note().to_string();
        if card.reply.focus == Focus::Note && blink(now) {
            note.push('_');
        }
        if note.is_empty() {
            draw_text("tap a chip or type a note...", nf.x + fs, nf.y + nf.h * 0.68, fs * 0.9, view::TRACK_EDGE);
        } else {
            draw_text(&note, nf.x + fs, nf.y + nf.h * 0.68, fs * 0.9, view::INK);
        }
        for (chip, label) in ly.chips.iter().zip(CHIP_LABELS) {
            let selected = card.note() == label;
            let bg = if selected { view::GOLD } else { view::TRACK };
            let fg = if selected { view::SKY } else { view::INK_DIM };
            button(*chip, label, bg, fg, fs * 0.85);
        }

        button(ly.back, "back", view::TRACK, view::INK, fs);
        let ready = !card.charging(now);
        let bg = if ready { view::ACCENT } else { view::TRACK };
        button(ly.recharge, "recharge", bg, view::INK, fs);
        bolt(ly.recharge.x + ly.recharge.w * 0.12, ly.recharge.y + ly.recharge.h * 0.5, ly.recharge.h * 0.55, view::GOLD);

        if card.ready(now) {
            hint(ly, "swipe up to send  ^  clears the hurdle", view::GOLD);
        } else {
            hint(ly, "review before sending - nothing goes out on its own", view::INK_DIM);
        }
    }

    /// A thumbs up/down pair, lit by the stored rating (story 020).
    fn thumbs(&self, up: Rect, down: Rect, message_id: u64, feature: AiFeature) {
        let rating = self.feedback.rating_of(message_id, feature);
        thumb(up, true, rating == Some(FeedbackRating::Helpful));
        thumb(down, false, rating == Some(FeedbackRating::Unhelpful));
    }
}

fn urgency_color(u: Urgency) -> Color {
    match u {
        Urgency::Critical => view::ACCENT,
        Urgency::High => view::GOLD,
        Urgency::Normal => view::INK,
        Urgency::Low => view::INK_DIM,
    }
}

fn blink(now: f64) -> bool {
    (now * 2.4) as i64 % 2 == 0
}

/// The revealed prefix of the draft during the typewriter animation, cut on
/// a char boundary.
fn reveal_prefix(draft: &str, progress: f32) -> &str {
    if progress >= 1.0 {
        return draft;
    }
    let count = (draft.chars().count() as f32 * progress) as usize;
    match draft.char_indices().nth(count) {
        Some((i, _)) => &draft[..i],
        None => draft,
    }
}

// ---- layout ----

/// Every rect the card draws and hit-tests, computed from screen size only,
/// so input and rendering can never disagree.
struct Lay {
    panel: Rect,
    pad: f32,
    close: Rect,
    // scout
    cat_up: Rect,
    cat_down: Rect,
    urg_up: Rect,
    urg_down: Rect,
    sum_up: Rect,
    sum_down: Rect,
    read_full: Rect,
    power_up: Rect,
    // full message + reply share the left button slot
    back: Rect,
    // reply
    draft_up: Rect,
    draft_down: Rect,
    draft_panel: Rect,
    note_field: Rect,
    chips: [Rect; 3],
    recharge: Rect,
}

impl Lay {
    /// Panel-relative y: 0.0 = top edge, 1.0 = bottom edge.
    fn py(&self, f: f32) -> f32 {
        self.panel.y + self.panel.h * f
    }

    fn text_w(&self) -> f32 {
        self.panel.w - 2.0 * self.pad
    }

    /// The same layout lowered by `dy`, for the entrance animation. Input
    /// keeps using the settled layout; the slide lasts a fifth of a second.
    fn shifted(mut self, dy: f32) -> Self {
        for r in [
            &mut self.panel,
            &mut self.close,
            &mut self.cat_up,
            &mut self.cat_down,
            &mut self.urg_up,
            &mut self.urg_down,
            &mut self.sum_up,
            &mut self.sum_down,
            &mut self.read_full,
            &mut self.power_up,
            &mut self.back,
            &mut self.draft_up,
            &mut self.draft_down,
            &mut self.draft_panel,
            &mut self.note_field,
            &mut self.recharge,
        ] {
            r.y += dy;
        }
        for r in &mut self.chips {
            r.y += dy;
        }
        self
    }
}

fn lay(w: f32, h: f32) -> Lay {
    let panel = Rect::new(w * 0.05, h * 0.085, w * 0.90, h * 0.815);
    let pad = w * 0.05;
    let py = |f: f32| panel.y + panel.h * f;

    let cs = h * 0.040;
    let close = Rect::new(panel.x + panel.w - cs - pad * 0.5, panel.y + pad * 0.5, cs, cs);

    let ts = h * 0.030;
    let tgap = w * 0.02;
    let thumbs_at = |y: f32| {
        (
            Rect::new(panel.x + panel.w - pad - 2.0 * ts - tgap, y, ts, ts),
            Rect::new(panel.x + panel.w - pad - ts, y, ts, ts),
        )
    };
    let (cat_up, cat_down) = thumbs_at(py(0.215));
    let (urg_up, urg_down) = thumbs_at(py(0.28));
    let (sum_up, sum_down) = thumbs_at(py(0.455));
    let (draft_up, draft_down) = thumbs_at(py(0.105));

    let btn_h = panel.h * 0.072;
    let btn_w = (panel.w - 3.0 * pad) / 2.0;
    let read_full = Rect::new(panel.x + pad, py(0.845), btn_w, btn_h);
    let power_up = Rect::new(panel.x + pad * 2.0 + btn_w, py(0.845), btn_w, btn_h);
    let back = read_full;
    let recharge = power_up;

    let draft_panel = Rect::new(panel.x + pad, py(0.15), panel.w - 2.0 * pad, panel.h * 0.445);
    let note_field = Rect::new(panel.x + pad, py(0.645), panel.w - 2.0 * pad, panel.h * 0.05);
    let cgap = w * 0.02;
    let chip_w = (panel.w - 2.0 * pad - 2.0 * cgap) / 3.0;
    let chip_h = panel.h * 0.048;
    let chips = [0, 1, 2].map(|i| Rect::new(panel.x + pad + i as f32 * (chip_w + cgap), py(0.715), chip_w, chip_h));

    Lay {
        panel,
        pad,
        close,
        cat_up,
        cat_down,
        urg_up,
        urg_down,
        sum_up,
        sum_down,
        read_full,
        power_up,
        back,
        draft_up,
        draft_down,
        draft_panel,
        note_field,
        chips,
        recharge,
    }
}

// ---- primitives-only widgets, in view.rs style ----

fn button(r: Rect, label: &str, bg: Color, fg: Color, fs: f32) {
    view::rounded_rect(r.x, r.y, r.w, r.h, r.h * 0.3, bg);
    let d = measure_text(label, None, fs as u16, 1.0);
    draw_text(label, r.x + (r.w - d.width) / 2.0, r.y + r.h * 0.5 + fs * 0.35, fs, fg);
}

/// Thumbs up/down as a triangle in a rounded chip; gold/pink when selected.
fn thumb(r: Rect, up: bool, selected: bool) {
    let bg = if selected {
        if up { view::GOLD } else { view::ACCENT }
    } else {
        view::TRACK
    };
    view::rounded_rect(r.x, r.y, r.w, r.h, r.w * 0.25, bg);
    let fg = if selected { view::SKY } else { view::INK_DIM };
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let s = r.w * 0.28;
    if up {
        draw_triangle(vec2(cx, cy - s), vec2(cx - s, cy + s * 0.7), vec2(cx + s, cy + s * 0.7), fg);
    } else {
        draw_triangle(vec2(cx, cy + s), vec2(cx - s, cy - s * 0.7), vec2(cx + s, cy - s * 0.7), fg);
    }
}

/// Lightning bolt from two triangles, centered on (cx, cy).
fn bolt(cx: f32, cy: f32, s: f32, color: Color) {
    draw_triangle(
        vec2(cx + s * 0.18, cy - s * 0.5),
        vec2(cx - s * 0.28, cy + s * 0.08),
        vec2(cx + s * 0.04, cy + s * 0.04),
        color,
    );
    draw_triangle(
        vec2(cx - s * 0.18, cy + s * 0.5),
        vec2(cx + s * 0.28, cy - s * 0.08),
        vec2(cx - s * 0.04, cy - s * 0.04),
        color,
    );
}

fn hint(ly: &Lay, text: &str, color: Color) {
    let fs = ly.panel.h * 0.020;
    let d = measure_text(text, None, fs as u16, 1.0);
    draw_text(text, ly.panel.x + (ly.panel.w - d.width) / 2.0, ly.py(0.955) + fs, fs, color);
}

/// Word-wrap `text` into lines no wider than `max_w` under `measure`.
/// Injected measurer keeps this testable without a graphics context.
fn wrap<F: Fn(&str) -> f32>(text: &str, max_w: f32, measure: F) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        if para.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate) > max_w {
                lines.push(line);
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Draw wrapped text from `y_top`; returns the y below the last line drawn.
/// Truncates with "..." past `max_lines`.
fn draw_wrapped(text: &str, x: f32, y_top: f32, max_w: f32, fs: f32, color: Color, max_lines: usize) -> f32 {
    let lines = wrap(text, max_w, |s| measure_text(s, None, fs as u16, 1.0).width);
    let lh = fs * 1.35;
    for (i, line) in lines.iter().take(max_lines).enumerate() {
        let mut shown = line.clone();
        if i + 1 == max_lines && lines.len() > max_lines {
            shown.push_str(" ...");
        }
        draw_text(&shown, x, y_top + fs + i as f32 * lh, fs, color);
    }
    y_top + lines.len().min(max_lines) as f32 * lh
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Channel, MessageStatus};

    const W: f32 = 480.0;
    const H: f32 = 854.0;

    fn msg(id: u64, body: &str) -> Message {
        Message {
            id,
            channel: Channel::Email,
            sender: "s.devries@example.com".into(),
            subject: "Invoice #4821 charged twice".into(),
            body: body.into(),
            received_at: 0,
            status: MessageStatus::Open,
        }
    }

    fn long_msg(id: u64) -> Message {
        msg(
            id,
            "Hi, I was billed twice for my March invoice. Can you refund one of the charges? \
             Order reference 4821. This has happened before and I would like it fixed for good.",
        )
    }

    fn tap_center(host: &mut CardHost, r: Rect, now: f64) -> CardAction {
        host.handle(Gesture::Tap(vec2(r.x + r.w / 2.0, r.y + r.h / 2.0)), W, H, now)
    }

    #[test]
    fn card_opens_on_the_scout_report() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        let card = host.open.as_ref().unwrap();
        assert_eq!(card.stage, Stage::Scout);
        assert_eq!(card.summary, meta::summarize(&long_msg(1)));
        assert!(card.draft().is_none(), "no draft before the power-up");
    }

    #[test]
    fn read_full_message_and_back() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        tap_center(&mut host, lay(W, H).read_full, 0.0);
        assert_eq!(host.open.as_ref().unwrap().stage, Stage::FullMessage);
        tap_center(&mut host, lay(W, H).back, 0.0);
        assert_eq!(host.open.as_ref().unwrap().stage, Stage::Scout);
    }

    #[test]
    fn power_up_charges_a_deterministic_draft() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        tap_center(&mut host, lay(W, H).power_up, 10.0);
        let card = host.open.as_ref().unwrap();
        assert_eq!(card.stage, Stage::Reply);
        assert!(card.charging(10.1));
        assert!(!card.ready(10.1), "draft is not sendable while charging");
        let expected = reply::generate_draft(&long_msg(1), &card.meta, "", 0);
        assert_eq!(card.draft(), Some(expected.as_str()));
        assert!(card.ready(10.0 + CHARGE_TIME + REVEAL_TIME + 0.01));
    }

    #[test]
    fn swipe_up_sends_only_a_ready_draft() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        // No draft yet: swipe up must not clear anything (story 008 no-auto-jump).
        assert_eq!(host.handle(Gesture::SwipeUp, W, H, 0.0), CardAction::None);
        host.demo_power_up(0.0);
        assert_eq!(host.handle(Gesture::SwipeUp, W, H, 0.1), CardAction::None);
        let done = CHARGE_TIME + REVEAL_TIME + 0.01;
        assert_eq!(host.handle(Gesture::SwipeUp, W, H, done), CardAction::Send);
        assert!(!host.is_open());
    }

    #[test]
    fn backing_out_keeps_the_draft_and_note() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        host.demo_power_up(0.0);
        let ready = CHARGE_TIME + REVEAL_TIME + 0.01;
        {
            let card = host.open.as_mut().unwrap();
            card.set_note("offer refund");
            card.reply.focus = Focus::Draft;
            card.type_char('!', ready);
        }
        let edited = host.open.as_ref().unwrap().draft().unwrap().to_string();
        // Tap outside the panel: back out without sending.
        assert_eq!(host.handle(Gesture::Tap(vec2(1.0, 1.0)), W, H, ready), CardAction::Closed);
        assert!(!host.is_open());
        host.open_for(&long_msg(1), 20.0);
        let card = host.open.as_ref().unwrap();
        assert_eq!(card.stage, Stage::Scout, "reopening leads with the scout report");
        assert_eq!(card.draft(), Some(edited.as_str()), "draft survives the balk");
        assert_eq!(card.note(), "offer refund");
    }

    #[test]
    fn recharge_regenerates_with_the_steering_note() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        host.demo_power_up(0.0);
        let first = host.open.as_ref().unwrap().draft().unwrap().to_string();
        let ready = CHARGE_TIME + REVEAL_TIME + 0.01;
        host.demo_recharge("keep it short", ready);
        let second = host.open.as_ref().unwrap().draft().unwrap().to_string();
        assert_ne!(first, second);
        assert!(second.lines().count() < first.lines().count(), "steering note ignored");
        // Recharging is blocked while the previous charge is still running.
        let variant_before = host.open.as_ref().unwrap().reply.variant;
        host.demo_recharge("another", ready + 0.1);
        assert_eq!(host.open.as_ref().unwrap().reply.variant, variant_before);
    }

    #[test]
    fn editing_applies_to_the_focused_field() {
        let mut card = Card::new(&long_msg(1));
        card.power_up(0.0);
        let ready = CHARGE_TIME + REVEAL_TIME + 0.01;
        card.reply.focus = Focus::Note;
        card.type_char('h', ready);
        card.type_char('i', ready);
        card.backspace(ready);
        assert_eq!(card.note(), "h");
        card.reply.focus = Focus::Draft;
        let before = card.draft().unwrap().to_string();
        card.type_char('!', ready);
        assert_eq!(card.draft().unwrap(), format!("{before}!"));
        card.backspace(ready);
        assert_eq!(card.draft().unwrap(), before);
        // Newlines never enter the single-line note.
        card.reply.focus = Focus::Note;
        card.type_char('\n', ready);
        assert_eq!(card.note(), "h");
    }

    #[test]
    fn thumbs_store_feedback_and_send_attaches_the_final_version() {
        let mut host = CardHost::new();
        host.open_for(&long_msg(1), 0.0);
        tap_center(&mut host, lay(W, H).sum_up, 0.0);
        tap_center(&mut host, lay(W, H).urg_down, 0.0);
        assert_eq!(host.feedback().rating_of(1, AiFeature::Summary), Some(FeedbackRating::Helpful));
        assert_eq!(host.feedback().rating_of(1, AiFeature::Urgency), Some(FeedbackRating::Unhelpful));

        host.demo_power_up(0.0);
        let ready = CHARGE_TIME + REVEAL_TIME + 0.01;
        tap_center(&mut host, lay(W, H).draft_down, ready);
        {
            let card = host.open.as_mut().unwrap();
            card.reply.focus = Focus::Draft;
            card.type_char('!', ready);
        }
        assert_eq!(host.demo_send(ready), CardAction::Send);
        let entry = host
            .feedback()
            .entries()
            .iter()
            .find(|e| e.feature == AiFeature::DraftReply)
            .unwrap();
        assert!(entry.final_value.as_deref().unwrap().ends_with('!'));
        assert_ne!(entry.ai_output, entry.final_value.clone().unwrap());
    }

    #[test]
    fn short_message_summary_is_the_verbatim_body() {
        let m = msg(2, "Short note.");
        let mut host = CardHost::new();
        host.open_for(&m, 0.0);
        assert_eq!(host.open.as_ref().unwrap().summary, "Short note.");
    }

    #[test]
    fn wrap_breaks_on_words_and_keeps_paragraphs() {
        let measure = |s: &str| s.chars().count() as f32;
        let lines = wrap("one two three\n\nfour", 9.0, measure);
        assert_eq!(lines, vec!["one two", "three", "", "four"]);
        // A single overlong word still gets its own line.
        assert_eq!(wrap("extraordinary", 5.0, measure), vec!["extraordinary"]);
    }

    #[test]
    fn reveal_prefix_respects_char_boundaries() {
        let s = "geëscaleerd";
        for i in 0..=10 {
            let p = reveal_prefix(s, i as f32 / 10.0);
            assert!(s.starts_with(p));
        }
        assert_eq!(reveal_prefix(s, 1.0), s);
        assert_eq!(reveal_prefix(s, 0.0), "");
    }
}
