use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{ClearHurdle, ClearResult, PlayerProgress};
use sqlx::SqlitePool;

use crate::combo::{self, ComboState};

#[derive(sqlx::FromRow)]
struct ProgressRow {
    total_xp: i64,
    combo_count: i32,
    combo_expires_at_ms: Option<i64>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before the unix epoch")
        .as_millis() as i64
}

async fn record_clear(
    pool: &SqlitePool,
    device_id: &str,
    request: &ClearHurdle,
    now_ms: i64,
) -> Result<ClearResult, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO player_progress (device_id) VALUES (?) ON CONFLICT(device_id) DO NOTHING",
    )
    .bind(device_id)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query_as::<_, ProgressRow>(
        "SELECT total_xp, combo_count, combo_expires_at_ms FROM player_progress WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_one(&mut *tx)
    .await?;
    let previous = row.combo_expires_at_ms.map(|expires_at_ms| ComboState {
        count: row.combo_count,
        expires_at_ms,
    });
    let outcome = combo::resolve_clear(now_ms, previous, request.urgency, request.on_time);
    let total_xp = row.total_xp + outcome.xp_awarded;
    sqlx::query(
        "UPDATE player_progress SET total_xp = ?, combo_count = ?, combo_expires_at_ms = ? \
         WHERE device_id = ?",
    )
    .bind(total_xp)
    .bind(outcome.combo_count)
    .bind(outcome.combo_expires_at_ms)
    .bind(device_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ClearResult {
        xp_awarded: outcome.xp_awarded,
        total_xp,
        combo_multiplier: outcome.multiplier,
        combo_count: outcome.combo_count,
        combo_window_remaining_ms: outcome.window_remaining_ms,
    })
}

async fn load_progress(
    pool: &SqlitePool,
    device_id: &str,
    now_ms: i64,
) -> Result<PlayerProgress, sqlx::Error> {
    let row = sqlx::query_as::<_, ProgressRow>(
        "SELECT total_xp, combo_count, combo_expires_at_ms FROM player_progress WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(PlayerProgress {
            total_xp: 0,
            combo_multiplier: 1.0,
            combo_count: 0,
            combo_window_remaining_ms: 0,
        });
    };
    let in_window = row
        .combo_expires_at_ms
        .is_some_and(|expires_at_ms| now_ms <= expires_at_ms);
    let (combo_count, combo_multiplier, combo_window_remaining_ms) = if in_window {
        (
            row.combo_count,
            combo::multiplier_for(row.combo_count),
            row.combo_expires_at_ms.unwrap() - now_ms,
        )
    } else {
        (0, 1.0, 0)
    };
    Ok(PlayerProgress {
        total_xp: row.total_xp,
        combo_multiplier,
        combo_count,
        combo_window_remaining_ms,
    })
}

#[post("/players/<device_id>/clears", data = "<body>")]
pub async fn clear(
    pool: &State<SqlitePool>,
    device_id: &str,
    body: Json<ClearHurdle>,
) -> Result<Json<ClearResult>, Status> {
    let result = record_clear(pool.inner(), device_id, &body, now_ms())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

#[get("/players/<device_id>/progress")]
pub async fn progress(
    pool: &State<SqlitePool>,
    device_id: &str,
) -> Result<Json<PlayerProgress>, Status> {
    let result = load_progress(pool.inner(), device_id, now_ms())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}
