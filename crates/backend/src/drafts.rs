use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{RechargeDraft, ReplyDraft};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::draft_writer::{DraftWriter, MessageContext};

#[derive(sqlx::FromRow)]
struct MessageRow {
    subject: String,
    body: String,
    language: String,
}

#[derive(sqlx::FromRow)]
struct DraftRow {
    id: i64,
    message_id: i64,
    content: String,
    steering_note: Option<String>,
    created_at: String,
}

impl From<DraftRow> for ReplyDraft {
    fn from(row: DraftRow) -> Self {
        ReplyDraft {
            id: row.id,
            message_id: row.message_id,
            content: row.content,
            steering_note: row.steering_note,
            created_at: row.created_at,
        }
    }
}

async fn fetch_message(
    pool: &SqlitePool,
    message_id: i64,
) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>("SELECT subject, body, language FROM messages WHERE id = ?")
        .bind(message_id)
        .fetch_optional(pool)
        .await
}

async fn insert_draft(
    pool: &SqlitePool,
    message_id: i64,
    content: &str,
    steering_note: Option<&str>,
) -> Result<DraftRow, sqlx::Error> {
    sqlx::query_as::<_, DraftRow>(
        "INSERT INTO reply_drafts (message_id, content, steering_note) VALUES (?, ?, ?) \
         RETURNING id, message_id, content, steering_note, created_at",
    )
    .bind(message_id)
    .bind(content)
    .bind(steering_note)
    .fetch_one(pool)
    .await
}

async fn generate_and_store(
    pool: &SqlitePool,
    writer: &dyn DraftWriter,
    message_id: i64,
    steering_note: Option<&str>,
) -> Result<Option<DraftRow>, Status> {
    let message = fetch_message(pool, message_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let Some(message) = message else {
        return Ok(None);
    };
    let context = MessageContext {
        subject: &message.subject,
        body: &message.body,
        language: &message.language,
    };
    let content = writer.write(&context, steering_note);
    let row = insert_draft(pool, message_id, &content, steering_note)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Some(row))
}

#[post("/messages/<message_id>/draft")]
pub async fn create(
    pool: &State<SqlitePool>,
    writer: &State<Arc<dyn DraftWriter>>,
    message_id: i64,
) -> Result<status::Created<Json<ReplyDraft>>, Status> {
    let row = generate_and_store(pool.inner(), writer.inner().as_ref(), message_id, None)
        .await?
        .ok_or(Status::NotFound)?;
    let location = format!("/messages/{message_id}/draft");
    Ok(status::Created::new(location).body(Json(row.into())))
}

#[get("/messages/<message_id>/draft")]
pub async fn latest(
    pool: &State<SqlitePool>,
    message_id: i64,
) -> Result<Json<ReplyDraft>, Status> {
    let row = sqlx::query_as::<_, DraftRow>(
        "SELECT id, message_id, content, steering_note, created_at FROM reply_drafts \
         WHERE message_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(message_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;
    Ok(Json(row.into()))
}

#[post("/messages/<message_id>/draft/recharge", data = "<body>")]
pub async fn recharge(
    pool: &State<SqlitePool>,
    writer: &State<Arc<dyn DraftWriter>>,
    message_id: i64,
    body: Json<RechargeDraft>,
) -> Result<status::Created<Json<ReplyDraft>>, Status> {
    let row = generate_and_store(
        pool.inner(),
        writer.inner().as_ref(),
        message_id,
        Some(body.steering_note.as_str()),
    )
    .await?
    .ok_or(Status::NotFound)?;
    let location = format!("/messages/{message_id}/draft");
    Ok(status::Created::new(location).body(Json(row.into())))
}
