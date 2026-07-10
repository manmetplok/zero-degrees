use shared::Urgency;

pub struct UrgencyScore {
    pub urgency: Urgency,
    pub rationale: String,
}

pub trait UrgencyScorer {
    fn score(&self, subject: &str, body: &str) -> UrgencyScore;
}

const CRITICAL_KEYWORDS: &[&str] = &[
    "service is down",
    "site is down",
    "system is down",
    "down for everyone",
    "data breach",
    "security breach",
    "legal complaint",
    "lawsuit",
    "sue us",
];

const HIGH_KEYWORDS: &[&str] = &[
    "refund",
    "cancel my",
    "cancel this",
    "not working",
    "broken",
    "escalate",
    "unacceptable",
    "asap",
    "locked out",
];

const LOW_KEYWORDS: &[&str] = &[
    "fyi",
    "for your information",
    "no rush",
    "no action needed",
    "no action required",
    "just letting you know",
    "informational only",
];

pub struct KeywordUrgencyScorer;

impl UrgencyScorer for KeywordUrgencyScorer {
    fn score(&self, subject: &str, body: &str) -> UrgencyScore {
        let text = format!("{subject} {body}").to_lowercase();

        if let Some(keyword) = CRITICAL_KEYWORDS.iter().find(|k| text.contains(*k)) {
            return UrgencyScore {
                urgency: Urgency::Critical,
                rationale: format!(
                    "Mentions \"{keyword}\", a sign of active, business-critical impact."
                ),
            };
        }
        if let Some(keyword) = HIGH_KEYWORDS.iter().find(|k| text.contains(*k)) {
            return UrgencyScore {
                urgency: Urgency::High,
                rationale: format!(
                    "Mentions \"{keyword}\", suggesting a frustrated customer who needs a fast response."
                ),
            };
        }
        if let Some(keyword) = LOW_KEYWORDS.iter().find(|k| text.contains(*k)) {
            return UrgencyScore {
                urgency: Urgency::Low,
                rationale: format!(
                    "Mentions \"{keyword}\", a routine, informational note with no action pending."
                ),
            };
        }
        UrgencyScore {
            urgency: Urgency::Normal,
            rationale: "No urgent or routine signal detected; treated as standard priority."
                .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(subject: &str, body: &str) -> Urgency {
        KeywordUrgencyScorer.score(subject, body).urgency
    }

    #[test]
    fn service_outage_is_critical() {
        assert_eq!(
            score("Urgent", "our service is down for everyone right now"),
            Urgency::Critical
        );
    }

    #[test]
    fn legal_complaint_is_critical() {
        assert_eq!(
            score("Notice", "this is a formal legal complaint about your product"),
            Urgency::Critical
        );
    }

    #[test]
    fn frustrated_refund_request_is_high() {
        assert_eq!(
            score("Refund", "this is unacceptable, please refund me now"),
            Urgency::High
        );
    }

    #[test]
    fn routine_informational_update_is_low() {
        assert_eq!(
            score("Heads up", "for your information, no action needed on your end"),
            Urgency::Low
        );
    }

    #[test]
    fn ambiguous_question_defaults_to_normal() {
        assert_eq!(
            score("Question", "how do I update my billing address?"),
            Urgency::Normal
        );
    }

    #[test]
    fn every_tier_gets_a_non_empty_rationale() {
        let cases = [
            ("Urgent", "our service is down"),
            ("Refund", "this is unacceptable, refund me"),
            ("Heads up", "fyi, no action needed"),
            ("Question", "how do I reset my password"),
        ];
        for (subject, body) in cases {
            assert!(!KeywordUrgencyScorer.score(subject, body).rationale.is_empty());
        }
    }
}
