use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{AssignRequest, AssignedMessage, AssignmentNotification, MessageAssignment};
use sqlx::SqlitePool;

use crate::messages::{channel_from_str, status_from_str};

const SELECT_MESSAGE: &str = "SELECT m.id, m.channel, m.sender, m.subject, m.body, m.received_at, \
     m.status, m.draft, p.device_id AS assignee_device_id, m.assigned_at \
     FROM messages m LEFT JOIN players p ON m.assigned_to = p.id";

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    draft: Option<String>,
    assignee_device_id: Option<String>,
    assigned_at: Option<String>,
}

fn to_assigned_message(row: MessageRow) -> Result<AssignedMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
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

async fn fetch_message(pool: &SqlitePool, id: i64) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(&format!("{SELECT_MESSAGE} WHERE m.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

#[get("/messages/<id>")]
pub async fn get_message(pool: &State<SqlitePool>, id: i64) -> Result<Json<AssignedMessage>, Status> {
    let row = fetch_message(pool.inner(), id)
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    Ok(Json(to_assigned_message(row)?))
}

async fn upsert_player(pool: &SqlitePool, device_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO players (device_id) VALUES (?)")
        .bind(device_id)
        .execute(pool)
        .await?;
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM players WHERE device_id = ?")
        .bind(device_id)
        .fetch_one(pool)
        .await?;
    Ok(id)
}

async fn record_notification(
    pool: &SqlitePool,
    message_id: i64,
    player_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO assignment_notifications (message_id, player_id) VALUES (?, ?)")
        .bind(message_id)
        .bind(player_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[post("/messages/<id>/assign", data = "<body>")]
pub async fn assign(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<AssignRequest>,
) -> Result<Json<AssignedMessage>, Status> {
    let pool = pool.inner();
    let player_id = upsert_player(pool, &body.runner_device_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let result = sqlx::query(
        "UPDATE messages SET assigned_to = ?, assigned_at = datetime('now') WHERE id = ?",
    )
    .bind(player_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    if result.rows_affected() == 0 {
        return Err(Status::NotFound);
    }
    record_notification(pool, id, player_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let row = fetch_message(pool, id)
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    Ok(Json(to_assigned_message(row)?))
}

#[post("/messages/<id>/claim", data = "<body>")]
pub async fn claim(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<AssignRequest>,
) -> Result<Json<AssignedMessage>, Status> {
    let pool = pool.inner();
    let player_id = upsert_player(pool, &body.runner_device_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let result = sqlx::query(
        "UPDATE messages SET assigned_to = ?, assigned_at = datetime('now') \
         WHERE id = ? AND assigned_to IS NULL",
    )
    .bind(player_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    if result.rows_affected() == 0 {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM messages WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|_| Status::InternalServerError)?;
        return Err(if exists.is_some() {
            Status::Conflict
        } else {
            Status::NotFound
        });
    }
    record_notification(pool, id, player_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let row = fetch_message(pool, id)
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    Ok(Json(to_assigned_message(row)?))
}

#[get("/players/<device_id>/lane")]
pub async fn lane(
    pool: &State<SqlitePool>,
    device_id: String,
) -> Result<Json<Vec<AssignedMessage>>, Status> {
    let pool = pool.inner();
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, m.channel, m.sender, m.subject, m.body, m.received_at, m.status, m.draft, \
             p.device_id AS assignee_device_id, m.assigned_at \
         FROM messages m JOIN players p ON m.assigned_to = p.id \
         WHERE p.device_id = ? AND m.status = 'open' ORDER BY m.id",
    )
    .bind(device_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_assigned_message)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}

#[get("/players/<device_id>/notifications?<after>")]
pub async fn notifications(
    pool: &State<SqlitePool>,
    device_id: String,
    after: Option<i64>,
) -> Result<Json<Vec<AssignmentNotification>>, Status> {
    let pool = pool.inner();
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT n.id, n.message_id, n.created_at FROM assignment_notifications n \
         JOIN players p ON n.player_id = p.id \
         WHERE p.device_id = ? AND n.id > ? ORDER BY n.id",
    )
    .bind(device_id)
    .bind(after.unwrap_or(0))
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    let notifications = rows
        .into_iter()
        .map(|(id, message_id, created_at)| AssignmentNotification {
            id,
            message_id: message_id as u64,
            created_at,
        })
        .collect();
    Ok(Json(notifications))
}
