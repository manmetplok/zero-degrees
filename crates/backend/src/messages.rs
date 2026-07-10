use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, CreateMessage, Message, MessageStatus, TriagedMessage, Urgency};
use sqlx::SqlitePool;

use crate::urgency::{KeywordUrgencyScorer, UrgencyScorer};

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    urgency: String,
    point_reward: i64,
    rationale: String,
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

fn urgency_to_str(urgency: Urgency) -> &'static str {
    match urgency {
        Urgency::Critical => "critical",
        Urgency::High => "high",
        Urgency::Normal => "normal",
        Urgency::Low => "low",
    }
}

fn urgency_from_str(s: &str) -> Option<Urgency> {
    match s {
        "critical" => Some(Urgency::Critical),
        "high" => Some(Urgency::High),
        "normal" => Some(Urgency::Normal),
        "low" => Some(Urgency::Low),
        _ => None,
    }
}

fn to_triaged(row: MessageRow) -> Result<TriagedMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let urgency = urgency_from_str(&row.urgency).ok_or(Status::InternalServerError)?;
    Ok(TriagedMessage {
        message: Message {
            id: row.id as u64,
            channel,
            sender: row.sender,
            subject: row.subject,
            body: row.body,
            received_at: row.received_at,
            status,
        },
        urgency,
        point_reward: row.point_reward as u32,
        rationale: row.rationale,
    })
}

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<TriagedMessage>>, Status> {
    let score = KeywordUrgencyScorer.score(&body.subject, &body.body);
    let point_reward = score.urgency.point_reward();

    let row = sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status, urgency, point_reward, rationale) \
         VALUES (?, ?, ?, ?, ?, 'open', ?, ?, ?) \
         RETURNING id, channel, sender, subject, body, received_at, status, urgency, point_reward, rationale",
    )
    .bind(channel_to_str(body.channel))
    .bind(&body.sender)
    .bind(&body.subject)
    .bind(&body.body)
    .bind(body.received_at)
    .bind(urgency_to_str(score.urgency))
    .bind(point_reward as i64)
    .bind(&score.rationale)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let triaged = to_triaged(row)?;
    let location = format!("/messages/{}", triaged.message.id);
    Ok(status::Created::new(location).body(Json(triaged)))
}

#[get("/messages")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<TriagedMessage>>, Status> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status, urgency, point_reward, rationale \
         FROM messages ORDER BY id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_triaged)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}

#[get("/messages/<id>")]
pub async fn get(pool: &State<SqlitePool>, id: i64) -> Result<Json<TriagedMessage>, Status> {
    let row = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status, urgency, point_reward, rationale \
         FROM messages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;
    let triaged = to_triaged(row)?;
    Ok(Json(triaged))
}
