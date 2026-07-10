use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{LeaderboardEntry, LeaderboardPeriod, LeaderboardResponse, TeamTotals};
use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
struct RankedPlayer {
    player_id: i64,
    device_id: String,
    xp: i64,
    clears: i64,
    avg_response_seconds: Option<f64>,
}

fn parse_period(period: Option<&str>) -> LeaderboardPeriod {
    match period {
        Some("today") => LeaderboardPeriod::Today,
        Some("this_week") => LeaderboardPeriod::ThisWeek,
        _ => LeaderboardPeriod::AllTime,
    }
}

fn cutoff_expr(period: LeaderboardPeriod) -> Option<&'static str> {
    match period {
        LeaderboardPeriod::Today => Some("datetime(date('now'))"),
        LeaderboardPeriod::ThisWeek => Some("datetime('now', '-7 days')"),
        LeaderboardPeriod::AllTime => None,
    }
}

#[get("/leaderboard?<period>")]
pub async fn get(
    pool: &State<SqlitePool>,
    period: Option<String>,
) -> Result<Json<LeaderboardResponse>, Status> {
    let period = parse_period(period.as_deref());
    let cutoff = cutoff_expr(period);

    let clears_filter = match cutoff {
        Some(expr) => format!("AND c.cleared_at >= {expr}"),
        None => String::new(),
    };
    let ranking_sql = format!(
        "SELECT p.id AS player_id, p.device_id AS device_id, \
         COALESCE(SUM(c.xp), 0) AS xp, COALESCE(COUNT(c.id), 0) AS clears, \
         AVG(c.response_time_seconds) AS avg_response_seconds \
         FROM players p \
         LEFT JOIN clears c ON c.player_id = p.id {clears_filter} \
         GROUP BY p.id, p.device_id \
         ORDER BY xp DESC, p.id ASC"
    );
    let ranked = sqlx::query_as::<_, RankedPlayer>(&ranking_sql)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    let open_filter = match cutoff {
        Some(expr) => format!("WHERE created_at >= {expr}"),
        None => String::new(),
    };
    let open_count: i64 = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM track_objects {open_filter}"
    ))
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let team_xp: i64 = ranked.iter().map(|row| row.xp).sum();
    let team_clears: i64 = ranked.iter().map(|row| row.clears).sum();

    let entries = ranked
        .into_iter()
        .enumerate()
        .map(|(index, row)| LeaderboardEntry {
            rank: index as i64 + 1,
            player_id: row.player_id,
            device_id: row.device_id,
            xp: row.xp,
            streak: 0,
            badge_count: 0,
            avg_response_seconds: row.avg_response_seconds,
        })
        .collect();

    Ok(Json(LeaderboardResponse {
        period,
        team: TeamTotals {
            xp: team_xp,
            clears: team_clears,
            incoming_volume: open_count + team_clears,
        },
        entries,
    }))
}
