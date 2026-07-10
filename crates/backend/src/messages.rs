use shared::{Channel, Message, MessageStatus};
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

fn to_message(row: MessageRow) -> Option<Message> {
    Some(Message {
        id: row.id as u64,
        channel: channel_from_str(&row.channel)?,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status: status_from_str(&row.status)?,
    })
}

pub async fn fetch_open_or_recent(
    pool: &SqlitePool,
    recent_since: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, body, received_at, status FROM messages \
         WHERE status = 'open' OR received_at >= ? ORDER BY id",
    )
    .bind(recent_since)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(to_message).collect())
}

pub async fn fetch_by_ids(pool: &SqlitePool, ids: &[u64]) -> Result<Vec<Message>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = sqlx::QueryBuilder::new(
        "SELECT id, channel, sender, subject, body, received_at, status FROM messages WHERE id IN (",
    );
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(*id as i64);
    }
    builder.push(") ORDER BY id");
    let rows = builder.build_query_as::<MessageRow>().fetch_all(pool).await?;
    Ok(rows.into_iter().filter_map(to_message).collect())
}
