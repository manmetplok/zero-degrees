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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// One hit from the binoculars search/filter endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageSearchResult {
    pub id: i64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub status: MessageStatus,
    pub category: Option<String>,
    pub sentiment: Option<Sentiment>,
    pub urgency: Option<Urgency>,
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
}

/// Open messages in track order, ready to render as hurdles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMessages {
    pub messages: Vec<Message>,
    pub remaining_count: i64,
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

/// A message's topic, assigned by AI at ingestion and overridable by a player.
/// Drives the hurdle's visual type on the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Billing,
    Complaint,
    Question,
    Feedback,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Billing,
        Category::Complaint,
        Category::Question,
        Category::Feedback,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Billing => "Billing",
            Category::Complaint => "Complaint",
            Category::Question => "Question",
            Category::Feedback => "Feedback",
        }
    }
}

/// A message as persisted by the backend, with its current effective category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategorizedMessage {
    pub id: i64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub status: MessageStatus,
    pub category: Category,
    pub sentiment: Sentiment,
    /// One-to-two-sentence AI scout report. `None` means the body was already
    /// under the summary threshold, so the client shows it directly.
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetMessageCategory {
    pub category: Category,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrgencyLevel {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClearHurdle {
    pub urgency: UrgencyLevel,
    pub on_time: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClearResult {
    pub xp_awarded: i64,
    pub total_xp: i64,
    pub combo_multiplier: f64,
    pub combo_count: i32,
    pub combo_window_remaining_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlayerProgress {
    pub total_xp: i64,
    pub combo_multiplier: f64,
    pub combo_count: i32,
    pub combo_window_remaining_ms: i64,
}

/// Who a message is currently assigned to, and since when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageAssignment {
    pub runner_device_id: String,
    pub assigned_at: String,
}

/// A message plus its current assignment state, as returned by the backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignedMessage {
    pub id: u64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub status: MessageStatus,
    pub draft: Option<String>,
    pub assignment: Option<MessageAssignment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignRequest {
    pub runner_device_id: String,
}

/// A persisted "you've been handed a message" record a runner can poll for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignmentNotification {
    pub id: i64,
    pub message_id: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardPeriod {
    Today,
    ThisWeek,
    AllTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub player_id: i64,
    pub device_id: String,
    pub xp: i64,
    pub streak: i64,
    pub badge_count: i64,
    pub avg_response_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamTotals {
    pub xp: i64,
    pub clears: i64,
    pub incoming_volume: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardResponse {
    pub period: LeaderboardPeriod,
    pub team: TeamTotals,
    pub entries: Vec<LeaderboardEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplyDraft {
    pub id: i64,
    pub message_id: i64,
    pub content: String,
    pub steering_note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RechargeDraft {
    pub steering_note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
    Angry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    ChillJog,
    NormalShift,
    NightmareMonday,
}

impl Default for Difficulty {
    fn default() -> Self {
        Difficulty::NormalShift
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedMessage {
    pub id: i64,
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub status: MessageStatus,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeedRequest {
    #[serde(default)]
    pub difficulty: Difficulty,
    #[serde(default)]
    pub reset: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedResponse {
    pub created: usize,
    pub cleared: usize,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDraftRequest {
    pub draft: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendReplyRequest {
    pub reply: String,
}

/// Full state of a message as tracked by the backend: the message itself
/// plus the reply lifecycle (draft in progress, sent reply, points, resolution).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDetail {
    pub message: Message,
    pub draft: Option<String>,
    pub reply: Option<String>,
    pub points_awarded: i64,
    pub resolved_at: Option<String>,
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
pub struct HurdleClearResult {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelCount {
    pub channel: Channel,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryCount {
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentimentCount {
    pub sentiment: Sentiment,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HazardZoneSummary {
    pub name: String,
    pub message_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerProgress {
    pub device_id: String,
    pub clears: i64,
}

/// Aggregated race-control overview: hurdle counts and distributions plus per-runner progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceControlStats {
    pub open_count: i64,
    pub cleared_count: i64,
    pub overdue_count: i64,
    pub channel_volume: Vec<ChannelCount>,
    pub category_distribution: Vec<CategoryCount>,
    pub sentiment_breakdown: Vec<SentimentCount>,
    pub hazard_zones: Vec<HazardZoneSummary>,
    pub runner_progress: Vec<RunnerProgress>,
}

/// A recurring theme detected across many open/recent messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HazardZone {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub message_count: usize,
    pub message_ids: Vec<u64>,
}

/// A hazard zone with its member messages and an AI-written root-cause briefing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HazardZoneDetail {
    pub zone: HazardZone,
    pub messages: Vec<Message>,
    pub briefing: String,
}
