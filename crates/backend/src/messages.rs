use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, CreateMessage, MessageRecord, MessageStatus, Sentiment};
use sqlx::SqlitePool;

use crate::sentiment::SentimentClassifier;

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    sentiment: String,
    created_at: String,
}

fn channel_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WebForm => "web_form",
        Channel::Review => "review",
        Channel::Ticket => "ticket",
    }
}

fn channel_from_str(s: &str) -> Option<Channel> {
    match s {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
    }
}

fn status_from_str(s: &str) -> Option<MessageStatus> {
    match s {
        "open" => Some(MessageStatus::Open),
        "cleared" => Some(MessageStatus::Cleared),
        "skipped" => Some(MessageStatus::Skipped),
        _ => None,
    }
}

fn sentiment_str(sentiment: Sentiment) -> &'static str {
    match sentiment {
        Sentiment::Positive => "positive",
        Sentiment::Neutral => "neutral",
        Sentiment::Negative => "negative",
        Sentiment::Angry => "angry",
    }
}

fn sentiment_from_str(s: &str) -> Option<Sentiment> {
    match s {
        "positive" => Some(Sentiment::Positive),
        "neutral" => Some(Sentiment::Neutral),
        "negative" => Some(Sentiment::Negative),
        "angry" => Some(Sentiment::Angry),
        _ => None,
    }
}

fn to_message_record(row: MessageRow) -> Result<MessageRecord, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let sentiment = sentiment_from_str(&row.sentiment).ok_or(Status::InternalServerError)?;
    Ok(MessageRecord {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        sentiment,
        created_at: row.created_at,
    })
}

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    classifier: &State<Box<dyn SentimentClassifier>>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<MessageRecord>>, Status> {
    let sentiment = classifier.inner().classify(&body.subject, &body.body);
    let row = sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status, sentiment) \
         VALUES (?, ?, ?, ?, ?, 'open', ?) \
         RETURNING id, channel, sender, subject, body, received_at, status, sentiment, created_at",
    )
    .bind(channel_str(body.channel))
    .bind(&body.sender)
    .bind(&body.subject)
    .bind(&body.body)
    .bind(body.received_at)
    .bind(sentiment_str(sentiment))
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let record = to_message_record(row)?;
    let location = format!("/messages/{}", record.id);
    Ok(status::Created::new(location).body(Json(record)))
}

#[get("/messages?<sentiment>")]
pub async fn list(
    pool: &State<SqlitePool>,
    sentiment: Option<String>,
) -> Result<Json<Vec<MessageRecord>>, Status> {
    let rows = match sentiment {
        Some(raw) => {
            let sentiment = sentiment_from_str(&raw).ok_or(Status::BadRequest)?;
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel, sender, subject, body, received_at, status, sentiment, created_at \
                 FROM messages WHERE sentiment = ? ORDER BY id",
            )
            .bind(sentiment_str(sentiment))
            .fetch_all(pool.inner())
            .await
        }
        None => {
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel, sender, subject, body, received_at, status, sentiment, created_at \
                 FROM messages ORDER BY id",
            )
            .fetch_all(pool.inner())
            .await
        }
    }
    .map_err(|_| Status::InternalServerError)?;
    let records = rows
        .into_iter()
        .map(to_message_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(records))
}
