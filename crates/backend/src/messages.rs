use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, CreateMessage, Message, MessageStatus, OpenMessages};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
}

fn channel_to_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WebForm => "web_form",
        Channel::Review => "review",
        Channel::Ticket => "ticket",
    }
}

fn channel_from_str(value: &str) -> Option<Channel> {
    match value {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
    }
}

fn status_from_str(value: &str) -> Option<MessageStatus> {
    match value {
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
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<Message>>, Status> {
    let row = sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status) \
         VALUES (?, ?, ?, ?, ?, 'open') \
         RETURNING id, channel, sender, subject, body, received_at, status",
    )
    .bind(channel_to_str(body.channel))
    .bind(&body.sender)
    .bind(&body.subject)
    .bind(&body.body)
    .bind(now_unix())
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let message = to_message(row)?;
    let location = format!("/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[get("/track/messages")]
pub async fn list_open(pool: &State<SqlitePool>) -> Result<Json<OpenMessages>, Status> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status FROM messages \
         WHERE status = 'open' ORDER BY received_at ASC, id ASC",
    )
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
