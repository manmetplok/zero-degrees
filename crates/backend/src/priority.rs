use shared::{Sentiment, Urgency};

const URGENCY_CRITICAL: f64 = 100.0;
const URGENCY_HIGH: f64 = 70.0;
const URGENCY_NORMAL: f64 = 40.0;
const URGENCY_LOW: f64 = 15.0;

const SENTIMENT_ANGRY: f64 = 12.0;
const SENTIMENT_NEGATIVE: f64 = 6.0;
const SENTIMENT_NEUTRAL: f64 = 0.0;
const SENTIMENT_POSITIVE: f64 = -4.0;

const AGE_WEIGHT_PER_HOUR: f64 = 1.0;
const AGE_CAP_HOURS: f64 = 60.0;

fn urgency_weight(urgency: Urgency) -> f64 {
    match urgency {
        Urgency::Critical => URGENCY_CRITICAL,
        Urgency::High => URGENCY_HIGH,
        Urgency::Normal => URGENCY_NORMAL,
        Urgency::Low => URGENCY_LOW,
    }
}

fn sentiment_weight(sentiment: Sentiment) -> f64 {
    match sentiment {
        Sentiment::Angry => SENTIMENT_ANGRY,
        Sentiment::Negative => SENTIMENT_NEGATIVE,
        Sentiment::Neutral => SENTIMENT_NEUTRAL,
        Sentiment::Positive => SENTIMENT_POSITIVE,
    }
}

fn age_weight(age_seconds: i64) -> f64 {
    let age_hours = age_seconds.max(0) as f64 / 3600.0;
    age_hours.min(AGE_CAP_HOURS) * AGE_WEIGHT_PER_HOUR
}

pub fn score(urgency: Urgency, sentiment: Sentiment, age_seconds: i64) -> f64 {
    urgency_weight(urgency) + sentiment_weight(sentiment) + age_weight(age_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_outranks_aged_low_regardless_of_sentiment() {
        let fresh_critical = score(Urgency::Critical, Sentiment::Positive, 0);
        let aged_angry_low = score(Urgency::Low, Sentiment::Angry, 60 * 3600);
        assert!(fresh_critical > aged_angry_low);
    }

    #[test]
    fn priority_rises_as_a_message_waits() {
        let fresh = score(Urgency::Normal, Sentiment::Neutral, 0);
        let waited_a_day = score(Urgency::Normal, Sentiment::Neutral, 24 * 3600);
        assert!(waited_a_day > fresh);
    }

    #[test]
    fn age_bonus_caps_after_max_hours() {
        let at_cap = score(Urgency::Normal, Sentiment::Neutral, 60 * 3600);
        let past_cap = score(Urgency::Normal, Sentiment::Neutral, 200 * 3600);
        assert_eq!(at_cap, past_cap);
    }

    #[test]
    fn angry_sentiment_outranks_neutral_at_equal_urgency_and_age() {
        let angry = score(Urgency::High, Sentiment::Angry, 0);
        let neutral = score(Urgency::High, Sentiment::Neutral, 0);
        assert!(angry > neutral);
    }

    #[test]
    fn positive_sentiment_ranks_below_neutral_at_equal_urgency_and_age() {
        let positive = score(Urgency::High, Sentiment::Positive, 0);
        let neutral = score(Urgency::High, Sentiment::Neutral, 0);
        assert!(positive < neutral);
    }

    #[test]
    fn higher_urgency_outranks_lower_at_equal_sentiment_and_age() {
        assert!(
            score(Urgency::Critical, Sentiment::Neutral, 0)
                > score(Urgency::High, Sentiment::Neutral, 0)
        );
        assert!(
            score(Urgency::High, Sentiment::Neutral, 0)
                > score(Urgency::Normal, Sentiment::Neutral, 0)
        );
        assert!(
            score(Urgency::Normal, Sentiment::Neutral, 0)
                > score(Urgency::Low, Sentiment::Neutral, 0)
        );
    }
}
