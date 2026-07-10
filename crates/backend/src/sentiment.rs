use shared::Sentiment;

pub trait SentimentClassifier: Send + Sync {
    fn classify(&self, subject: &str, body: &str) -> Sentiment;
}

pub struct KeywordSentimentClassifier;

const ANGRY_KEYWORDS: &[&str] = &[
    "furious",
    "outraged",
    "unacceptable",
    "ridiculous",
    "disgusted",
    "scam",
    "worst",
];

const NEGATIVE_KEYWORDS: &[&str] = &[
    "disappointed",
    "broken",
    "problem",
    "issue",
    "refund",
    "cancel",
    "delay",
    "delayed",
    "fail",
    "failed",
    "error",
    "complain",
    "complaint",
    "locked out",
];

const POSITIVE_KEYWORDS: &[&str] = &[
    "thanks",
    "thank you",
    "great",
    "love",
    "awesome",
    "excellent",
    "perfect",
    "appreciate",
    "happy",
    "amazing",
];

impl SentimentClassifier for KeywordSentimentClassifier {
    fn classify(&self, subject: &str, body: &str) -> Sentiment {
        let text = format!("{subject} {body}").to_lowercase();
        if ANGRY_KEYWORDS.iter().any(|k| text.contains(k)) {
            Sentiment::Angry
        } else if NEGATIVE_KEYWORDS.iter().any(|k| text.contains(k)) {
            Sentiment::Negative
        } else if POSITIVE_KEYWORDS.iter().any(|k| text.contains(k)) {
            Sentiment::Positive
        } else {
            Sentiment::Neutral
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_angry_over_negative_when_both_present() {
        let classifier = KeywordSentimentClassifier;
        let sentiment = classifier.classify(
            "This is unacceptable",
            "I am furious about this broken order, worst service ever.",
        );
        assert_eq!(sentiment, Sentiment::Angry);
    }

    #[test]
    fn classifies_negative_message() {
        let classifier = KeywordSentimentClassifier;
        let sentiment = classifier.classify(
            "Refund request",
            "The item arrived broken and I am disappointed.",
        );
        assert_eq!(sentiment, Sentiment::Negative);
    }

    #[test]
    fn classifies_positive_message() {
        let classifier = KeywordSentimentClassifier;
        let sentiment = classifier.classify("Thank you!", "Your support team is awesome.");
        assert_eq!(sentiment, Sentiment::Positive);
    }

    #[test]
    fn classifies_neutral_message_by_default() {
        let classifier = KeywordSentimentClassifier;
        let sentiment =
            classifier.classify("Pricing question", "Do you offer a family plan with four seats?");
        assert_eq!(sentiment, Sentiment::Neutral);
    }
}
