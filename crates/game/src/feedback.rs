//! In-memory store for AI coach feedback (story 020): one-tap thumbs on each
//! AI output, persisted together with that output and the player's final
//! sent version. Mirrors the backend's `POST /feedback` payload and
//! `GET /feedback/aggregate` shape (see `shared::CreateAiFeedback` and
//! `shared::FeatureFeedbackSummary`), so syncing later is a transport swap.
//! Pure state, no rendering; race control reads `aggregate()` for the
//! helpful/unhelpful ratios per AI feature.

use shared::{AiFeature, FeedbackRating};

/// One rating, kept with the AI output it judged and (for editable outputs
/// like draft replies) the player's final version.
#[derive(Debug, Clone)]
pub struct FeedbackEntry {
    pub message_id: u64,
    pub feature: AiFeature,
    pub ai_output: String,
    /// The player's sent version; only set for player-editable outputs.
    pub final_value: Option<String>,
    pub rating: FeedbackRating,
}

/// Aggregate ratings for one AI feature, for the race-control screen.
#[allow(dead_code)] // consumed by race control (story 011, other agent)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureStats {
    pub feature: AiFeature,
    pub helpful: u32,
    pub unhelpful: u32,
}

impl FeatureStats {
    /// Share of helpful ratings; 0.0 when the feature has no ratings yet.
    #[allow(dead_code)] // consumed by race control (story 011, other agent)
    pub fn helpful_ratio(&self) -> f64 {
        let total = self.helpful + self.unhelpful;
        if total == 0 {
            0.0
        } else {
            f64::from(self.helpful) / f64::from(total)
        }
    }
}

#[derive(Default)]
pub struct FeedbackStore {
    entries: Vec<FeedbackEntry>,
}

impl FeedbackStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// One-tap rating. Re-rating the same feature of the same message moves
    /// the player's thumb (and refreshes the judged output) instead of
    /// stacking votes.
    pub fn rate(
        &mut self,
        message_id: u64,
        feature: AiFeature,
        ai_output: &str,
        rating: FeedbackRating,
    ) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.message_id == message_id && e.feature == feature)
        {
            entry.rating = rating;
            entry.ai_output = ai_output.to_string();
        } else {
            self.entries.push(FeedbackEntry {
                message_id,
                feature,
                ai_output: ai_output.to_string(),
                final_value: None,
                rating,
            });
        }
    }

    /// The player's current thumb for one AI output, for highlighting.
    pub fn rating_of(&self, message_id: u64, feature: AiFeature) -> Option<FeedbackRating> {
        self.entries
            .iter()
            .find(|e| e.message_id == message_id && e.feature == feature)
            .map(|e| e.rating)
    }

    /// Record the player's sent version alongside their draft rating
    /// (story 020: rating, original draft, and sent version stay together).
    pub fn set_final_value(&mut self, message_id: u64, final_value: &str) {
        for entry in &mut self.entries {
            if entry.message_id == message_id && entry.feature == AiFeature::DraftReply {
                entry.final_value = Some(final_value.to_string());
            }
        }
    }

    /// Helpful/unhelpful counts per AI feature, in a stable order, including
    /// features with no ratings yet — race control shows all four rows.
    #[allow(dead_code)] // consumed by race control (story 011, other agent)
    pub fn aggregate(&self) -> Vec<FeatureStats> {
        [
            AiFeature::Category,
            AiFeature::Urgency,
            AiFeature::Summary,
            AiFeature::DraftReply,
        ]
        .into_iter()
        .map(|feature| {
            let mut stats = FeatureStats {
                feature,
                helpful: 0,
                unhelpful: 0,
            };
            for e in self.entries.iter().filter(|e| e.feature == feature) {
                match e.rating {
                    FeedbackRating::Helpful => stats.helpful += 1,
                    FeedbackRating::Unhelpful => stats.unhelpful += 1,
                }
            }
            stats
        })
        .collect()
    }

    /// Every stored rating, e.g. for a later bulk sync to the backend.
    #[allow(dead_code)] // sync + race control consume this later
    pub fn entries(&self) -> &[FeedbackEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_is_stored_with_the_ai_output() {
        let mut store = FeedbackStore::new();
        store.rate(1, AiFeature::Summary, "Customer was billed twice.", FeedbackRating::Helpful);
        let entry = &store.entries()[0];
        assert_eq!(entry.ai_output, "Customer was billed twice.");
        assert_eq!(store.rating_of(1, AiFeature::Summary), Some(FeedbackRating::Helpful));
        assert_eq!(store.rating_of(1, AiFeature::Urgency), None);
    }

    #[test]
    fn re_rating_moves_the_thumb_instead_of_stacking_votes() {
        let mut store = FeedbackStore::new();
        store.rate(1, AiFeature::DraftReply, "draft v1", FeedbackRating::Helpful);
        store.rate(1, AiFeature::DraftReply, "draft v2", FeedbackRating::Unhelpful);
        assert_eq!(store.entries().len(), 1);
        assert_eq!(store.rating_of(1, AiFeature::DraftReply), Some(FeedbackRating::Unhelpful));
        assert_eq!(store.entries()[0].ai_output, "draft v2");
    }

    #[test]
    fn sent_version_is_attached_to_the_draft_rating() {
        let mut store = FeedbackStore::new();
        store.rate(1, AiFeature::DraftReply, "ai draft", FeedbackRating::Unhelpful);
        store.rate(1, AiFeature::Summary, "summary", FeedbackRating::Helpful);
        store.set_final_value(1, "my rewritten reply");
        let draft = store
            .entries()
            .iter()
            .find(|e| e.feature == AiFeature::DraftReply)
            .unwrap();
        assert_eq!(draft.final_value.as_deref(), Some("my rewritten reply"));
        // Non-editable outputs keep no final version.
        let summary = store.entries().iter().find(|e| e.feature == AiFeature::Summary).unwrap();
        assert_eq!(summary.final_value, None);
    }

    #[test]
    fn aggregate_reports_all_features_with_ratios() {
        let mut store = FeedbackStore::new();
        store.rate(1, AiFeature::Summary, "s1", FeedbackRating::Helpful);
        store.rate(2, AiFeature::Summary, "s2", FeedbackRating::Helpful);
        store.rate(3, AiFeature::Summary, "s3", FeedbackRating::Unhelpful);
        store.rate(1, AiFeature::Urgency, "High", FeedbackRating::Unhelpful);
        let stats = store.aggregate();
        assert_eq!(stats.len(), 4, "race control expects all four features");
        let summary = stats.iter().find(|s| s.feature == AiFeature::Summary).unwrap();
        assert_eq!((summary.helpful, summary.unhelpful), (2, 1));
        assert!((summary.helpful_ratio() - 2.0 / 3.0).abs() < 1e-9);
        let category = stats.iter().find(|s| s.feature == AiFeature::Category).unwrap();
        assert_eq!(category.helpful_ratio(), 0.0);
    }
}
