use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, CreateMessage, Message, MessageStatus};
use sqlx::SqlitePool;

use crate::summarizer::{summary_for, Summarizer};

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    summary: Option<String>,
}

fn channel_to_str(channel: Channel) -> &'static str {
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
        summary: row.summary,
    })
}

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    summarizer: &State<Box<dyn Summarizer>>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<Message>>, Status> {
    let summary = summary_for(&body.body, summarizer.inner().as_ref());
    let row = sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status, summary) \
         VALUES (?, ?, ?, ?, ?, 'open', ?) \
         RETURNING id, channel, sender, subject, body, received_at, status, summary",
    )
    .bind(channel_to_str(body.channel))
    .bind(&body.sender)
    .bind(&body.subject)
    .bind(&body.body)
    .bind(body.received_at)
    .bind(&summary)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let message = to_message(row)?;
    let location = format!("/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[get("/messages")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<Message>>, Status> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status, summary \
         FROM messages ORDER BY id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_message)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}
