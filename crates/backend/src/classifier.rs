use shared::Category;

pub trait Classifier: Send + Sync {
    fn classify(&self, subject: &str, body: &str) -> Category;
}

pub struct KeywordClassifier;

const BILLING_KEYWORDS: &[&str] = &[
    "invoice", "bill", "billed", "charge", "charged", "refund", "payment", "subscription",
    "price", "overcharged",
];

const COMPLAINT_KEYWORDS: &[&str] = &[
    "angry", "furious", "worst", "terrible", "unacceptable", "complain", "complaint",
    "disappointed", "broken", "awful", "asap", "urgent",
];

const FEEDBACK_KEYWORDS: &[&str] = &[
    "feedback", "suggestion", "suggest", "love", "great", "awesome", "idea", "recommend",
];

impl Classifier for KeywordClassifier {
    fn classify(&self, subject: &str, body: &str) -> Category {
        let text = format!("{subject} {body}").to_lowercase();
        let matches_any = |keywords: &[&str]| keywords.iter().any(|word| text.contains(word));

        if matches_any(BILLING_KEYWORDS) {
            Category::Billing
        } else if matches_any(COMPLAINT_KEYWORDS) {
            Category::Complaint
        } else if matches_any(FEEDBACK_KEYWORDS) {
            Category::Feedback
        } else {
            Category::Question
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_billing_keywords_as_billing() {
        let category = KeywordClassifier.classify(
            "Invoice #4821 charged twice",
            "I was billed twice for my March invoice, please refund one charge.",
        );
        assert_eq!(category, Category::Billing);
    }

    #[test]
    fn classifies_complaint_keywords_as_complaint() {
        let category = KeywordClassifier.classify(
            "This is unacceptable",
            "Your service is terrible and I am furious about the delay.",
        );
        assert_eq!(category, Category::Complaint);
    }

    #[test]
    fn classifies_feedback_keywords_as_feedback() {
        let category = KeywordClassifier.classify(
            "Great support experience",
            "Just wanted to say I love the product, here's an idea for improvement.",
        );
        assert_eq!(category, Category::Feedback);
    }

    #[test]
    fn falls_back_to_question_when_no_keywords_match() {
        let category = KeywordClassifier.classify(
            "Question about opening hours",
            "What time do you open on Saturdays? I'd like to visit the store.",
        );
        assert_eq!(category, Category::Question);
    }
}
