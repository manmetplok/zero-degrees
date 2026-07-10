use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{Channel, MessageSearchResult, MessageStatus, Sentiment, Urgency};
use sqlx::sqlite::Sqlite;
use sqlx::{QueryBuilder, SqlitePool};

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    body: String,
    received_at: i64,
    status: String,
    category: Option<String>,
    sentiment: Option<String>,
    urgency: Option<String>,
    summary: Option<String>,
}

fn channel_to_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WebForm => "web_form",
        Channel::Review => "review",
        Channel::Ticket => "ticket",
    }
}

fn str_to_channel(value: &str) -> Option<Channel> {
    match value {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
    }
}

fn status_to_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Open => "open",
        MessageStatus::Cleared => "cleared",
        MessageStatus::Skipped => "skipped",
    }
}

fn str_to_status(value: &str) -> Option<MessageStatus> {
    match value {
        "open" => Some(MessageStatus::Open),
        "cleared" => Some(MessageStatus::Cleared),
        "skipped" => Some(MessageStatus::Skipped),
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

fn str_to_sentiment(value: &str) -> Option<Sentiment> {
    match value {
        "positive" => Some(Sentiment::Positive),
        "neutral" => Some(Sentiment::Neutral),
        "negative" => Some(Sentiment::Negative),
        "angry" => Some(Sentiment::Angry),
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

fn str_to_urgency(value: &str) -> Option<Urgency> {
    match value {
        "critical" => Some(Urgency::Critical),
        "high" => Some(Urgency::High),
        "normal" => Some(Urgency::Normal),
        "low" => Some(Urgency::Low),
        _ => None,
    }
}

fn parse_filter<T>(raw: Option<String>, parse: impl Fn(&str) -> Option<T>) -> Result<Option<T>, Status> {
    match raw {
        Some(value) => parse(&value).map(Some).ok_or(Status::UnprocessableEntity),
        None => Ok(None),
    }
}

fn to_search_result(row: MessageRow) -> Result<MessageSearchResult, Status> {
    let channel = str_to_channel(&row.channel).ok_or(Status::InternalServerError)?;
    let status = str_to_status(&row.status).ok_or(Status::InternalServerError)?;
    let sentiment = match row.sentiment {
        Some(value) => Some(str_to_sentiment(&value).ok_or(Status::InternalServerError)?),
        None => None,
    };
    let urgency = match row.urgency {
        Some(value) => Some(str_to_urgency(&value).ok_or(Status::InternalServerError)?),
        None => None,
    };
    Ok(MessageSearchResult {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        body: row.body,
        received_at: row.received_at,
        status,
        category: row.category,
        sentiment,
        urgency,
        summary: row.summary,
    })
}

#[get("/messages/search?<q>&<channel>&<category>&<sentiment>&<urgency>&<status>&<sort>")]
pub async fn search(
    pool: &State<SqlitePool>,
    q: Option<String>,
    channel: Option<String>,
    category: Option<String>,
    sentiment: Option<String>,
    urgency: Option<String>,
    status: Option<String>,
    sort: Option<String>,
) -> Result<Json<Vec<MessageSearchResult>>, Status> {
    let channel = parse_filter(channel, str_to_channel)?;
    let status = parse_filter(status, str_to_status)?;
    let sentiment = parse_filter(sentiment, str_to_sentiment)?;
    let urgency = parse_filter(urgency, str_to_urgency)?;

    let sort = sort.unwrap_or_else(|| if q.is_some() { "relevance".into() } else { "recent".into() });
    if sort != "relevance" && sort != "recent" {
        return Err(Status::UnprocessableEntity);
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, channel, sender, subject, body, received_at, status, category, sentiment, urgency, summary \
         FROM messages WHERE 1 = 1",
    );

    if let Some(channel) = channel {
        builder.push(" AND channel = ").push_bind(channel_to_str(channel));
    }
    if let Some(status) = status {
        builder.push(" AND status = ").push_bind(status_to_str(status));
    }
    if let Some(category) = &category {
        builder.push(" AND category = ").push_bind(category.clone());
    }
    if let Some(sentiment) = sentiment {
        builder.push(" AND sentiment = ").push_bind(sentiment_to_str(sentiment));
    }
    if let Some(urgency) = urgency {
        builder.push(" AND urgency = ").push_bind(urgency_to_str(urgency));
    }
    if let Some(text) = &q {
        let pattern = format!("%{}%", text);
        builder
            .push(" AND (sender LIKE ")
            .push_bind(pattern.clone())
            .push(" OR subject LIKE ")
            .push_bind(pattern.clone())
            .push(" OR body LIKE ")
            .push_bind(pattern)
            .push(")");
    }

    if sort == "relevance" {
        if let Some(text) = &q {
            let pattern = format!("%{}%", text);
            builder
                .push(" ORDER BY (CASE WHEN sender LIKE ")
                .push_bind(pattern.clone())
                .push(" THEN 2 ELSE 0 END + CASE WHEN subject LIKE ")
                .push_bind(pattern.clone())
                .push(" THEN 2 ELSE 0 END + CASE WHEN body LIKE ")
                .push_bind(pattern)
                .push(" THEN 1 ELSE 0 END) DESC, received_at DESC");
        } else {
            builder.push(" ORDER BY received_at DESC");
        }
    } else {
        builder.push(" ORDER BY received_at DESC");
    }

    let rows = builder
        .build_query_as::<MessageRow>()
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    let results = rows
        .into_iter()
        .map(to_search_result)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(results))
}
