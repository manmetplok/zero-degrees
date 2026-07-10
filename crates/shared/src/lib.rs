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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrophyKind {
    SpeedDemon,
    Firefighter,
    Peacekeeper,
    CleanSweep,
    HighJumper,
}

impl TrophyKind {
    pub const ALL: [TrophyKind; 5] = [
        TrophyKind::SpeedDemon,
        TrophyKind::Firefighter,
        TrophyKind::Peacekeeper,
        TrophyKind::CleanSweep,
        TrophyKind::HighJumper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrophyKind::SpeedDemon => "Speed Demon",
            TrophyKind::Firefighter => "Firefighter",
            TrophyKind::Peacekeeper => "Peacekeeper",
            TrophyKind::CleanSweep => "Clean Sweep",
            TrophyKind::HighJumper => "High Jumper",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrophyTier {
    Bronze,
    Silver,
    Gold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trophy {
    pub kind: TrophyKind,
    pub tier: TrophyTier,
    pub first_awarded_at: String,
    pub tier_awarded_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrophyProgress {
    pub kind: TrophyKind,
    pub tier: Option<TrophyTier>,
    pub count: i64,
    pub next_tier: Option<TrophyTier>,
    pub next_threshold: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordClear {
    pub duration_seconds: i64,
    pub was_burning: bool,
    pub is_angry_aura: bool,
    pub is_critical: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordDayEnd {
    pub track_empty: bool,
}
