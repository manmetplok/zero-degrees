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

/// How much a message weighs toward the boss's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Priority {
    pub fn weight(self) -> i64 {
        match self {
            Priority::Low => 1,
            Priority::Normal => 2,
            Priority::High => 3,
            Priority::Critical => 5,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBossMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossMessage {
    pub id: i64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub priority: Priority,
    pub status: MessageStatus,
    pub received_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClearBossMessage {
    pub runner: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossHit {
    pub runner: String,
    pub message_id: i64,
    pub subject: String,
    pub damage: i64,
    pub cleared_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerContribution {
    pub runner: String,
    pub hits: i64,
    pub damage: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossStatus {
    pub battle_id: Option<i64>,
    pub health: i64,
    pub max_health: i64,
    pub burning_count: i64,
    pub enraged: bool,
    pub victory: bool,
    pub recent_hits: Vec<BossHit>,
    pub contributions: Vec<RunnerContribution>,
}
