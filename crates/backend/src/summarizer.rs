pub const SUMMARY_THRESHOLD_CHARS: usize = 240;
const SUMMARY_MAX_CHARS: usize = 240;
const SUMMARY_SENTENCE_COUNT: usize = 2;

pub trait Summarizer: Send + Sync {
    fn summarize(&self, body: &str) -> String;
}

pub struct MockSummarizer;

impl Summarizer for MockSummarizer {
    fn summarize(&self, body: &str) -> String {
        let summary = first_sentences(body, SUMMARY_SENTENCE_COUNT);
        if summary.chars().count() > SUMMARY_MAX_CHARS {
            let truncated: String = summary.chars().take(SUMMARY_MAX_CHARS).collect();
            format!("{}...", truncated.trim_end())
        } else {
            summary
        }
    }
}

fn first_sentences(text: &str, count: usize) -> String {
    let trimmed = text.trim();
    let mut sentences = Vec::new();
    let mut start = 0;
    for (i, c) in trimmed.char_indices() {
        if matches!(c, '.' | '!' | '?') {
            let end = i + c.len_utf8();
            sentences.push(trimmed[start..end].trim().to_string());
            start = end;
            if sentences.len() == count {
                break;
            }
        }
    }
    if sentences.len() < count && start < trimmed.len() {
        sentences.push(trimmed[start..].trim().to_string());
    }
    sentences.join(" ")
}

pub fn summary_for(body: &str, summarizer: &dyn Summarizer) -> Option<String> {
    if body.chars().count() <= SUMMARY_THRESHOLD_CHARS {
        None
    } else {
        Some(summarizer.summarize(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_for_returns_none_at_or_under_threshold() {
        let body = "a".repeat(SUMMARY_THRESHOLD_CHARS);
        assert_eq!(summary_for(&body, &MockSummarizer), None);
    }

    #[test]
    fn summary_for_calls_summarizer_over_threshold() {
        let body = "a".repeat(SUMMARY_THRESHOLD_CHARS + 1);
        assert!(summary_for(&body, &MockSummarizer).is_some());
    }

    #[test]
    fn mock_summarizer_keeps_first_two_sentences() {
        let body = "First sentence. Second sentence. Third sentence that should be dropped.";
        assert_eq!(
            MockSummarizer.summarize(body),
            "First sentence. Second sentence."
        );
    }

    #[test]
    fn mock_summarizer_truncates_long_sentence_with_ellipsis() {
        let body = format!("{}.", "word ".repeat(100));
        let summary = MockSummarizer.summarize(&body);
        assert!(summary.ends_with("..."));
        assert!(summary.chars().count() <= SUMMARY_MAX_CHARS + 3);
        assert!(summary.chars().count() < body.chars().count());
    }

    #[test]
    fn mock_summarizer_is_deterministic() {
        let body = "Server is down. Customers cannot check out. Please investigate urgently.";
        assert_eq!(MockSummarizer.summarize(body), MockSummarizer.summarize(body));
    }
}
