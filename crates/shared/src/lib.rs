//! API types shared between the game client and the backend.

use serde::{Deserialize, Serialize};

/// Where a message came from. Each channel gets its own hurdle look on the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Email,
    WebForm,
    Review,
    Ticket,
}

impl Channel {
    pub const ALL: [Channel; 4] = [
        Channel::Email,
        Channel::WebForm,
        Channel::Review,
        Channel::Ticket,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Channel::Email => "Email",
            Channel::WebForm => "Web form",
            Channel::Review => "Review",
            Channel::Ticket => "Ticket",
        }
    }
}

/// An incoming customer message. One open message = one hurdle on the track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: u64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    /// Unix timestamp (seconds) when the message was received.
    pub received_at: i64,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Open,
    Cleared,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectLink {
    Ticket { key: String },
    Email { message_id: String },
    Review { review_id: String },
    Generic { url: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackObject {
    pub id: i64,
    pub position: f64,
    pub link: ObjectLink,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTrackObject {
    pub position: f64,
    pub link: ObjectLink,
}

/// Which AI-generated output a feedback rating applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiFeature {
    Category,
    Urgency,
    Summary,
    DraftReply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackRating {
    Helpful,
    Unhelpful,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAiFeedback {
    pub feature: AiFeature,
    /// The hurdle/message the AI output was generated for, when known.
    pub message_id: Option<i64>,
    pub ai_output: String,
    /// The player's final version, when the feature allows editing (e.g. a rewritten draft reply
    /// or a corrected category). Absent for feedback on features with no player-editable output.
    pub final_value: Option<String>,
    pub rating: FeedbackRating,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiFeedback {
    pub id: i64,
    pub feature: AiFeature,
    pub message_id: Option<i64>,
    pub ai_output: String,
    pub final_value: Option<String>,
    pub rating: FeedbackRating,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureFeedbackSummary {
    pub feature: AiFeature,
    pub helpful: i64,
    pub unhelpful: i64,
    pub helpful_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackTrendPoint {
    pub date: String,
    pub feature: AiFeature,
    pub helpful: i64,
    pub unhelpful: i64,
    pub helpful_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackAggregate {
    pub by_feature: Vec<FeatureFeedbackSummary>,
    pub trend: Vec<FeedbackTrendPoint>,
}
