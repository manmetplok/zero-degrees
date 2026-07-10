//! Binocular filter logic (stories 012 and 005): pure, testable free-text
//! search and combinable filters over messages plus their AI metadata.
//! Rendering and input live in binoculars.rs; server-side search is a future
//! backend endpoint (see api-changerequests/course-search.md).

use std::collections::HashMap;

use shared::{Channel, Message, MessageStatus};

use crate::meta::{self, Category, MessageMeta, Sentiment, Urgency};

/// Cached `meta::enrich` results so filtering and drawing never re-derive
/// summaries mid-frame. Rebuilt whenever the track changes.
pub struct MetaCache {
    map: HashMap<u64, MessageMeta>,
    summaries: HashMap<u64, String>,
}

impl MetaCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            summaries: HashMap::new(),
        }
    }

    pub fn rebuild<'a>(&mut self, messages: impl Iterator<Item = &'a Message>) {
        self.map.clear();
        self.summaries.clear();
        for m in messages {
            self.map.insert(m.id, meta::enrich(m));
            self.summaries.insert(m.id, meta::summarize(m));
        }
    }

    pub fn get(&self, id: u64) -> Option<&MessageMeta> {
        self.map.get(&id)
    }

    /// The cached AI scout report for a message (story 006 output).
    pub fn summary(&self, id: u64) -> Option<&str> {
        self.summaries.get(&id).map(String::as_str)
    }
}

/// Combinable binocular filters; empty dimensions match everything.
#[derive(Clone, Default)]
pub struct Filter {
    pub query: String,
    pub channels: Vec<Channel>,
    pub categories: Vec<Category>,
    pub sentiments: Vec<Sentiment>,
    pub urgencies: Vec<Urgency>,
    pub statuses: Vec<MessageStatus>,
}

fn allows<T: PartialEq>(set: &[T], value: &T) -> bool {
    set.is_empty() || set.contains(value)
}

/// Toggle `value` in a filter dimension: absent adds it, present removes it.
pub fn toggle<T: PartialEq>(set: &mut Vec<T>, value: T) {
    if let Some(i) = set.iter().position(|v| *v == value) {
        set.remove(i);
    } else {
        set.push(value);
    }
}

impl Filter {
    /// True when the filter narrows the view at all (drives track dimming).
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
            || !self.channels.is_empty()
            || !self.categories.is_empty()
            || !self.sentiments.is_empty()
            || !self.urgencies.is_empty()
            || !self.statuses.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Filter::default();
    }

    /// All filter dimensions AND together (story 012, combined filters).
    pub fn matches(&self, message: &Message, meta: &MessageMeta) -> bool {
        self.chips_match(message, meta) && self.query_score(message).is_some()
    }

    fn chips_match(&self, message: &Message, meta: &MessageMeta) -> bool {
        allows(&self.channels, &message.channel)
            && allows(&self.categories, &meta.category)
            && allows(&self.sentiments, &meta.sentiment)
            && allows(&self.urgencies, &meta.urgency)
            && allows(&self.statuses, &message.status)
    }

    /// Relevance of `message` for the free-text query. Every whitespace-split
    /// term must appear in the sender, subject, or body; sender hits rank
    /// above subject hits above body hits. `None` when a term is missing.
    pub fn query_score(&self, message: &Message) -> Option<i32> {
        let sender = message.sender.to_lowercase();
        let subject = message.subject.to_lowercase();
        let body = message.body.to_lowercase();
        let mut score = 0;
        for term in self.query.split_whitespace() {
            let term = term.to_lowercase();
            let mut s = 0;
            if sender.contains(&term) {
                s += 30;
            }
            if subject.contains(&term) {
                s += 20;
            }
            if body.contains(&term) {
                s += 10;
            }
            if s == 0 {
                return None;
            }
            score += s;
        }
        Some(score)
    }
}

/// Ranked scan of the course: ids of matching messages, most relevant first,
/// most recent as the tie-break (story 012, text search scenario).
pub fn search<'a>(
    messages: impl Iterator<Item = &'a Message>,
    cache: &MetaCache,
    filter: &Filter,
) -> Vec<u64> {
    let mut hits: Vec<(i32, i64, u64)> = Vec::new();
    for m in messages {
        let Some(meta) = cache.get(m.id) else { continue };
        if !filter.chips_match(m, meta) {
            continue;
        }
        let Some(score) = filter.query_score(m) else { continue };
        hits.push((score, m.received_at, m.id));
    }
    // Score desc, then recency desc, then id desc for a total order.
    hits.sort_by(|a, b| b.cmp(a));
    hits.into_iter().map(|(_, _, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: u64, channel: Channel, sender: &str, subject: &str, body: &str) -> Message {
        Message {
            id,
            channel,
            sender: sender.into(),
            subject: subject.into(),
            body: body.into(),
            received_at: 1_780_000_000 + id as i64 * 60,
            status: MessageStatus::Open,
        }
    }

    fn fixture() -> Vec<Message> {
        vec![
            msg(
                1,
                Channel::Email,
                "s.devries@example.com",
                "Invoice #4821 charged twice",
                "I was billed twice. Please refund one of the charges.",
            ),
            msg(
                2,
                Channel::WebForm,
                "Contact form",
                "Can't reset my password",
                "The reset link says token expired.",
            ),
            msg(
                3,
                Channel::Email,
                "m.jansen@example.com",
                "Refund for duplicate charge",
                "There is a duplicate charge on my statement, please refund it.",
            ),
            msg(
                4,
                Channel::Review,
                "TrustSpot review",
                "★★★★★ Great service",
                "Support fixed my refund within a day, thanks!",
            ),
        ]
    }

    fn cache_for(messages: &[Message]) -> MetaCache {
        let mut cache = MetaCache::new();
        cache.rebuild(messages.iter());
        cache
    }

    #[test]
    fn empty_filter_matches_everything_and_is_inactive() {
        let messages = fixture();
        let cache = cache_for(&messages);
        let filter = Filter::default();
        assert!(!filter.is_active());
        assert_eq!(search(messages.iter(), &cache, &filter).len(), messages.len());
    }

    #[test]
    fn combined_filters_and_together() {
        let messages = fixture();
        let cache = cache_for(&messages);
        let mut filter = Filter::default();
        // Channel email AND category billing (story 012 scenario).
        filter.channels.push(Channel::Email);
        filter.categories.push(Category::Billing);
        let ids = search(messages.iter(), &cache, &filter);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1) && ids.contains(&3));
        // Adding a text query narrows further.
        filter.query = "duplicate".into();
        assert_eq!(search(messages.iter(), &cache, &filter), vec![3]);
    }

    #[test]
    fn status_filter_separates_open_from_cleared() {
        let mut messages = fixture();
        messages[0].status = MessageStatus::Cleared;
        let cache = cache_for(&messages);
        let mut filter = Filter::default();
        filter.statuses.push(MessageStatus::Open);
        let ids = search(messages.iter(), &cache, &filter);
        assert!(!ids.contains(&1) && ids.len() == 3);
        filter.statuses = vec![MessageStatus::Cleared];
        assert_eq!(search(messages.iter(), &cache, &filter), vec![1]);
    }

    #[test]
    fn search_matches_sender_and_ranks_it_above_body_hits() {
        let messages = fixture();
        let cache = cache_for(&messages);
        let mut filter = Filter::default();
        filter.query = "refund".into();
        let ids = search(messages.iter(), &cache, &filter);
        // Subject hits (3) before body-only hits; recency breaks the tie (4 > 1).
        assert_eq!(ids, vec![3, 4, 1]);
        // Sender search finds the customer by name.
        filter.query = "jansen".into();
        assert_eq!(search(messages.iter(), &cache, &filter), vec![3]);
        // All terms must match: an off-topic extra term drops the message.
        filter.query = "jansen password".into();
        assert!(search(messages.iter(), &cache, &filter).is_empty());
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut set: Vec<Channel> = Vec::new();
        toggle(&mut set, Channel::Email);
        assert_eq!(set, vec![Channel::Email]);
        toggle(&mut set, Channel::Email);
        assert!(set.is_empty());
    }
}
