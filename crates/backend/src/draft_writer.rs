pub struct MessageContext<'a> {
    pub subject: &'a str,
    pub body: &'a str,
    pub language: &'a str,
}

pub trait DraftWriter: Send + Sync {
    fn write(&self, message: &MessageContext, steering_note: Option<&str>) -> String;
}

pub struct TemplateDraftWriter;

impl DraftWriter for TemplateDraftWriter {
    fn write(&self, message: &MessageContext, steering_note: Option<&str>) -> String {
        if is_dutch(message.language) {
            dutch_draft(message, steering_note)
        } else {
            english_draft(message, steering_note)
        }
    }
}

fn is_dutch(language: &str) -> bool {
    matches!(language.to_lowercase().as_str(), "nl" | "dutch" | "nederlands")
}

fn snippet(body: &str, max_chars: usize) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}

fn english_draft(message: &MessageContext, steering_note: Option<&str>) -> String {
    let mut draft = format!(
        "Hi,\n\nThanks for reaching out about \"{}\". You wrote: \"{}\". We understand this matters to you and we're on it.",
        message.subject,
        snippet(message.body, 160),
    );
    if let Some(note) = steering_note {
        draft.push_str(&format!(" {note}"));
    }
    draft.push_str(
        "\n\nWe'll follow up with a concrete next step shortly.\n\nWarm regards,\nThe Meridian Team",
    );
    draft
}

fn dutch_draft(message: &MessageContext, steering_note: Option<&str>) -> String {
    let mut draft = format!(
        "Beste,\n\nBedankt voor je bericht over \"{}\". Je schreef: \"{}\". We begrijpen dat dit belangrijk voor je is en gaan ermee aan de slag.",
        message.subject,
        snippet(message.body, 160),
    );
    if let Some(note) = steering_note {
        draft.push_str(&format!(" {note}"));
    }
    draft.push_str(
        "\n\nWe komen snel terug met een concrete volgende stap.\n\nMet vriendelijke groet,\nHet Meridian Team",
    );
    draft
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(subject: &'a str, body: &'a str, language: &'a str) -> MessageContext<'a> {
        MessageContext {
            subject,
            body,
            language,
        }
    }

    #[test]
    fn drafts_in_english_by_default_and_echoes_subject() {
        let writer = TemplateDraftWriter;
        let draft = writer.write(&context("Late delivery", "My package is late.", "en"), None);
        assert!(draft.contains("Late delivery"));
        assert!(draft.contains("Meridian"));
    }

    #[test]
    fn drafts_in_dutch_when_language_is_dutch() {
        let writer = TemplateDraftWriter;
        let draft = writer.write(
            &context("Late levering", "Mijn pakket is laat.", "nl"),
            None,
        );
        assert!(draft.starts_with("Beste"));
        assert!(draft.contains("Meridian"));
    }

    #[test]
    fn recharge_incorporates_steering_note() {
        let writer = TemplateDraftWriter;
        let draft = writer.write(
            &context("Refund request", "I want my money back.", "en"),
            Some("Offer a 10% discount instead of a full refund"),
        );
        assert!(draft.contains("Offer a 10% discount instead of a full refund"));
    }
}
