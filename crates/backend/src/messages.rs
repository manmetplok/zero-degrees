use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{CategorizedMessage, Category, Channel, CreateMessage, MessageStatus, SetMessageCategory};
use sqlx::SqlitePool;

use crate::classifier::{Classifier, KeywordClassifier};

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

fn category_to_str(category: Category) -> &'static str {
    match category {
        Category::Billing => "billing",
        Category::Complaint => "complaint",
        Category::Question => "question",
        Category::Feedback => "feedback",
    }
}

fn category_from_str(s: &str) -> Option<Category> {
    match s {
        "billing" => Some(Category::Billing),
        "complaint" => Some(Category::Complaint),
        "question" => Some(Category::Question),
        "feedback" => Some(Category::Feedback),
        _ => None,
    }
}

fn to_message(row: MessageRow) -> Result<CategorizedMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let ai_category = category_from_str(&row.ai_category).ok_or(Status::InternalServerError)?;
    let category = match row.manual_category {
        Some(manual) => category_from_str(&manual).ok_or(Status::InternalServerError)?,
        None => ai_category,
    };
    Ok(CategorizedMessage {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        category,
    })
}

const SELECT_COLUMNS: &str =
    "id, channel, sender, subject, body, received_at, status, ai_category, manual_category";

#[post("/messages", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateMessage>,
) -> Result<status::Created<Json<CategorizedMessage>>, Status> {
    let category = KeywordClassifier.classify(&body.subject, &body.body);
    let query = format!(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status, ai_category) \
         VALUES (?, ?, ?, ?, ?, 'open', ?) RETURNING {SELECT_COLUMNS}"
    );
    let row = sqlx::query_as::<_, MessageRow>(&query)
        .bind(channel_to_str(body.channel))
        .bind(&body.sender)
        .bind(&body.subject)
        .bind(&body.body)
        .bind(body.received_at)
        .bind(category_to_str(category))
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let message = to_message(row)?;
    let location = format!("/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[get("/messages")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<CategorizedMessage>>, Status> {
    let query = format!("SELECT {SELECT_COLUMNS} FROM messages ORDER BY id");
    let rows = sqlx::query_as::<_, MessageRow>(&query)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let messages = rows
        .into_iter()
        .map(to_message)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(messages))
}

#[patch("/messages/<id>/category", data = "<body>")]
pub async fn set_category(
    pool: &State<SqlitePool>,
    id: i64,
    body: Json<SetMessageCategory>,
) -> Result<Json<CategorizedMessage>, Status> {
    let query = format!("UPDATE messages SET manual_category = ? WHERE id = ? RETURNING {SELECT_COLUMNS}");
    let row = sqlx::query_as::<_, MessageRow>(&query)
        .bind(category_to_str(body.category))
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    let message = to_message(row)?;
    Ok(Json(message))
}
