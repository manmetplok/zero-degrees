use rocket::http::Status;


use shared::{Channel, MessageStatus, SeedMessage, Sentiment, Urgency};
use sqlx::SqlitePool;

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
    sentiment: String,
    created_at: String,
}

pub struct SeedNewMessage {
    pub channel: Channel,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub received_at: i64,
    pub urgency: Urgency,
    pub sentiment: Sentiment,
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

fn urgency_to_str(urgency: Urgency) -> &'static str {
    match urgency {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::High => "high",
        Urgency::Critical => "critical",
    }
}

fn urgency_from_str(value: &str) -> Option<Urgency> {
    match value {
        "low" => Some(Urgency::Low),
        "normal" => Some(Urgency::Normal),
        "high" => Some(Urgency::High),
        "critical" => Some(Urgency::Critical),
        _ => None,
    }
}

fn sentiment_to_str(sentiment: Sentiment) -> &'static str {
    match sentiment {
        Sentiment::Positive => "positive",
        Sentiment::Neutral => "neutral",
        Sentiment::Negative => "negative",
        Sentiment::Angry => "angry",
    }
}

fn sentiment_from_str(value: &str) -> Option<Sentiment> {
    match value {
        "positive" => Some(Sentiment::Positive),
        "neutral" => Some(Sentiment::Neutral),
        "negative" => Some(Sentiment::Negative),
        "angry" => Some(Sentiment::Angry),
        _ => None,
    }
}

fn to_seed_message(row: MessageRow) -> Result<SeedMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    let urgency = urgency_from_str(&row.urgency).ok_or(Status::InternalServerError)?;
    let sentiment = sentiment_from_str(&row.sentiment).ok_or(Status::InternalServerError)?;
    Ok(SeedMessage {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        urgency,
        sentiment,
        created_at: row.created_at,
    })
}

pub async fn clear(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM messages").execute(pool).await?;
    Ok(result.rows_affected())
}

pub async fn insert_batch(
    pool: &SqlitePool,
    messages: Vec<SeedNewMessage>,
) -> Result<Vec<SeedMessage>, Status> {
    let mut created = Vec::with_capacity(messages.len());
    for message in messages {
        let row = sqlx::query_as::<_, MessageRow>(
            "INSERT INTO messages (channel, sender, subject, body, received_at, status, urgency, sentiment, created_at) \
             VALUES (?, ?, ?, ?, ?, 'open', ?, ?, datetime('now')) \
             RETURNING id, channel, sender, subject, body, received_at, status, urgency, sentiment, created_at",
        )
        .bind(channel_to_str(message.channel))
        .bind(message.sender)
        .bind(message.subject)
        .bind(message.body)
        .bind(message.received_at)
        .bind(urgency_to_str(message.urgency))
        .bind(sentiment_to_str(message.sentiment))
        .fetch_one(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;
        created.push(to_seed_message(row)?);
    }
    Ok(created)
}

