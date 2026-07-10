use shared::Urgency;

pub fn to_str(urgency: Urgency) -> &'static str {
    match urgency {
        Urgency::Critical => "critical",
        Urgency::High => "high",
        Urgency::Normal => "normal",
        Urgency::Low => "low",
    }
}

pub fn from_str(value: &str) -> Option<Urgency> {
    match value {
        "critical" => Some(Urgency::Critical),
        "high" => Some(Urgency::High),
        "normal" => Some(Urgency::Normal),
        "low" => Some(Urgency::Low),
        _ => None,
    }
}
