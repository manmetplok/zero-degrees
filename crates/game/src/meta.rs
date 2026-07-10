//! AI enrichment metadata for messages: category, urgency, sentiment,
//! summary. This is the single interface every gameplay feature reads;
//! the backend AI endpoints that should produce these values do not exist
//! yet (see api-changerequest.md), so `enrich` stands in with deterministic
//! keyword heuristics over the message text. Swapping in real AI output
//! later only touches this module.

use shared::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Urgency {
    Low,
    Normal,
    High,
    Critical,
}

impl Urgency {
    pub const ALL: [Urgency; 4] = [
        Urgency::Low,
        Urgency::Normal,
        Urgency::High,
        Urgency::Critical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Urgency::Low => "Low",
            Urgency::Normal => "Normal",
            Urgency::High => "High",
            Urgency::Critical => "Critical",
        }
    }

    /// Point reward scaling with hurdle height (story 004).
    pub fn base_xp(self) -> u32 {
        match self {
            Urgency::Low => 60,
            Urgency::Normal => 100,
            Urgency::High => 160,
            Urgency::Critical => 250,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
    Angry,
}

impl Sentiment {
    pub const ALL: [Sentiment; 4] = [
        Sentiment::Positive,
        Sentiment::Neutral,
        Sentiment::Negative,
        Sentiment::Angry,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Sentiment::Positive => "Positive",
            Sentiment::Neutral => "Neutral",
            Sentiment::Negative => "Negative",
            Sentiment::Angry => "Angry",
        }
    }
}

/// Everything the AI derives about one message.
#[derive(Debug, Clone)]
pub struct MessageMeta {
    pub category: Category,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
    /// One-to-two sentence scout report (story 006).
    pub summary: String,
    /// Why the urgency is what it is — "height is honest" (story 004).
    pub urgency_reason: String,
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Derive metadata for a message. Deterministic: same message, same result.
pub fn enrich(message: &Message) -> MessageMeta {
    let text = format!("{} {}", message.subject, message.body).to_lowercase();

    let category = if contains_any(
        &text,
        &["invoice", "billed", "charge", "refund", "pricing", "subscription", "cancel"],
    ) {
        Category::Billing
    } else if contains_any(
        &text,
        &["complaint", "slow", "nobody answered", "flooding", "unacceptable", "☆"],
    ) {
        Category::Complaint
    } else if text.contains('?') || contains_any(&text, &["question", "how do", "do you"]) {
        Category::Question
    } else {
        Category::Feedback
    };

    let (urgency, urgency_reason) = if contains_any(
        &text,
        &["urgent", "asap", "down", "locked out", "legal", "blocks", "demo in"],
    ) {
        (
            Urgency::Critical,
            "Time-critical: the customer is blocked right now.".to_string(),
        )
    } else if contains_any(&text, &["fails", "broken", "error", "500", "can't", "expired"]) {
        (
            Urgency::High,
            "Something is not working for the customer.".to_string(),
        )
    } else if category == Category::Billing {
        (
            Urgency::Normal,
            "Money-related; should be handled within the day.".to_string(),
        )
    } else {
        (
            Urgency::Low,
            "Routine message with no time pressure.".to_string(),
        )
    };

    let sentiment = if contains_any(&text, &["★★★★", "great", "thanks", "fixed", "keep customers"]) {
        Sentiment::Positive
    } else if contains_any(&text, &["hammering", "unacceptable", "!!!", "flooding"]) {
        Sentiment::Angry
    } else if contains_any(&text, &["slow", "fails", "twice", "nobody answered", "locked out"]) {
        Sentiment::Negative
    } else {
        Sentiment::Neutral
    };

    let summary = summarize(message);

    MessageMeta {
        category,
        urgency,
        sentiment,
        summary,
        urgency_reason,
    }
}

/// Threshold under which the original text is shown instead of a summary
/// (story 006, "short message" scenario).
pub const SUMMARY_THRESHOLD: usize = 120;

fn summarize(message: &Message) -> String {
    if message.body.len() <= SUMMARY_THRESHOLD {
        return message.body.clone();
    }
    // Mock of the AI summary: lead with the subject, then the first sentence.
    let first_sentence = message
        .body
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or(&message.body)
        .trim();
    format!("{}: {}.", message.subject, first_sentence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Channel, MessageStatus};

    fn msg(subject: &str, body: &str) -> Message {
        Message {
            id: 1,
            channel: Channel::Email,
            sender: "test@example.com".into(),
            subject: subject.into(),
            body: body.into(),
            received_at: 0,
            status: MessageStatus::Open,
        }
    }

    #[test]
    fn urgent_outage_is_critical_billing_stays_normal() {
        let outage = enrich(&msg("Urgent: locked out", "Our whole team is locked out ASAP."));
        assert_eq!(outage.urgency, Urgency::Critical);
        let billing = enrich(&msg("Invoice question", "I was billed for the wrong plan amount."));
        assert_eq!(billing.category, Category::Billing);
    }

    #[test]
    fn short_message_is_shown_verbatim() {
        let m = msg("Hi", "Short note.");
        assert_eq!(enrich(&m).summary, "Short note.");
    }

    #[test]
    fn enrich_is_deterministic() {
        let m = msg("Slow delivery", "It took three weeks to arrive and nobody answered my emails in between, which honestly made me doubt whether anyone was reading them.");
        assert_eq!(enrich(&m).summary, enrich(&m).summary);
        assert_eq!(enrich(&m).sentiment, Sentiment::Negative);
    }
}
