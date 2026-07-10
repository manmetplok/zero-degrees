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

/// How time-critical a message is. Drives hurdle height (story 004) and the
/// response-time target it races against (story 014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Critical,
    High,
    Normal,
    Low,
}

impl Urgency {
    pub const ALL: [Urgency; 4] = [
        Urgency::Critical,
        Urgency::High,
        Urgency::Normal,
        Urgency::Low,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Urgency::Critical => "Critical",
            Urgency::High => "High",
            Urgency::Normal => "Normal",
            Urgency::Low => "Low",
        }
    }
}

/// The response-time target for one urgency level, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResponseTarget {
    pub urgency: Urgency,
    pub target_seconds: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UpdateResponseTarget {
    pub target_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHurdleMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub urgency: Urgency,
    /// Unix timestamp (seconds); defaults to now when omitted.
    pub received_at: Option<i64>,
}

/// A message tracked by the backend, with waiting time and overdue state
/// computed against its urgency's response-time target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HurdleMessage {
    pub id: i64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub urgency: Urgency,
    pub status: MessageStatus,
    pub received_at: i64,
    pub waiting_seconds: i64,
    pub burning: bool,
    pub cleared_at: Option<i64>,
    pub response_seconds: Option<i64>,
    pub points_awarded: Option<i32>,
    pub speed_bonus_awarded: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClearResult {
    pub message: HurdleMessage,
    pub points_awarded: i32,
    pub speed_bonus_awarded: i32,
    pub burning: bool,
}

/// Aggregated response-time performance for one urgency level, for team
/// stats and race control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseTimeStats {
    pub urgency: Urgency,
    pub target_seconds: i64,
    pub cleared_count: i64,
    pub burning_count: i64,
    pub average_response_seconds: f64,
}
