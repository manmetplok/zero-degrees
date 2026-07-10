//! Message metadata: the AI-derived triage signals (category, urgency,
//! sentiment) and the pure rules built on them — track priority, response
//! targets, overdue detection, clear rewards, manual category overrides,
//! and the response log kept for race control.
//!
//! `enrich` is a deterministic keyword mock standing in for backend AI
//! endpoints that don't exist yet (see api-changerequests/hurdle-metadata.md).
//! Everything here is clock-free: callers pass unix timestamps in, so the
//! rules stay testable without macroquad.

use std::collections::HashMap;
use std::path::PathBuf;

use shared::Message;

use crate::score;

/// Message topic, rendered as the hurdle's visual type (story 003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Billing,
    Complaint,
    Question,
    Feedback,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Billing,
        Category::Complaint,
        Category::Question,
        Category::Feedback,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Billing => "Billing",
            Category::Complaint => "Complaint",
            Category::Question => "Question",
            Category::Feedback => "Feedback",
        }
    }

    /// Next category in the manual re-typing cycle (tap the type chip).
    pub fn next(self) -> Category {
        match self {
            Category::Billing => Category::Complaint,
            Category::Complaint => Category::Question,
            Category::Question => Category::Feedback,
            Category::Feedback => Category::Billing,
        }
    }

    /// Stable identifier used in the local override store and the API.
    pub fn slug(self) -> &'static str {
        match self {
            Category::Billing => "billing",
            Category::Complaint => "complaint",
            Category::Question => "question",
            Category::Feedback => "feedback",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.slug() == slug)
    }
}

/// How time-critical a message is; rendered as hurdle height (story 004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Critical,
    High,
    Normal,
    Low,
}

impl Urgency {
    /// For settings and filter UIs (built elsewhere).
    #[allow(dead_code)]
    pub const ALL: [Urgency; 4] = [
        Urgency::Critical,
        Urgency::High,
        Urgency::Normal,
        Urgency::Low,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Urgency::Critical => "critical",
            Urgency::High => "high",
            Urgency::Normal => "normal",
            Urgency::Low => "low",
        }
    }

    /// XP for clearing a hurdle of this urgency, before combo and timing
    /// multipliers. `Low` pays the flat `score::BASE_XP`; taller hurdles
    /// pay more (story 004).
    pub const fn base_xp(self) -> u32 {
        match self {
            Urgency::Critical => score::BASE_XP * 4,
            Urgency::High => score::BASE_XP * 5 / 2,
            Urgency::Normal => score::BASE_XP * 3 / 2,
            Urgency::Low => score::BASE_XP,
        }
    }

    /// Response-time target in seconds: how long a message may wait before
    /// its hurdle catches fire (story 014). Configurable per urgency; these
    /// are the defaults until the backend serves team settings.
    pub const fn response_target(self) -> i64 {
        match self {
            Urgency::Critical => 15 * 60,
            Urgency::High => 60 * 60,
            Urgency::Normal => 4 * 60 * 60,
            Urgency::Low => 8 * 60 * 60,
        }
    }
}

/// Customer mood, rendered as an aura around the hurdle (story 005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
    Angry,
}

impl Sentiment {
    pub fn label(self) -> &'static str {
        match self {
            Sentiment::Positive => "positive",
            Sentiment::Neutral => "neutral",
            Sentiment::Negative => "negative",
            Sentiment::Angry => "angry",
        }
    }
}

/// The triage signals for one message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MessageMeta {
    pub category: Category,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
    /// The phrase that drove the urgency call — the "why it matters" signal
    /// shown on the detail card so the game reading never hides real triage.
    pub signal: Option<&'static str>,
    /// True when the category comes from a manual override, not the AI.
    pub overridden: bool,
}

// Keyword tables for the deterministic mock classifier. First match wins,
// scanned in declaration order against the lowercased subject + body.
const KW_COMPLAINT: &[&str] = &[
    "complaint", "slow", "nobody answered", "flooding", "hammering",
    "locked out", "fails", "broken", "not working", "unacceptable", " 500",
];
const KW_BILLING: &[&str] = &[
    "invoice", "charged", "billed", "refund", "payment", "pricing",
    "subscription", "cancel",
];
const KW_FEEDBACK: &[&str] = &["★★★★", "thank", "love it", "great service", "keep customers"];

const KW_CRITICAL: &[&str] = &[
    "urgent", "asap", "service is down", "is down", "legal", "locked out",
    "outage", "emergency",
];
const KW_HIGH: &[&str] = &[
    "blocks", "blocking", "fails", " 500", "twice", "flooding", "hammering",
    "can't", "cannot", "expired",
];
const KW_LOW: &[&str] = &["★★★★", "do you offer", "no longer need", "thank"];

const KW_ANGRY: &[&str] = &["asap", "urgent", "hammering", "!!", "angry", "furious", "unacceptable"];
const KW_NEGATIVE: &[&str] = &[
    "twice", "fails", "expired", "slow", "nobody answered", "keeps saying",
    "blocks", "locked out", "☆",
];
const KW_POSITIVE: &[&str] = &["★★★★★", "great", "thank", "fixed", "love"];

fn find_keyword(text: &str, keywords: &'static [&'static str]) -> Option<&'static str> {
    keywords.iter().copied().find(|kw| text.contains(kw))
}

/// Deterministic mock of the AI enrichment call: classifies category,
/// urgency, and sentiment from keywords. Same message in, same meta out.
pub fn enrich(message: &Message) -> MessageMeta {
    let text = format!("{} {}", message.subject, message.body).to_lowercase();
    let category = if find_keyword(&text, KW_COMPLAINT).is_some() {
        Category::Complaint
    } else if find_keyword(&text, KW_BILLING).is_some() {
        Category::Billing
    } else if find_keyword(&text, KW_FEEDBACK).is_some() {
        Category::Feedback
    } else {
        Category::Question
    };
    let (urgency, signal) = if let Some(kw) = find_keyword(&text, KW_CRITICAL) {
        (Urgency::Critical, Some(kw))
    } else if let Some(kw) = find_keyword(&text, KW_HIGH) {
        (Urgency::High, Some(kw))
    } else if let Some(kw) = find_keyword(&text, KW_LOW) {
        (Urgency::Low, Some(kw))
    } else {
        (Urgency::Normal, None)
    };
    let sentiment = if find_keyword(&text, KW_ANGRY).is_some() {
        Sentiment::Angry
    } else if find_keyword(&text, KW_NEGATIVE).is_some() {
        Sentiment::Negative
    } else if find_keyword(&text, KW_POSITIVE).is_some() {
        Sentiment::Positive
    } else {
        Sentiment::Neutral
    };
    MessageMeta {
        category,
        urgency,
        sentiment,
        signal,
        overridden: false,
    }
}

/// `enrich` plus the player's manual category override, if any (story 003).
pub fn enriched(message: &Message, overrides: &Overrides) -> MessageMeta {
    let mut meta = enrich(message);
    if let Some(category) = overrides.get(message.id) {
        meta.overridden = category != meta.category;
        meta.category = category;
    }
    meta
}

// ---- scout report (story 006) ----

/// Bodies at or under this length are shown verbatim on the card and no
/// summary model call is made (story 006).
pub const SUMMARY_THRESHOLD: usize = 120;

/// Deterministic mock of the AI scout-report call: short bodies come back
/// verbatim, longer ones lead with the subject and the first sentence.
pub fn summarize(message: &Message) -> String {
    if message.body.len() <= SUMMARY_THRESHOLD {
        return message.body.clone();
    }
    let first_sentence = message
        .body
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or(&message.body)
        .trim();
    format!("{}: {}.", message.subject, first_sentence)
}

/// One-line "why it matters" for the urgency call, shown on the detail card
/// so the game reading never hides the real triage data (story 004).
pub fn why_it_matters(meta: &MessageMeta) -> String {
    match (meta.urgency, meta.signal) {
        (Urgency::Critical, Some(kw)) => {
            format!("Time-critical: the message says \"{}\".", kw.trim())
        }
        (Urgency::High, Some(kw)) => {
            format!("Something is failing for the customer (\"{}\").", kw.trim())
        }
        (Urgency::Low, Some(kw)) => format!("Routine: \"{}\" signals no time pressure.", kw.trim()),
        _ => "No urgency signals found; the default response target applies.".to_string(),
    }
}

// ---- priority (story 009) ----

// Urgency bands are far enough apart that mood + age (capped below) can lift
// a waiting hurdle past the next band's fresh arrivals, but never past a
// fresh critical: critical always cuts to the front.
const AGE_WEIGHT: f32 = 20.0;
const AGE_CAP: f32 = 25.0;

/// Track priority: higher sorts nearer the runner. Urgency dominates,
/// customer mood adds pressure, and waiting time (relative to the urgency's
/// response target) makes ignored hurdles creep up the track.
pub fn priority(meta: &MessageMeta, age_secs: i64) -> f32 {
    let urgency = match meta.urgency {
        Urgency::Critical => 100.0,
        Urgency::High => 60.0,
        Urgency::Normal => 30.0,
        Urgency::Low => 10.0,
    };
    let mood = match meta.sentiment {
        Sentiment::Angry => 12.0,
        Sentiment::Negative => 6.0,
        Sentiment::Neutral | Sentiment::Positive => 0.0,
    };
    let target = meta.urgency.response_target() as f32;
    let age = (age_secs.max(0) as f32 / target * AGE_WEIGHT).min(AGE_CAP);
    urgency + mood + age
}

// ---- response timing (story 014) ----

/// Multiplier on the base reward for clearing within the response target.
pub const SPEED_BONUS: f32 = 1.25;
/// Fraction of the base reward still paid for clearing an overdue hurdle.
pub const LATE_FACTOR: f32 = 0.5;

/// A hurdle burns once its message has waited past the urgency's target.
pub fn is_overdue(urgency: Urgency, waited_secs: i64) -> bool {
    waited_secs > urgency.response_target()
}

/// XP a clear pays before the combo multiplier: the urgency's base with a
/// speed bonus when on time, partial points once the hurdle is burning.
pub fn clear_xp(urgency: Urgency, waited_secs: i64) -> u32 {
    let base = urgency.base_xp() as f32;
    let factor = if is_overdue(urgency, waited_secs) {
        LATE_FACTOR
    } else {
        SPEED_BONUS
    };
    (base * factor).round() as u32
}

/// Compact waiting-time label: "<1m", "12m", "1h 05m", "2d 3h".
pub fn format_wait(secs: i64) -> String {
    let s = secs.max(0);
    let (m, h, d) = (s / 60, s / 3600, s / 86_400);
    if s < 60 {
        "<1m".to_string()
    } else if h == 0 {
        format!("{m}m")
    } else if d == 0 {
        format!("{}h {:02}m", h, (s % 3600) / 60)
    } else {
        format!("{}d {}h", d, (s % 86_400) / 3600)
    }
}

// ---- demo clock ----

/// The mock inbox (inbox.rs) stamps messages near 1_780_000_000. The demo
/// clock starts two hours later — so the course opens with a mix of burning,
/// nearly-due, and comfortable hurdles — and runs at 60x (one game hour per
/// real minute) so countdowns and ignition are visible in a short session.
pub const DEMO_EPOCH: i64 = 1_780_000_000 + 2 * 3600;
pub const DEMO_TIME_SCALE: f64 = 60.0;

/// Demo "now" as a unix timestamp, from seconds since app start.
pub fn demo_now(elapsed: f64) -> i64 {
    DEMO_EPOCH + (elapsed * DEMO_TIME_SCALE) as i64
}

// ---- manual category overrides (story 003) ----

/// Manual category overrides, persisted as a tiny TSV next to the save data.
/// Local-only until the backend grows a category-override endpoint (see
/// api-changerequests/hurdle-metadata.md).
pub struct Overrides {
    map: HashMap<u64, Category>,
    /// None keeps the store memory-only (tests).
    path: Option<PathBuf>,
}

impl Overrides {
    /// Store under `$ZD_DATA_DIR`, or the OS temp dir until save data has a
    /// real home (see ARCHITECTURE.md open items on persistence).
    pub fn load_default() -> Self {
        let dir = std::env::var_os("ZD_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Self::load(dir.join("zero-degrees-overrides.tsv"))
    }

    pub fn load(path: PathBuf) -> Self {
        let map = std::fs::read_to_string(&path)
            .map(|text| parse_overrides(&text))
            .unwrap_or_default();
        Self {
            map,
            path: Some(path),
        }
    }

    /// Memory-only store, for tests.
    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self {
            map: HashMap::new(),
            path: None,
        }
    }

    pub fn get(&self, message_id: u64) -> Option<Category> {
        self.map.get(&message_id).copied()
    }

    pub fn set(&mut self, message_id: u64, category: Category) {
        self.map.insert(message_id, category);
        if let Some(path) = &self.path {
            let _ = std::fs::write(path, serialize_overrides(&self.map));
        }
    }
}

/// One override per line: `<message id>\t<category slug>`.
pub fn parse_overrides(text: &str) -> HashMap<u64, Category> {
    text.lines()
        .filter_map(|line| {
            let (id, slug) = line.split_once('\t')?;
            Some((id.trim().parse().ok()?, Category::from_slug(slug.trim())?))
        })
        .collect()
}

pub fn serialize_overrides(map: &HashMap<u64, Category>) -> String {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(id, _)| **id);
    entries
        .into_iter()
        .map(|(id, category)| format!("{id}\t{}\n", category.slug()))
        .collect()
}

// ---- response log (story 014, read by race control) ----

/// One cleared hurdle's response time, kept for team statistics and the
/// race-control screen (story 011, built elsewhere — hence the unused-field
/// allowance).
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct ResponseRecord {
    pub message_id: u64,
    pub category: Category,
    pub urgency: Urgency,
    pub waited_secs: i64,
    pub on_time: bool,
}

/// In-memory record of cleared hurdles' response times.
#[derive(Default)]
pub struct ResponseLog {
    pub records: Vec<ResponseRecord>,
}

impl ResponseLog {
    pub fn record(&mut self, record: ResponseRecord) {
        self.records.push(record);
    }

    /// For race control (built elsewhere).
    #[allow(dead_code)]
    pub fn on_time_count(&self) -> usize {
        self.records.iter().filter(|r| r.on_time).count()
    }

    /// For race control (built elsewhere).
    #[allow(dead_code)]
    pub fn overdue_count(&self) -> usize {
        self.records.len() - self.on_time_count()
    }

    /// For race control (built elsewhere).
    #[allow(dead_code)]
    pub fn average_wait_secs(&self) -> Option<i64> {
        if self.records.is_empty() {
            return None;
        }
        let total: i64 = self.records.iter().map(|r| r.waited_secs).sum();
        Some(total / self.records.len() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::sample_messages;

    #[test]
    fn enrich_is_deterministic_and_reads_the_samples() {
        let messages = sample_messages(8);
        for message in &messages {
            assert_eq!(enrich(message), enrich(message));
        }
        // Spot-check known samples: billing double-charge, locked-out demo,
        // five-star praise.
        let billing = enrich(&messages[0]);
        assert_eq!(billing.category, Category::Billing);
        assert_eq!(billing.urgency, Urgency::High);
        assert_eq!(billing.sentiment, Sentiment::Negative);
        let locked_out = enrich(&messages[7]);
        assert_eq!(locked_out.urgency, Urgency::Critical);
        assert_eq!(locked_out.sentiment, Sentiment::Angry);
        assert!(locked_out.signal.is_some());
        let praise = enrich(&messages[6]);
        assert_eq!(praise.category, Category::Feedback);
        assert_eq!(praise.urgency, Urgency::Low);
        assert_eq!(praise.sentiment, Sentiment::Positive);
    }

    #[test]
    fn override_changes_category_and_flags_it() {
        let message = &sample_messages(1)[0];
        let mut overrides = Overrides::in_memory();
        let ai = enriched(message, &overrides);
        assert!(!ai.overridden);
        overrides.set(message.id, ai.category.next());
        let manual = enriched(message, &overrides);
        assert_eq!(manual.category, ai.category.next());
        assert!(manual.overridden);
        // Everything but the category is untouched.
        assert_eq!(manual.urgency, ai.urgency);
        assert_eq!(manual.sentiment, ai.sentiment);
    }

    #[test]
    fn overrides_roundtrip_through_the_tsv_format() {
        let mut map = HashMap::new();
        map.insert(3, Category::Feedback);
        map.insert(1, Category::Complaint);
        let text = serialize_overrides(&map);
        assert_eq!(parse_overrides(&text), map);
        // Garbage lines are ignored rather than poisoning the store.
        assert!(parse_overrides("nonsense\n7\tnot-a-category\n").is_empty());
    }

    #[test]
    fn category_cycle_visits_every_category() {
        let mut seen = vec![Category::Billing];
        while seen.len() < Category::ALL.len() {
            seen.push(seen.last().unwrap().next());
        }
        for category in Category::ALL {
            assert!(seen.contains(&category));
        }
        assert_eq!(seen.last().unwrap().next(), Category::Billing);
    }

    fn meta_with(urgency: Urgency, sentiment: Sentiment) -> MessageMeta {
        MessageMeta {
            category: Category::Question,
            urgency,
            sentiment,
            signal: None,
            overridden: false,
        }
    }

    #[test]
    fn priority_rises_with_urgency_and_age() {
        let fresh = |u| priority(&meta_with(u, Sentiment::Neutral), 0);
        assert!(fresh(Urgency::Critical) > fresh(Urgency::High));
        assert!(fresh(Urgency::High) > fresh(Urgency::Normal));
        assert!(fresh(Urgency::Normal) > fresh(Urgency::Low));
        let normal = meta_with(Urgency::Normal, Sentiment::Neutral);
        assert!(priority(&normal, 3600) > priority(&normal, 0));
    }

    #[test]
    fn fresh_critical_outranks_any_aged_lower_hurdle() {
        let critical = priority(&meta_with(Urgency::Critical, Sentiment::Neutral), 0);
        let aged_angry_high = priority(&meta_with(Urgency::High, Sentiment::Angry), 30 * 86_400);
        assert!(critical > aged_angry_high);
    }

    #[test]
    fn long_waiting_angry_normal_creeps_past_a_fresh_high() {
        let waiting = priority(&meta_with(Urgency::Normal, Sentiment::Angry), 16 * 3600);
        let fresh_high = priority(&meta_with(Urgency::High, Sentiment::Neutral), 0);
        assert!(waiting > fresh_high);
    }

    #[test]
    fn overdue_starts_past_the_urgency_target() {
        let target = Urgency::High.response_target();
        assert!(!is_overdue(Urgency::High, target));
        assert!(is_overdue(Urgency::High, target + 1));
        // Criticals burn far sooner than lows.
        assert!(Urgency::Critical.response_target() < Urgency::Low.response_target());
    }

    #[test]
    fn on_time_clears_earn_a_speed_bonus_and_late_clears_partial_points() {
        let base = Urgency::High.base_xp();
        let on_time = clear_xp(Urgency::High, 60);
        let late = clear_xp(Urgency::High, Urgency::High.response_target() + 60);
        assert!(on_time > base);
        assert!(late < base);
        assert_eq!(on_time, (base as f32 * SPEED_BONUS).round() as u32);
        assert_eq!(late, (base as f32 * LATE_FACTOR).round() as u32);
    }

    #[test]
    fn taller_hurdles_pay_more_and_low_pays_the_flat_base() {
        assert_eq!(Urgency::Low.base_xp(), crate::score::BASE_XP);
        assert!(Urgency::Normal.base_xp() > Urgency::Low.base_xp());
        assert!(Urgency::High.base_xp() > Urgency::Normal.base_xp());
        assert!(Urgency::Critical.base_xp() > Urgency::High.base_xp());
    }

    #[test]
    fn wait_labels_read_naturally() {
        assert_eq!(format_wait(30), "<1m");
        assert_eq!(format_wait(12 * 60), "12m");
        assert_eq!(format_wait(3900), "1h 05m");
        assert_eq!(format_wait(2 * 86_400 + 3 * 3600), "2d 3h");
        assert_eq!(format_wait(-5), "<1m");
    }
}
