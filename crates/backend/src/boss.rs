use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{
    BossHit, BossMessage, BossStatus, Channel, ClearBossMessage, CreateBossMessage, MessageStatus,
    Priority, RunnerContribution,
};
use sqlx::sqlite::SqliteConnection;
use sqlx::SqlitePool;

pub struct BossConfig {
    pub burn_threshold_secs: i64,
    pub enrage_threshold: i64,
}

impl BossConfig {
    pub fn from_env() -> Self {
        let burn_threshold_secs = std::env::var("BOSS_BURN_THRESHOLD_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3600);
        let enrage_threshold = std::env::var("BOSS_ENRAGE_THRESHOLD")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3);
        Self {
            burn_threshold_secs,
            enrage_threshold,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct OpenMessage {
    pub weight: i64,
    pub received_at: i64,
}

pub fn total_health(open: &[OpenMessage]) -> i64 {
    open.iter().map(|message| message.weight).sum()
}

pub fn count_burning(open: &[OpenMessage], now: i64, burn_threshold_secs: i64) -> i64 {
    open.iter()
        .filter(|message| now - message.received_at >= burn_threshold_secs)
        .count() as i64
}

pub fn is_enraged(burning_count: i64, enrage_threshold: i64) -> bool {
    burning_count >= enrage_threshold
}

pub fn grown_max_health(previous_max: i64, current_health: i64) -> i64 {
    previous_max.max(current_health)
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: i64,
    channel: String,
    sender: String,
    subject: String,
    priority: String,
    weight: i64,
    status: String,
    received_at: i64,
}

#[derive(sqlx::FromRow)]
struct BattleRow {
    id: i64,
    max_health: i64,
    ended_at: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct ContributionRow {
    runner: String,
    hits: i64,
    damage: i64,
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

fn priority_to_str(priority: Priority) -> &'static str {
    match priority {
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

fn priority_from_str(value: &str) -> Option<Priority> {
    match value {
        "low" => Some(Priority::Low),
        "normal" => Some(Priority::Normal),
        "high" => Some(Priority::High),
        "critical" => Some(Priority::Critical),
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

fn to_boss_message(row: MessageRow) -> Result<BossMessage, Status> {
    let channel = channel_from_str(&row.channel).ok_or(Status::InternalServerError)?;
    let priority = priority_from_str(&row.priority).ok_or(Status::InternalServerError)?;
    let status = status_from_str(&row.status).ok_or(Status::InternalServerError)?;
    Ok(BossMessage {
        id: row.id,
        channel,
        sender: row.sender,
        subject: row.subject,
        priority,
        status,
        received_at: row.received_at,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn open_messages(conn: &mut SqliteConnection) -> Result<Vec<OpenMessage>, sqlx::Error> {
    sqlx::query_as::<_, OpenMessage>(
        "SELECT weight, received_at FROM messages WHERE status = 'open'",
    )
    .fetch_all(conn)
    .await
}

async fn active_battle(conn: &mut SqliteConnection, now: i64) -> Result<BattleRow, sqlx::Error> {
    if let Some(row) = sqlx::query_as::<_, BattleRow>(
        "SELECT id, max_health, ended_at FROM boss_battles WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(&mut *conn)
    .await?
    {
        return Ok(row);
    }
    sqlx::query_as::<_, BattleRow>(
        "INSERT INTO boss_battles (spawned_at, max_health) VALUES (?, 0) \
         RETURNING id, max_health, ended_at",
    )
    .bind(now)
    .fetch_one(conn)
    .await
}

async fn latest_battle(conn: &mut SqliteConnection) -> Result<Option<BattleRow>, sqlx::Error> {
    sqlx::query_as::<_, BattleRow>(
        "SELECT id, max_health, ended_at FROM boss_battles ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(conn)
    .await
}

async fn recent_hits(conn: &mut SqliteConnection, battle_id: i64) -> Result<Vec<BossHit>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct HitRow {
        runner: String,
        message_id: i64,
        subject: String,
        damage: i64,
        cleared_at: i64,
    }
    let rows = sqlx::query_as::<_, HitRow>(
        "SELECT h.runner AS runner, h.message_id AS message_id, m.subject AS subject, \
                h.damage AS damage, h.cleared_at AS cleared_at \
         FROM boss_hits h JOIN messages m ON m.id = h.message_id \
         WHERE h.battle_id = ? ORDER BY h.cleared_at DESC, h.id DESC LIMIT 10",
    )
    .bind(battle_id)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| BossHit {
            runner: row.runner,
            message_id: row.message_id,
            subject: row.subject,
            damage: row.damage,
            cleared_at: row.cleared_at,
        })
        .collect())
}

async fn contributions(conn: &mut SqliteConnection) -> Result<Vec<RunnerContribution>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ContributionRow>(
        "SELECT runner, COUNT(*) AS hits, SUM(damage) AS damage \
         FROM boss_hits GROUP BY runner ORDER BY damage DESC",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RunnerContribution {
            runner: row.runner,
            hits: row.hits,
            damage: row.damage,
        })
        .collect())
}

async fn build_status(
    conn: &mut SqliteConnection,
    config: &BossConfig,
    now: i64,
) -> Result<BossStatus, sqlx::Error> {
    let battle = latest_battle(conn).await?;
    let contributions = contributions(conn).await?;
    let Some(battle) = battle else {
        return Ok(BossStatus {
            battle_id: None,
            health: 0,
            max_health: 0,
            burning_count: 0,
            enraged: false,
            victory: false,
            recent_hits: Vec::new(),
            contributions,
        });
    };
    let open = open_messages(conn).await?;
    let health = total_health(&open);
    let burning_count = count_burning(&open, now, config.burn_threshold_secs);
    let recent_hits = recent_hits(conn, battle.id).await?;
    Ok(BossStatus {
        battle_id: Some(battle.id),
        health,
        max_health: battle.max_health,
        burning_count,
        enraged: is_enraged(burning_count, config.enrage_threshold),
        victory: battle.ended_at.is_some(),
        recent_hits,
        contributions,
    })
}

#[post("/boss/messages", data = "<body>")]
pub async fn create_message(
    pool: &State<SqlitePool>,
    body: Json<CreateBossMessage>,
) -> Result<status::Created<Json<BossMessage>>, Status> {
    let now = now_unix();
    let weight = body.priority.weight();
    let mut tx = pool.begin().await.map_err(|_| Status::InternalServerError)?;

    let row = sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (channel, sender, subject, body, priority, weight, status, received_at) \
         VALUES (?, ?, ?, '', ?, ?, 'open', ?) \
         RETURNING id, channel, sender, subject, priority, weight, status, received_at",
    )
    .bind(channel_to_str(body.channel))
    .bind(&body.sender)
    .bind(&body.subject)
    .bind(priority_to_str(body.priority))
    .bind(weight)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let battle = active_battle(&mut *tx, now)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let open = open_messages(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let new_max = grown_max_health(battle.max_health, total_health(&open));
    if new_max != battle.max_health {
        sqlx::query("UPDATE boss_battles SET max_health = ? WHERE id = ?")
            .bind(new_max)
            .bind(battle.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }

    tx.commit().await.map_err(|_| Status::InternalServerError)?;

    let message = to_boss_message(row)?;
    let location = format!("/boss/messages/{}", message.id);
    Ok(status::Created::new(location).body(Json(message)))
}

#[post("/boss/messages/<id>/clear", data = "<body>")]
pub async fn clear_message(
    pool: &State<SqlitePool>,
    config: &State<BossConfig>,
    id: i64,
    body: Json<ClearBossMessage>,
) -> Result<Json<BossStatus>, Status> {
    let now = now_unix();
    let mut tx = pool.begin().await.map_err(|_| Status::InternalServerError)?;

    let message = sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel, sender, subject, priority, weight, status, received_at \
         FROM messages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    if message.status != "open" {
        return Err(Status::Conflict);
    }

    sqlx::query(
        "UPDATE messages SET status = 'cleared', cleared_by = ?, cleared_at = ? WHERE id = ?",
    )
    .bind(&body.runner)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let battle = active_battle(&mut *tx, now)
        .await
        .map_err(|_| Status::InternalServerError)?;

    sqlx::query(
        "INSERT INTO boss_hits (battle_id, message_id, runner, damage, cleared_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(battle.id)
    .bind(id)
    .bind(&body.runner)
    .bind(message.weight)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let open = open_messages(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    if total_health(&open) == 0 {
        sqlx::query("UPDATE boss_battles SET ended_at = ? WHERE id = ?")
            .bind(now)
            .bind(battle.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }

    let result = build_status(&mut *tx, config.inner(), now)
        .await
        .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;

    Ok(Json(result))
}

#[get("/boss/status")]
pub async fn boss_status(
    pool: &State<SqlitePool>,
    config: &State<BossConfig>,
) -> Result<Json<BossStatus>, Status> {
    let now = now_unix();
    let mut conn = pool.acquire().await.map_err(|_| Status::InternalServerError)?;
    let result = build_status(&mut conn, config.inner(), now)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(weight: i64, received_at: i64) -> OpenMessage {
        OpenMessage {
            weight,
            received_at,
        }
    }

    #[test]
    fn total_health_is_zero_with_no_open_messages() {
        assert_eq!(total_health(&[]), 0);
    }

    #[test]
    fn total_health_sums_weights_of_open_messages() {
        let open = vec![message(1, 0), message(3, 0), message(5, 0)];
        assert_eq!(total_health(&open), 9);
    }

    #[test]
    fn count_burning_includes_messages_exactly_at_threshold() {
        let open = vec![message(1, 0)];
        assert_eq!(count_burning(&open, 3600, 3600), 1);
    }

    #[test]
    fn count_burning_excludes_messages_under_threshold() {
        let open = vec![message(1, 0)];
        assert_eq!(count_burning(&open, 3599, 3600), 0);
    }

    #[test]
    fn is_enraged_triggers_at_threshold_and_above() {
        assert!(!is_enraged(2, 3));
        assert!(is_enraged(3, 3));
        assert!(is_enraged(4, 3));
    }

    #[test]
    fn grown_max_health_rises_with_new_messages() {
        assert_eq!(grown_max_health(10, 15), 15);
    }

    #[test]
    fn grown_max_health_holds_steady_as_messages_clear() {
        assert_eq!(grown_max_health(15, 6), 15);
    }
}
