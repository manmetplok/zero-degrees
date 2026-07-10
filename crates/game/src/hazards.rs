//! Hazard zones (story 010): recurring-theme detection over the open and
//! recent messages on the track. The real system calls an AI clustering
//! endpoint (see api-changerequests/course-search.md); this module stands in
//! with deterministic keyword clustering, designed together with the inbox
//! generator so its themed templates emit exactly the keywords matched here —
//! a nightmare-Monday checkout spike genuinely surfaces as a zone. Zone
//! briefings are templated from cluster stats. Pure logic, no rendering.

use shared::MessageStatus;

use crate::meta::{self, Sentiment, Urgency};
use crate::track::{Hurdle, HURDLE_SPACING};

/// Minimum open hurdles sharing a theme in one stretch to form a zone.
pub const MIN_ZONE: usize = 4;
/// Maximum track gap between themed hurdles to count as the same stretch
/// (at most one unrelated hurdle in between).
const MAX_GAP: f32 = HURDLE_SPACING * 2.5;
/// Track margin the zone band extends past its outermost hurdles.
const ZONE_MARGIN: f32 = 1.5;

struct ThemeDef {
    title: &'static str,
    root_cause: &'static str,
    keywords: &'static [&'static str],
}

/// Order matters: the first matching theme claims a message, so the more
/// specific vocabularies come first.
const THEMES: &[ThemeDef] = &[
    ThemeDef {
        title: "Checkout failures",
        root_cause: "The payment step at checkout is failing; reports point at the payment provider.",
        keywords: &[
            "checkout",
            "afrekenen",
            "betaalstap",
            "card declined",
            "payment step",
        ],
    },
    ThemeDef {
        title: "API errors",
        root_cause: "The export API and webhooks are throwing 500s; likely one backend incident.",
        keywords: &["api", "webhook", "export", "500"],
    },
    ThemeDef {
        title: "Duplicate charges",
        root_cause: "Renewals were charged twice this cycle; refund and audit the billing run.",
        keywords: &[
            "charged twice",
            "billed twice",
            "duplicate charge",
            "dubbel afgeschreven",
            "twee keer afgeschreven",
        ],
    },
    ThemeDef {
        title: "Login & password trouble",
        root_cause: "Password-reset tokens expire immediately, locking customers out.",
        keywords: &[
            "password",
            "wachtwoord",
            "reset link",
            "resetlink",
            "locked out",
            "log in",
            "inloggen",
            "sso",
        ],
    },
    ThemeDef {
        title: "Delivery delays",
        root_cause: "Shipments are stuck and tracking stopped updating; looks like a carrier issue.",
        keywords: &[
            "deliver",
            "package",
            "pakket",
            "bezorgd",
            "geleverd",
            "arrived",
            "onderweg",
        ],
    },
];

/// A named stretch of track where many hurdles share a root cause.
#[derive(Debug, Clone)]
pub struct HazardZone {
    pub title: &'static str,
    root_cause: &'static str,
    /// Every message in the stretch, any status.
    pub ids: Vec<u64>,
    /// How many of those are still open (drives the banner count).
    pub open: usize,
    /// Track span the zone covers, including a margin.
    pub start: f32,
    pub end: f32,
    angry: usize,
    negative: usize,
    peak: Urgency,
    window_secs: i64,
}

impl HazardZone {
    /// Banner text, e.g. "Checkout failures · 14 hurdles".
    pub fn label(&self) -> String {
        format!("{} · {} hurdles", self.title, self.open)
    }

    pub fn contains(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }

    /// Zone briefing summarizing the common root cause; templated stand-in
    /// for the future AI-written text.
    pub fn briefing(&self) -> Vec<String> {
        let mins = (self.window_secs / 60).max(1);
        vec![
            format!("Root cause: {}", self.root_cause),
            format!(
                "{} reports in {} min; mood: {} angry, {} negative.",
                self.ids.len(),
                mins,
                self.angry,
                self.negative
            ),
            format!(
                "Peak height: {}. Clear the stretch to defuse it.",
                self.peak.label()
            ),
        ]
    }
}

fn theme_of(hurdle: &Hurdle) -> Option<usize> {
    let text = format!("{} {}", hurdle.message.subject, hurdle.message.body).to_lowercase();
    THEMES
        .iter()
        .position(|t| t.keywords.iter().any(|k| text.contains(k)))
}

fn build_zone(theme: &ThemeDef, run: &[&Hurdle]) -> HazardZone {
    let mut angry = 0;
    let mut negative = 0;
    let mut peak = Urgency::Low;
    for h in run {
        let m = meta::enrich(&h.message);
        match m.sentiment {
            Sentiment::Angry => angry += 1,
            Sentiment::Negative => negative += 1,
            _ => {}
        }
        peak = peak.max(m.urgency);
    }
    let first_ts = run.iter().map(|h| h.message.received_at).min().unwrap_or(0);
    let last_ts = run.iter().map(|h| h.message.received_at).max().unwrap_or(0);
    HazardZone {
        title: theme.title,
        root_cause: theme.root_cause,
        ids: run.iter().map(|h| h.message.id).collect(),
        open: run
            .iter()
            .filter(|h| h.message.status == MessageStatus::Open)
            .count(),
        start: run.first().map(|h| h.at).unwrap_or(0.0) - ZONE_MARGIN,
        end: run.last().map(|h| h.at).unwrap_or(0.0) + ZONE_MARGIN,
        angry,
        negative,
        peak,
        window_secs: last_ts - first_ts,
    }
}

/// Detect hazard zones on the current track: dense stretches of hurdles
/// sharing a theme, with at least `MIN_ZONE` still open. Deterministic; zones
/// dissolve as their hurdles get cleared. Sorted by track position.
pub fn detect_zones(hurdles: &[Hurdle]) -> Vec<HazardZone> {
    let mut zones = Vec::new();
    for (ti, theme) in THEMES.iter().enumerate() {
        let mut matched: Vec<&Hurdle> = hurdles
            .iter()
            .filter(|h| theme_of(h) == Some(ti))
            .collect();
        matched.sort_by(|a, b| a.at.total_cmp(&b.at));

        let mut run: Vec<&Hurdle> = Vec::new();
        for h in matched {
            if let Some(prev) = run.last() {
                if h.at - prev.at > MAX_GAP {
                    flush(theme, &mut run, &mut zones);
                }
            }
            run.push(h);
        }
        flush(theme, &mut run, &mut zones);
    }
    zones.sort_by(|a, b| a.start.total_cmp(&b.start));
    zones
}

fn flush(theme: &ThemeDef, run: &mut Vec<&Hurdle>, zones: &mut Vec<HazardZone>) {
    let open = run
        .iter()
        .filter(|h| h.message.status == MessageStatus::Open)
        .count();
    if open >= MIN_ZONE {
        zones.push(build_zone(theme, run));
    }
    run.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{generate_course, CourseSpec, Difficulty};
    use crate::track::Track;
    use shared::{Channel, Message};

    fn track_for(difficulty: Difficulty) -> Track {
        Track::new(generate_course(&CourseSpec {
            seed: 42,
            difficulty,
            count: 50,
        }))
    }

    #[test]
    fn nightmare_checkout_spike_becomes_a_hazard_zone() {
        let track = track_for(Difficulty::NightmareMonday);
        let zones = detect_zones(&track.hurdles);
        let checkout = zones
            .iter()
            .find(|z| z.title == "Checkout failures")
            .expect("nightmare course must surface the checkout spike");
        assert!(checkout.open >= 10, "spike too small: {}", checkout.open);
        assert!(checkout.end > checkout.start);
        assert!(checkout.label().contains("Checkout failures ·"));
        // The zone groups real checkout hurdles.
        for id in &checkout.ids {
            let h = track.hurdles.iter().find(|h| h.message.id == *id).unwrap();
            let text = format!("{} {}", h.message.subject, h.message.body).to_lowercase();
            assert!(
                ["checkout", "afrekenen", "betaalstap", "card declined", "payment step"]
                    .iter()
                    .any(|k| text.contains(k)),
                "{text}"
            );
        }
    }

    #[test]
    fn chill_jog_has_no_checkout_zone() {
        let zones = detect_zones(&track_for(Difficulty::ChillJog).hurdles);
        assert!(zones.iter().all(|z| z.title != "Checkout failures"));
    }

    #[test]
    fn detection_is_deterministic() {
        let track = track_for(Difficulty::NightmareMonday);
        let a = detect_zones(&track.hurdles);
        let b = detect_zones(&track.hurdles);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }

    #[test]
    fn briefing_summarizes_the_cluster() {
        let track = track_for(Difficulty::NightmareMonday);
        let zones = detect_zones(&track.hurdles);
        let zone = zones.iter().find(|z| z.title == "Checkout failures").unwrap();
        let briefing = zone.briefing();
        assert!(briefing[0].contains("payment"));
        assert!(briefing[1].contains("reports in"));
    }

    fn checkoutish(id: u64, status: MessageStatus) -> Message {
        Message {
            id,
            channel: Channel::WebForm,
            sender: "Contact form".into(),
            subject: "Checkout fails at the payment step".into(),
            body: "The payment step at checkout fails with error CHK-500.".into(),
            received_at: 1_780_000_000 + id as i64 * 60,
            status,
        }
    }

    #[test]
    fn zone_needs_enough_open_hurdles_and_dissolves_when_cleared() {
        let build = |cleared: usize| {
            let mut messages: Vec<Message> =
                (1..=5).map(|i| checkoutish(i, MessageStatus::Open)).collect();
            for m in messages.iter_mut().take(cleared) {
                m.status = MessageStatus::Cleared;
            }
            Track::new(messages)
        };
        assert_eq!(detect_zones(&build(0).hurdles).len(), 1);
        assert_eq!(detect_zones(&build(1).hurdles).len(), 1);
        // Clearing below MIN_ZONE dissolves the zone.
        assert!(detect_zones(&build(2).hurdles).is_empty());
    }

    #[test]
    fn scattered_matches_do_not_merge_across_big_gaps() {
        // Two checkout messages far apart with unrelated hurdles between:
        // no dense stretch, so no zone.
        let mut messages = vec![checkoutish(1, MessageStatus::Open)];
        for i in 2..=6 {
            messages.push(Message {
                id: i,
                channel: Channel::Email,
                sender: "x@example.com".into(),
                subject: "Change of address".into(),
                body: "Please update our shipping address.".into(),
                received_at: 1_780_000_000 + i as i64 * 60,
                status: MessageStatus::Open,
            });
        }
        messages.push(checkoutish(7, MessageStatus::Open));
        assert!(detect_zones(&Track::new(messages).hurdles).is_empty());
    }
}
