use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{
    CategorizedMessage, Category, Channel, CreateMessage, Message, MessageDetail, MessageStatus,
    OpenMessages, SaveDraftRequest, SendReplyRequest, Sentiment, SetMessageCategory,
};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::classifier::{Classifier, KeywordClassifier};
use crate::sentiment::{KeywordSentimentClassifier, SentimentClassifier};
use crate::urgency_scorer::{KeywordUrgencyScorer, UrgencyScorer};
use crate::summarizer::{summary_for, MockSummarizer};

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    ai_category: String,
    manual_category: Option<String>,
    sentiment: String,
    urgency: String,
    point_reward: i64,
    rationale: Option<String>,
    summary: Option<String>,
}

pub(crate) fn sentiment_to_str(sentiment: Sentiment) -> &'static str {
    match sentiment {
        Sentiment::Positive => "positive",
        Sentiment::Neutral => "neutral",
        Sentiment::Negative => "negative",
        Sentiment::Angry => "angry",
    }
}

pub(crate) fn sentiment_from_str(value: &str) -> Option<Sentiment> {
    match value {
        "positive" => Some(Sentiment::Positive),
        "neutral" => Some(Sentiment::Neutral),
        "negative" => Some(Sentiment::Negative),
        "angry" => Some(Sentiment::Angry),
        _ => None,
    }
}

pub(crate) fn channel_to_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WebForm => "web_form",
        Channel::Review => "review",
        Channel::Ticket => "ticket",
    }
}

pub(crate) fn channel_from_str(value: &str) -> Option<Channel> {
    match value {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
    }
}

pub(crate) fn status_from_str(value: &str) -> Option<MessageStatus> {
    match value {
        "open" => Some(MessageStatus::Open),
        "cleared" => Some(MessageStatus::Cleared),
        "skipped" => Some(MessageStatus::Skipped),
        _ => None,
    }
}

fn category_to_str(category: Category) -> &'static str {
    match category {
        Category::Billing => "billing",
        Category::Complaint => "complaint",
        Category::Question => "question",
        Category::Feedback => "feedback",
    }
}

fn category_from_str(value: &str) -> Option<Category> {
    match value {
        "billing" => Some(Category::Billing),
        "complaint" => Some(Category::Complaint),
        "question" => Some(Category::Question),
        "feedback" => Some(Category::Feedback),
        _ => None,
    }
}

fn to_message(row: MessageRow) -> Result<Message, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    Ok(Message {
        id: row.id as u64,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
    })
}

fn to_categorized(row: MessageRow) -> Result<CategorizedMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let ai_category = category_from_str(&row.ai_category).ok_or(Status::InternalServerError)?;
    let category = match &row.manual_category {
        Some(manual) => category_from_str(manual).ok_or(Status::InternalServerError)?,
        None => ai_category,
    };
    let sentiment = sentiment_from_str(&row.sentiment).ok_or(Status::InternalServerError)?;
    let urgency = crate::urgency::from_str(&row.urgency).ok_or(Status::InternalServerError)?;
    Ok(CategorizedMessage {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        category,
        sentiment,
        urgency,
        point_reward: row.point_reward,
        rationale: row.rationale,
        summary: row.summary,
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

const SELECT_COLUMNS: &str =
    "id, channel, sender, subject, body, received_at, status, ai_category, manual_category, sentiment, urgency, point_reward, rationale, summary";

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<CategorizedMessage>>, Status> {
    let category = KeywordClassifier.classify(&body.subject, &body.body);
    let sentiment = KeywordSentimentClassifier.classify(&body.subject, &body.body);
    let score = KeywordUrgencyScorer.score(&body.subject, &body.body);
    let summary = summary_for(&body.body, &MockSummarizer);
    let query = format!(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status, ai_category, sentiment, urgency, point_reward, rationale, summary) \
         VALUES (?, ?, ?, ?, ?, 'open', ?, ?, ?, ?, ?, ?) RETURNING {SELECT_COLUMNS}"
    );
    let row = sqlx::query_as::<_, MessageRow>(&query)
        .bind(channel_to_str(body.channel))
        .bind(&body.sender)
        .bind(&body.subject)
        .bind(&body.body)
        .bind(now_unix())
        .bind(category_to_str(category))
        .bind(sentiment_to_str(sentiment))
        .bind(crate::urgency::to_str(score.urgency))
        .bind(score.urgency.point_reward() as i64)
        .bind(&score.rationale)
        .bind(&summary)
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let message = to_categorized(row)?;
    let location = format!("/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[get("/messages?<sentiment>")]
pub async fn list(
    pool: &State<SqlitePool>,
    sentiment: Option<String>,
) -> Result<Json<Vec<CategorizedMessage>>, Status> {
    let sentiment = match sentiment {
        Some(value) => Some(sentiment_from_str(&value).ok_or(Status::BadRequest)?),
        None => None,
    };
    let query = match sentiment {
        Some(_) => format!("SELECT {SELECT_COLUMNS} FROM messages WHERE sentiment = ? ORDER BY id"),
        None => format!("SELECT {SELECT_COLUMNS} FROM messages ORDER BY id"),
    };
    let mut q = sqlx::query_as::<_, MessageRow>(&query);
    if let Some(sentiment) = sentiment {
        q = q.bind(sentiment_to_str(sentiment));
    }
    let rows = q
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_categorized)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}

#[get("/track/messages")]
pub async fn list_open(pool: &State<SqlitePool>) -> Result<Json<OpenMessages>, Status> {
    let query = format!(
        "SELECT {SELECT_COLUMNS} FROM messages WHERE status = 'open' \
         ORDER BY received_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, MessageRow>(&query)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_message)
        .collect::<Result<Vec<_>, _>>()?;
    let remaining_count = messages.len() as i64;
    Ok(Json(OpenMessages {
        messages,
        remaining_count,
    }))
}

#[patch("/messages/<id>/category", data = "<body>")]
pub async fn set_category(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<SetMessageCategory>,
) -> Result<Json<CategorizedMessage>, Status> {
    let query =
        format!("UPDATE messages SET manual_category = ? WHERE id = ? RETURNING {SELECT_COLUMNS}");
    let row = sqlx::query_as::<_, MessageRow>(&query)
        .bind(category_to_str(body.category))
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    let message = to_categorized(row)?;
    Ok(Json(message))
}

pub struct NewMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
}

pub async fn insert(pool: &SqlitePool, new: NewMessage) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status) \
         VALUES (?, ?, ?, ?, ?, 'open') RETURNING id",
    )
    .bind(channel_to_str(new.channel))
    .bind(new.sender)
    .bind(new.subject)
    .bind(new.body)
    .bind(new.received_at)
    .fetch_one(pool)
    .await
    .expect("insert message failed");
    id
}

const POINTS_PER_CLEAR: i64 = 10;

#[derive(sqlx::FromRow)]
struct DetailRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    draft: Option<String>,
    reply: Option<String>,
    points_awarded: i64,
    resolved_at: Option<String>,
}

fn to_message_detail(row: DetailRow) -> Result<MessageDetail, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    Ok(MessageDetail {
        message: Message {
            id: row.id as u64,
            channel,
            sender: row.sender,
            subject: row.subject,
            body: row.body,
            received_at: row.received_at,
            status,
        },
        draft: row.draft,
        reply: row.reply,
        points_awarded: row.points_awarded,
        resolved_at: row.resolved_at,
    })
}

async fn fetch_detail_row(pool: &SqlitePool, id: i64) -> Result<Option<DetailRow>, Status> {
    sqlx::query_as::<_, DetailRow>(
        "SELECT id, channel, sender, subject, body, received_at, status, draft, reply, \
         points_awarded, resolved_at FROM messages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[get("/messages/<id>/detail")]
pub async fn get_detail(pool: &State<SqlitePool>, id: i64) -> Result<Json<MessageDetail>, Status> {
    let row = fetch_detail_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
    to_message_detail(row).map(Json)
}

#[put("/messages/<id>/draft", data = "<body>")]
pub async fn save_draft(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<SaveDraftRequest>,
) -> Result<Json<MessageDetail>, Status> {
    let row = fetch_detail_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
    if row.status != "open" {
        return Err(Status::Conflict);
    }
    sqlx::query("UPDATE messages SET draft = ? WHERE id = ?")
        .bind(&body.draft)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let row = fetch_detail_row(pool.inner(), id)
        .await?
        .ok_or(Status::InternalServerError)?;
    to_message_detail(row).map(Json)
}

#[post("/messages/<id>/send", data = "<body>")]
pub async fn send(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<SendReplyRequest>,
) -> Result<Json<MessageDetail>, Status> {
    let row = fetch_detail_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
    if row.status != "open" {
        return Err(Status::Conflict);
    }
    sqlx::query(
        "UPDATE messages SET status = 'cleared', reply = ?, points_awarded = ?, \
         resolved_at = datetime('now') WHERE id = ?",
    )
    .bind(&body.reply)
    .bind(POINTS_PER_CLEAR)
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let row = fetch_detail_row(pool.inner(), id)
        .await?
        .ok_or(Status::InternalServerError)?;
    to_message_detail(row).map(Json)
}

pub async fn fetch_open_or_recent(
    pool: &SqlitePool,
    recent_since: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let query = format!(
        "SELECT {SELECT_COLUMNS} FROM messages \
         WHERE status = 'open' OR received_at >= ? ORDER BY id"
    );
    let rows = sqlx::query_as::<_, MessageRow>(&query)
        .bind(recent_since)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().filter_map(|r| to_message(r).ok()).collect())
}

pub async fn fetch_by_ids(pool: &SqlitePool, ids: &[u64]) -> Result<Vec<Message>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = sqlx::QueryBuilder::new(format!(
        "SELECT {SELECT_COLUMNS} FROM messages WHERE id IN ("
    ));
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(*id as i64);
    }
    builder.push(") ORDER BY id");
    let rows = builder.build_query_as::<MessageRow>().fetch_all(pool).await?;
    Ok(rows.into_iter().filter_map(|r| to_message(r).ok()).collect())
}
