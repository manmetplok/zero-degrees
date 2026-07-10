use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{
    Channel, ClearResult, CreateHurdleMessage, HurdleMessage, MessageStatus, ResponseTimeStats,
    Urgency,
};
use sqlx::SqlitePool;

use crate::response_targets::target_for;
use crate::scoring;
use crate::urgency;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before the unix epoch")
        .as_secs() as i64
}

fn channel_str(channel: Channel) -> &'static str {
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

fn status_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Open => "open",
        MessageStatus::Cleared => "cleared",
        MessageStatus::Skipped => "skipped",
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

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    urgency: String,
    status: String,
    received_at: i64,
    cleared_at: Option<i64>,
    response_seconds: Option<i64>,
    points_awarded: Option<i32>,
    speed_bonus_awarded: Option<i32>,
}

const MESSAGE_COLUMNS: &str = "id, channel, sender, subject, body, urgency, status, \
    received_at, cleared_at, response_seconds, points_awarded, speed_bonus_awarded";

fn to_hurdle_message(row: MessageRow, target_seconds: i64, at: i64) -> Result<HurdleMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let urgency = urgency::from_str(&row.urgency).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let waiting_seconds = match status {
        MessageStatus::Open => scoring::elapsed_seconds(at, row.received_at),
        _ => row.response_seconds.unwrap_or(0),
    };
    let burning = match status {
        MessageStatus::Open => scoring::is_burning(waiting_seconds, target_seconds),
        MessageStatus::Cleared => row
            .response_seconds
            .map(|seconds| scoring::is_burning(seconds, target_seconds))
            .unwrap_or(false),
        MessageStatus::Skipped => false,
    };
    Ok(HurdleMessage {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        urgency,
        status,
        received_at: row.received_at,
        waiting_seconds,
        burning,
        cleared_at: row.cleared_at,
        response_seconds: row.response_seconds,
        points_awarded: row.points_awarded,
        speed_bonus_awarded: row.speed_bonus_awarded,
    })
}

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateHurdleMessage>,
) -> Result<status::Created<Json<HurdleMessage>>, Status> {
    let received_at = body.received_at.unwrap_or_else(now);
    let query = format!(
        "INSERT INTO messages (channel, sender, subject, body, urgency, status, received_at) \
         VALUES (?, ?, ?, ?, ?, 'open', ?) RETURNING {MESSAGE_COLUMNS}"
    );
    let row = sqlx::query_as::<_, MessageRow>(&query)
        .bind(channel_str(body.channel))
        .bind(&body.sender)
        .bind(&body.subject)
        .bind(&body.body)
        .bind(urgency::to_str(body.urgency))
        .bind(received_at)
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let target_seconds = target_for(pool.inner(), body.urgency).await;
    let message = to_hurdle_message(row, target_seconds, now())?;
    let location = format!("/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[get("/messages")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<HurdleMessage>>, Status> {
    let query = format!("SELECT {MESSAGE_COLUMNS} FROM messages ORDER BY id");
    let rows = sqlx::query_as::<_, MessageRow>(&query)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let at = now();
    let mut targets = Vec::with_capacity(Urgency::ALL.len());
    for level in Urgency::ALL {
        targets.push((level, target_for(pool.inner(), level).await));
    }
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        let level = urgency::from_str(&row.urgency).ok_or(Status::InternalServerError)?;
        let target_seconds = targets
            .iter()
            .find(|(urgency, _)| *urgency == level)
            .map(|(_, seconds)| *seconds)
            .ok_or(Status::InternalServerError)?;
        messages.push(to_hurdle_message(row, target_seconds, at)?);
    }
    Ok(Json(messages))
}

#[post("/messages/<id>/clear")]
pub async fn clear(pool: &State<SqlitePool>, id: i64) -> Result<Json<ClearResult>, Status> {
    let select = format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = ?");
    let row = sqlx::query_as::<_, MessageRow>(&select)
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    let current_status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    if current_status != MessageStatus::Open {
        return Err(Status::Conflict);
    }

    let urgency = urgency::from_str(&row.urgency).ok_or(Status::InternalServerError)?;
    let target_seconds = target_for(pool.inner(), urgency).await;
    let cleared_at = now();
    let response_seconds = scoring::elapsed_seconds(cleared_at, row.received_at);
    let score = scoring::score_clear(urgency, response_seconds, target_seconds);

    let update = format!(
        "UPDATE messages SET status = ?, cleared_at = ?, response_seconds = ?, \
         points_awarded = ?, speed_bonus_awarded = ? WHERE id = ? RETURNING {MESSAGE_COLUMNS}"
    );
    let updated = sqlx::query_as::<_, MessageRow>(&update)
        .bind(status_str(MessageStatus::Cleared))
        .bind(cleared_at)
        .bind(response_seconds)
        .bind(score.points_awarded)
        .bind(score.speed_bonus_awarded)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    let message = to_hurdle_message(updated, target_seconds, cleared_at)?;
    Ok(Json(ClearResult {
        message,
        points_awarded: score.points_awarded,
        speed_bonus_awarded: score.speed_bonus_awarded,
        burning: score.burning,
    }))
}

#[get("/response-times")]
pub async fn response_time_stats(
    pool: &State<SqlitePool>,
) -> Result<Json<Vec<ResponseTimeStats>>, Status> {
    let mut stats = Vec::with_capacity(Urgency::ALL.len());
    for level in Urgency::ALL {
        let target_seconds = target_for(pool.inner(), level).await;
        let (cleared_count, burning_count, average_response_seconds): (i64, i64, Option<f64>) =
            sqlx::query_as(
                "SELECT COUNT(*), \
                        COALESCE(SUM(CASE WHEN response_seconds > ? THEN 1 ELSE 0 END), 0), \
                        AVG(response_seconds) \
                 FROM messages WHERE urgency = ? AND status = 'cleared'",
            )
            .bind(target_seconds)
            .bind(urgency::to_str(level))
            .fetch_one(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;
        stats.push(ResponseTimeStats {
            urgency: level,
            target_seconds,
            cleared_count,
            burning_count,
            average_response_seconds: average_response_seconds.unwrap_or(0.0),
        });
    }
    Ok(Json(stats))
}
