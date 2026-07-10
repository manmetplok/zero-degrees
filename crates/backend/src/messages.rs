use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, Message, MessageDetail, MessageStatus, SaveDraftRequest, SendReplyRequest};
use sqlx::SqlitePool;

const POINTS_PER_CLEAR: i64 = 10;

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
    reply: Option<String>,
    points_awarded: i64,
    resolved_at: Option<String>,
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

fn to_message_detail(row: MessageRow) -> Result<MessageDetail, Status> {
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

async fn fetch_row(pool: &SqlitePool, id: i64) -> Result<Option<MessageRow>, Status> {
    sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status, draft, reply, \
         points_awarded, resolved_at FROM messages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[get("/messages/<id>")]
pub async fn get(pool: &State<SqlitePool>, id: i64) -> Result<Json<MessageDetail>, Status> {
    let row = fetch_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
    to_message_detail(row).map(Json)
}

#[put("/messages/<id>/draft", data = "<body>")]
pub async fn save_draft(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<SaveDraftRequest>,
) -> Result<Json<MessageDetail>, Status> {
    let row = fetch_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
    if row.status != "open" {
        return Err(Status::Conflict);
    }
    sqlx::query("UPDATE messages SET draft = ? WHERE id = ?")
        .bind(&body.draft)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let row = fetch_row(pool.inner(), id)
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
    let row = fetch_row(pool.inner(), id).await?.ok_or(Status::NotFound)?;
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
    let row = fetch_row(pool.inner(), id)
        .await?
        .ok_or(Status::InternalServerError)?;
    to_message_detail(row).map(Json)
}
