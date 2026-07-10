use std::collections::{HashMap, HashSet};

use shared::Message;

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "from", "have", "has", "was", "were", "are",
    "you", "your", "our", "not", "but", "all", "any", "can", "will", "when", "what", "there",
    "their", "they", "them", "then", "than", "into", "about", "again", "still", "just", "also",
    "been", "being", "please", "thanks", "thank", "hello", "regards",
];

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_string())
        .filter(|w| w.len() > 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

fn message_tokens(message: &Message) -> HashSet<String> {
    let mut tokens: HashSet<String> = tokenize(&message.subject).into_iter().collect();
    tokens.extend(tokenize(&message.body));
    tokens
}

fn keyword_frequencies(messages: &[Message]) -> HashMap<String, usize> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    for message in messages {
        for token in message_tokens(message) {
            *freq.entry(token).or_insert(0) += 1;
        }
    }
    freq
}

fn top_keyword(freq: &HashMap<String, usize>) -> Option<String> {
    let mut candidates: Vec<(usize, &String)> = freq.iter().map(|(k, v)| (*v, k)).collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    candidates.first().map(|(_, token)| (*token).clone())
}

fn best_keyword(tokens: &HashSet<String>, freq: &HashMap<String, usize>) -> Option<String> {
    let mut candidates: Vec<(usize, &String)> = tokens
        .iter()
        .filter_map(|token| {
            let count = *freq.get(token).unwrap_or(&0);
            if count >= 2 {
                Some((count, token))
            } else {
                None
            }
        })
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    candidates.first().map(|(_, token)| (*token).clone())
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub struct ThemeCluster {
    pub name: String,
    pub description: String,
    pub message_ids: Vec<u64>,
}

pub trait ThemeDetector: Send + Sync {
    fn detect(&self, messages: &[Message]) -> Vec<ThemeCluster>;
}

pub struct KeywordClusterDetector {
    pub min_cluster_size: usize,
}

impl Default for KeywordClusterDetector {
    fn default() -> Self {
        Self { min_cluster_size: 3 }
    }
}

impl ThemeDetector for KeywordClusterDetector {
    fn detect(&self, messages: &[Message]) -> Vec<ThemeCluster> {
        let freq = keyword_frequencies(messages);
        let mut groups: HashMap<String, Vec<u64>> = HashMap::new();
        for message in messages {
            let tokens = message_tokens(message);
            if let Some(keyword) = best_keyword(&tokens, &freq) {
                groups.entry(keyword).or_default().push(message.id);
            }
        }

        let mut clusters: Vec<ThemeCluster> = groups
            .into_iter()
            .filter(|(_, ids)| ids.len() >= self.min_cluster_size)
            .map(|(keyword, mut ids)| {
                ids.sort_unstable();
                ThemeCluster {
                    name: format!("{} issues", title_case(&keyword)),
                    description: format!("{} messages reference '{}'.", ids.len(), keyword),
                    message_ids: ids,
                }
            })
            .collect();

        clusters.sort_by(|a, b| {
            b.message_ids
                .len()
                .cmp(&a.message_ids.len())
                .then_with(|| a.name.cmp(&b.name))
        });
        clusters
    }
}

pub trait BriefingWriter: Send + Sync {
    fn write(&self, messages: &[Message]) -> String;
}

pub struct KeywordBriefingWriter;

impl BriefingWriter for KeywordBriefingWriter {
    fn write(&self, messages: &[Message]) -> String {
        if messages.is_empty() {
            return "No messages in this zone.".to_string();
        }
        let freq = keyword_frequencies(messages);
        let keyword = top_keyword(&freq).unwrap_or_else(|| "this issue".to_string());

        let mut channels: Vec<&'static str> = messages.iter().map(|m| m.channel.label()).collect();
        channels.sort_unstable();
        channels.dedup();

        format!(
            "{} messages across {} channel(s) ({}) point to a shared root cause around '{}'. Fix that and the whole group clears.",
            messages.len(),
            channels.len(),
            channels.join(", "),
            keyword,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Channel, MessageStatus};

    fn message(id: u64, subject: &str, body: &str) -> Message {
        Message {
            id,
            channel: Channel::Email,
            sender: "customer@example.com".into(),
            subject: subject.into(),
            body: body.into(),
            received_at: 0,
            status: MessageStatus::Open,
        }
    }

    #[test]
    fn forms_cluster_when_keyword_shared_by_enough_messages() {
        let messages = vec![
            message(1, "Checkout broken", "The checkout page throws an error."),
            message(2, "Can't checkout", "Checkout fails at payment step."),
            message(3, "Checkout issue", "Getting stuck during checkout again."),
            message(4, "Unrelated", "My profile picture will not upload."),
        ];
        let clusters = KeywordClusterDetector::default().detect(&messages);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].name, "Checkout issues");
        assert_eq!(clusters[0].message_ids, vec![1, 2, 3]);
    }

    #[test]
    fn no_cluster_forms_below_min_cluster_size() {
        let messages = vec![
            message(1, "Checkout broken", "The checkout page throws an error."),
            message(2, "Can't checkout", "Checkout fails at payment step."),
        ];
        let clusters = KeywordClusterDetector::default().detect(&messages);
        assert!(clusters.is_empty());
    }

    #[test]
    fn separates_two_distinct_themes() {
        let messages = vec![
            message(1, "Checkout broken", "The checkout page throws an error."),
            message(2, "Can't checkout", "Checkout fails at payment step."),
            message(3, "Checkout issue", "Getting stuck during checkout again."),
            message(4, "Shipping delay", "My shipping has not moved in a week."),
            message(5, "Shipping stuck", "Shipping status has not updated."),
            message(6, "Shipping late", "Still waiting on shipping to arrive."),
        ];
        let clusters = KeywordClusterDetector::default().detect(&messages);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].name, "Checkout issues");
        assert_eq!(clusters[1].name, "Shipping issues");
    }

    #[test]
    fn stopwords_never_form_a_cluster() {
        let messages = vec![
            message(1, "Hello", "Thanks for your help with this."),
            message(2, "Hello", "Thanks again about that."),
            message(3, "Hello", "Thanks for being there."),
        ];
        let clusters = KeywordClusterDetector::default().detect(&messages);
        assert!(clusters.is_empty());
    }

    #[test]
    fn briefing_mentions_count_channel_and_keyword() {
        let messages = vec![
            message(1, "Checkout broken", "The checkout page throws an error."),
            message(2, "Can't checkout", "Checkout fails at payment step."),
            message(3, "Checkout issue", "Getting stuck during checkout again."),
        ];
        let briefing = KeywordBriefingWriter.write(&messages);
        assert!(briefing.contains('3'));
        assert!(briefing.contains("checkout"));
        assert!(briefing.contains(Channel::Email.label()));
    }
}
