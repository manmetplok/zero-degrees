use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{AssignedMessage, Channel, MessageAssignment, MessageStatus};
use sqlx::SqlitePool;

const SELECT_MESSAGE: &str = "SELECT m.id, m.channel, m.sender, m.subject, m.body, m.received_at, \
     m.status, m.draft, p.device_id AS assignee_device_id, m.assigned_at \
     FROM messages m LEFT JOIN players p ON m.assigned_to = p.id";

#[derive(sqlx::FromRow)]
pub(crate) struct MessageRow {
    pub id: i64,
    pub channel: String,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub status: String,
    pub draft: Option<String>,
    pub assignee_device_id: Option<String>,
    pub assigned_at: Option<String>,
}

fn channel_to_db(channel: Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WebForm => "web_form",
        Channel::Review => "review",
        Channel::Ticket => "ticket",
    }
}

fn channel_from_db(value: &str) -> Option<Channel> {
    match value {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
    }
}

fn status_from_db(value: &str) -> Option<MessageStatus> {
    match value {
        "open" => Some(MessageStatus::Open),
        "cleared" => Some(MessageStatus::Cleared),
        "skipped" => Some(MessageStatus::Skipped),
        _ => None,
    }
}

pub(crate) fn to_assigned_message(row: MessageRow) -> Result<AssignedMessage, Status> {
    let channel = channel_from_db(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_db(&row.status).ok_or(Status::InternalServerError)?;
    let assignment = match (row.assignee_device_id, row.assigned_at) {
        (Some(runner_device_id), Some(assigned_at)) => Some(MessageAssignment {
            runner_device_id,
            assigned_at,
        }),
        _ => None,
    };
    Ok(AssignedMessage {
        id: row.id as u64,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        draft: row.draft,
        assignment,
    })
}

pub(crate) async fn fetch_message(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(&format!("{SELECT_MESSAGE} WHERE m.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

#[get("/messages")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<AssignedMessage>>, Status> {
    let rows = sqlx::query_as::<_, MessageRow>(&format!("{SELECT_MESSAGE} ORDER BY m.id"))
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_assigned_message)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}

#[get("/messages/<id>")]
pub async fn get(pool: &State<SqlitePool>, id: i64) -> Result<Json<AssignedMessage>, Status> {
    let row = fetch_message(pool.inner(), id)
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    Ok(Json(to_assigned_message(row)?))
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
    .bind(channel_to_db(new.channel))
    .bind(new.sender)
    .bind(new.subject)
    .bind(new.body)
    .bind(new.received_at)
    .fetch_one(pool)
    .await
    .expect("insert message failed");
    id
}
