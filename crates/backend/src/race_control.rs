use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{
    CategoryCount, Channel, ChannelCount, RaceControlStats, RunnerProgress, Sentiment,
    SentimentCount,
};
use sqlx::SqlitePool;

const OVERDUE_THRESHOLD_SECONDS: i64 = 24 * 60 * 60;

fn channel_from_str(value: &str) -> Option<Channel> {
    match value {
        "email" => Some(Channel::Email),
        "web_form" => Some(Channel::WebForm),
        "review" => Some(Channel::Review),
        "ticket" => Some(Channel::Ticket),
        _ => None,
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

async fn status_count(pool: &SqlitePool, status: &str) -> Result<i64, Status> {
    sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE status = ?")
        .bind(status)
        .fetch_one(pool)
        .await
        .map_err(|_| Status::InternalServerError)
}

async fn overdue_count(pool: &SqlitePool) -> Result<i64, Status> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE status = 'open' \
         AND (strftime('%s', 'now') - strftime('%s', received_at)) > ?",
    )
    .bind(OVERDUE_THRESHOLD_SECONDS)
    .fetch_one(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[derive(sqlx::FromRow)]
struct ChannelRow {
    channel: String,
    count: i64,
}

async fn channel_volume(pool: &SqlitePool) -> Result<Vec<ChannelCount>, Status> {
    let rows = sqlx::query_as::<_, ChannelRow>(
        "SELECT channel, COUNT(*) as count FROM messages GROUP BY channel ORDER BY channel",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    rows.into_iter()
        .map(|row| {
            channel_from_str(&row.channel)
                .map(|channel| ChannelCount {
                    channel,
                    count: row.count,
                })
                .ok_or(Status::InternalServerError)
        })
        .collect()
}

#[derive(sqlx::FromRow)]
struct CategoryRow {
    category: String,
    count: i64,
}

async fn category_distribution(pool: &SqlitePool) -> Result<Vec<CategoryCount>, Status> {
    let rows = sqlx::query_as::<_, CategoryRow>(
        "SELECT category, COUNT(*) as count FROM messages \
         WHERE category IS NOT NULL GROUP BY category ORDER BY category",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(rows
        .into_iter()
        .map(|row| CategoryCount {
            category: row.category,
            count: row.count,
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct SentimentRow {
    sentiment: String,
    count: i64,
}

async fn sentiment_breakdown(pool: &SqlitePool) -> Result<Vec<SentimentCount>, Status> {
    let rows = sqlx::query_as::<_, SentimentRow>(
        "SELECT sentiment, COUNT(*) as count FROM messages \
         WHERE sentiment IS NOT NULL GROUP BY sentiment ORDER BY sentiment",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    rows.into_iter()
        .map(|row| {
            sentiment_from_str(&row.sentiment)
                .map(|sentiment| SentimentCount {
                    sentiment,
                    count: row.count,
                })
                .ok_or(Status::InternalServerError)
        })
        .collect()
}

#[derive(sqlx::FromRow)]
struct RunnerRow {
    device_id: String,
    clears: i64,
}

async fn runner_progress(pool: &SqlitePool) -> Result<Vec<RunnerProgress>, Status> {
    let rows = sqlx::query_as::<_, RunnerRow>(
        "SELECT players.device_id as device_id, COUNT(messages.id) as clears \
         FROM players \
         LEFT JOIN messages ON messages.cleared_by = players.id AND messages.status = 'cleared' \
         GROUP BY players.id ORDER BY players.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(rows
        .into_iter()
        .map(|row| RunnerProgress {
            device_id: row.device_id,
            clears: row.clears,
        })
        .collect())
}

#[get("/race-control/stats")]
pub async fn stats(pool: &State<SqlitePool>) -> Result<Json<RaceControlStats>, Status> {
    let pool = pool.inner();
    Ok(Json(RaceControlStats {
        open_count: status_count(pool, "open").await?,
        cleared_count: status_count(pool, "cleared").await?,
        overdue_count: overdue_count(pool).await?,
        channel_volume: channel_volume(pool).await?,
        category_distribution: category_distribution(pool).await?,
        sentiment_breakdown: sentiment_breakdown(pool).await?,
        hazard_zones: Vec::new(),
        runner_progress: runner_progress(pool).await?,
    }))
}
