use crate::streak::StreakState;
use chrono::{NaiveDate, Utc};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{DailyRunStatus, ReportDailyProgress};
use sqlx::SqlitePool;

pub const DAILY_GOAL_XP: u32 = 100;

#[derive(sqlx::FromRow)]
struct StreakRow {
    current_streak: i64,
    best_streak: i64,
    has_shield: bool,
    last_settled_date: Option<String>,
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

fn parse_state(row: Option<StreakRow>) -> StreakState {
    let Some(row) = row else {
        return StreakState::new();
    };
    StreakState {
        current_streak: row.current_streak.max(0) as u32,
        best_streak: row.best_streak.max(0) as u32,
        has_shield: row.has_shield,
        last_settled: row
            .last_settled_date
            .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
    }
}

async fn load_state(pool: &SqlitePool, player_id: i64) -> Result<StreakState, Status> {
    let row = sqlx::query_as::<_, StreakRow>(
        "SELECT current_streak, best_streak, has_shield, last_settled_date \
         FROM player_streaks WHERE player_id = ?",
    )
    .bind(player_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(parse_state(row))
}

async fn save_state(pool: &SqlitePool, player_id: i64, state: &StreakState) -> Result<(), Status> {
    sqlx::query(
        "INSERT INTO player_streaks (player_id, current_streak, best_streak, has_shield, last_settled_date) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(player_id) DO UPDATE SET \
            current_streak = excluded.current_streak, \
            best_streak = excluded.best_streak, \
            has_shield = excluded.has_shield, \
            last_settled_date = excluded.last_settled_date",
    )
    .bind(player_id)
    .bind(state.current_streak as i64)
    .bind(state.best_streak as i64)
    .bind(state.has_shield)
    .bind(state.last_settled.map(|d| d.format("%Y-%m-%d").to_string()))
    .execute(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(())
}

async fn load_today_progress(
    pool: &SqlitePool,
    player_id: i64,
    day: NaiveDate,
) -> Result<(u32, bool), Status> {
    let row: Option<(i64, bool)> = sqlx::query_as(
        "SELECT progress_xp, goal_met FROM daily_runs WHERE player_id = ? AND run_date = ?",
    )
    .bind(player_id)
    .bind(day.format("%Y-%m-%d").to_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(row
        .map(|(progress, goal_met)| (progress.max(0) as u32, goal_met))
        .unwrap_or((0, false)))
}

async fn save_today_progress(
    pool: &SqlitePool,
    player_id: i64,
    day: NaiveDate,
    progress: u32,
    goal_met: bool,
) -> Result<(), Status> {
    sqlx::query(
        "INSERT INTO daily_runs (player_id, run_date, progress_xp, goal_met) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(player_id, run_date) DO UPDATE SET \
            progress_xp = excluded.progress_xp, \
            goal_met = excluded.goal_met",
    )
    .bind(player_id)
    .bind(day.format("%Y-%m-%d").to_string())
    .bind(progress as i64)
    .bind(goal_met)
    .execute(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(())
}

fn to_status(state: &StreakState, today_progress: u32, goal_met_today: bool) -> DailyRunStatus {
    DailyRunStatus {
        current_streak: state.current_streak,
        best_streak: state.best_streak,
        has_shield: state.has_shield,
        daily_goal_xp: DAILY_GOAL_XP,
        today_progress_xp: today_progress,
        goal_met_today,
    }
}

#[get("/players/<player_id>/daily-run")]
pub async fn status(pool: &State<SqlitePool>, player_id: i64) -> Result<Json<DailyRunStatus>, Status> {
    let pool = pool.inner();
    let today = today();
    let mut state = load_state(pool, player_id).await?;
    state.settle_missed_days(today);
    save_state(pool, player_id, &state).await?;
    let (progress, goal_met) = load_today_progress(pool, player_id, today).await?;
    Ok(Json(to_status(&state, progress, goal_met)))
}

#[post("/players/<player_id>/daily-run/progress", data = "<body>")]
pub async fn report_progress(
    pool: &State<SqlitePool>,
    player_id: i64,
    body: Json<ReportDailyProgress>,
) -> Result<Json<DailyRunStatus>, Status> {
    let pool = pool.inner();
    let today = today();
    let mut state = load_state(pool, player_id).await?;
    state.settle_missed_days(today);

    let (previous_progress, already_met) = load_today_progress(pool, player_id, today).await?;
    let progress = previous_progress.saturating_add(body.xp_earned);
    let goal_met = already_met || progress >= DAILY_GOAL_XP;

    if goal_met && !already_met {
        state.record_goal_met(today);
    }

    save_today_progress(pool, player_id, today, progress, goal_met).await?;
    save_state(pool, player_id, &state).await?;

    Ok(Json(to_status(&state, progress, goal_met)))
}
