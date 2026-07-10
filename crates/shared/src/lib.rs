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

/// AI-assigned urgency, from calmest to most dramatic. Drives hurdle height
/// and point reward: low is the base jog, critical is the maximum hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    Normal,
    High,
    Critical,
}

impl Urgency {
    pub fn point_reward(self) -> u32 {
        match self {
            Urgency::Low => 10,
            Urgency::Normal => 20,
            Urgency::High => 50,
            Urgency::Critical => 100,
        }
    }
}

/// Request payload to ingest a new message; the backend scores its urgency.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
}

/// A persisted message plus its AI triage: the urgency level, the point
/// reward it scales to, and the rationale shown on the detail card so the
/// game reading never hides the real triage data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriagedMessage {
    #[serde(flatten)]
    pub message: Message,
    pub urgency: Urgency,
    pub point_reward: u32,
    pub rationale: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_reward_increases_with_urgency_severity() {
        let rewards = [
            Urgency::Low.point_reward(),
            Urgency::Normal.point_reward(),
            Urgency::High.point_reward(),
            Urgency::Critical.point_reward(),
        ];
        assert!(rewards.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
