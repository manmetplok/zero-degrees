use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{RecordClear, RecordDayEnd, Trophy, TrophyKind, TrophyProgress, TrophyTier};
use sqlx::SqlitePool;

struct TrophyDef {
    kind: TrophyKind,
    thresholds: [i64; 3],
}

const TROPHY_DEFS: [TrophyDef; 5] = [
    TrophyDef {
        kind: TrophyKind::SpeedDemon,
        thresholds: [10, 30, 100],
    },
    TrophyDef {
        kind: TrophyKind::Firefighter,
        thresholds: [5, 15, 50],
    },
    TrophyDef {
        kind: TrophyKind::Peacekeeper,
        thresholds: [5, 15, 50],
    },
    TrophyDef {
        kind: TrophyKind::CleanSweep,
        thresholds: [1, 5, 20],
    },
    TrophyDef {
        kind: TrophyKind::HighJumper,
        thresholds: [10, 30, 100],
    },
];

const SPEED_DEMON_MAX_SECONDS: i64 = 5 * 60;

fn tier_for_count(thresholds: [i64; 3], count: i64) -> Option<TrophyTier> {
    if count >= thresholds[2] {
        Some(TrophyTier::Gold)
    } else if count >= thresholds[1] {
        Some(TrophyTier::Silver)
    } else if count >= thresholds[0] {
        Some(TrophyTier::Bronze)
    } else {
        None
    }
}

fn tier_index(tier: TrophyTier) -> usize {
    match tier {
        TrophyTier::Bronze => 0,
        TrophyTier::Silver => 1,
        TrophyTier::Gold => 2,
    }
}

fn next_tier(tier: Option<TrophyTier>) -> Option<TrophyTier> {
    match tier {
        None => Some(TrophyTier::Bronze),
        Some(TrophyTier::Bronze) => Some(TrophyTier::Silver),
        Some(TrophyTier::Silver) => Some(TrophyTier::Gold),
        Some(TrophyTier::Gold) => None,
    }
}

fn kind_str(kind: TrophyKind) -> &'static str {
    match kind {
        TrophyKind::SpeedDemon => "speed_demon",
        TrophyKind::Firefighter => "firefighter",
        TrophyKind::Peacekeeper => "peacekeeper",
        TrophyKind::CleanSweep => "clean_sweep",
        TrophyKind::HighJumper => "high_jumper",
    }
}

fn kind_from_str(s: &str) -> Option<TrophyKind> {
    match s {
        "speed_demon" => Some(TrophyKind::SpeedDemon),
        "firefighter" => Some(TrophyKind::Firefighter),
        "peacekeeper" => Some(TrophyKind::Peacekeeper),
        "clean_sweep" => Some(TrophyKind::CleanSweep),
        "high_jumper" => Some(TrophyKind::HighJumper),
        _ => None,
    }
}

fn tier_str(tier: TrophyTier) -> &'static str {
    match tier {
        TrophyTier::Bronze => "bronze",
        TrophyTier::Silver => "silver",
        TrophyTier::Gold => "gold",
    }
}

fn tier_from_str(s: &str) -> Option<TrophyTier> {
    match s {
        "bronze" => Some(TrophyTier::Bronze),
        "silver" => Some(TrophyTier::Silver),
        "gold" => Some(TrophyTier::Gold),
        _ => None,
    }
}

async fn player_exists(pool: &SqlitePool, player_id: i64) -> Result<bool, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar("SELECT id FROM players WHERE id = ?")
        .bind(player_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

async fn require_player(pool: &SqlitePool, player_id: i64) -> Result<(), Status> {
    if player_exists(pool, player_id)
        .await
        .map_err(|_| Status::InternalServerError)?
    {
        Ok(())
    } else {
        Err(Status::NotFound)
    }
}

async fn count_for(pool: &SqlitePool, player_id: i64, kind: TrophyKind) -> Result<i64, sqlx::Error> {
    let count: i64 = match kind {
        TrophyKind::SpeedDemon => {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM clears WHERE player_id = ? AND duration_seconds < ?",
            )
            .bind(player_id)
            .bind(SPEED_DEMON_MAX_SECONDS)
            .fetch_one(pool)
            .await?
        }
        TrophyKind::Firefighter => {
            sqlx::query_scalar("SELECT COUNT(*) FROM clears WHERE player_id = ? AND was_burning = 1")
                .bind(player_id)
                .fetch_one(pool)
                .await?
        }
        TrophyKind::Peacekeeper => {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM clears WHERE player_id = ? AND is_angry_aura = 1",
            )
            .bind(player_id)
            .fetch_one(pool)
            .await?
        }
        TrophyKind::HighJumper => {
            sqlx::query_scalar("SELECT COUNT(*) FROM clears WHERE player_id = ? AND is_critical = 1")
                .bind(player_id)
                .fetch_one(pool)
                .await?
        }
        TrophyKind::CleanSweep => {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM day_ends WHERE player_id = ? AND track_empty = 1",
            )
            .bind(player_id)
            .fetch_one(pool)
            .await?
        }
    };
    Ok(count)
}

async fn earned_tier(
    pool: &SqlitePool,
    player_id: i64,
    kind: TrophyKind,
) -> Result<Option<TrophyTier>, sqlx::Error> {
    let row: Option<String> = sqlx::query_scalar(
        "SELECT tier FROM player_trophies WHERE player_id = ? AND kind = ?",
    )
    .bind(player_id)
    .bind(kind_str(kind))
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|s| tier_from_str(&s)))
}

async fn evaluate_and_award(pool: &SqlitePool, player_id: i64) -> Result<Vec<Trophy>, sqlx::Error> {
    let mut awarded = Vec::new();
    for def in TROPHY_DEFS.iter() {
        let count = count_for(pool, player_id, def.kind).await?;
        let reached_tier = match tier_for_count(def.thresholds, count) {
            Some(tier) => tier,
            None => continue,
        };
        let current_tier = earned_tier(pool, player_id, def.kind).await?;
        if current_tier.is_some_and(|t| t >= reached_tier) {
            continue;
        }

        if current_tier.is_none() {
            sqlx::query("INSERT INTO player_trophies (player_id, kind, tier) VALUES (?, ?, ?)")
                .bind(player_id)
                .bind(kind_str(def.kind))
                .bind(tier_str(reached_tier))
                .execute(pool)
                .await?;
        } else {
            sqlx::query(
                "UPDATE player_trophies SET tier = ?, tier_awarded_at = datetime('now') \
                 WHERE player_id = ? AND kind = ?",
            )
            .bind(tier_str(reached_tier))
            .bind(player_id)
            .bind(kind_str(def.kind))
            .execute(pool)
            .await?;
        }

        let (first_awarded_at, tier_awarded_at): (String, String) = sqlx::query_as(
            "SELECT first_awarded_at, tier_awarded_at FROM player_trophies \
             WHERE player_id = ? AND kind = ?",
        )
        .bind(player_id)
        .bind(kind_str(def.kind))
        .fetch_one(pool)
        .await?;

        awarded.push(Trophy {
            kind: def.kind,
            tier: reached_tier,
            first_awarded_at,
            tier_awarded_at,
        });
    }
    Ok(awarded)
}

#[post("/players/<player_id>/clears", data = "<body>")]
pub async fn record_clear(
    pool: &State<SqlitePool>,
    player_id: i64,
    body: Json<RecordClear>,
) -> Result<Json<Vec<Trophy>>, Status> {
    require_player(pool.inner(), player_id).await?;
    sqlx::query(
        "INSERT INTO clears (player_id, duration_seconds, was_burning, is_angry_aura, is_critical) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(body.duration_seconds)
    .bind(body.was_burning)
    .bind(body.is_angry_aura)
    .bind(body.is_critical)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let awarded = evaluate_and_award(pool.inner(), player_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(awarded))
}

#[post("/players/<player_id>/day-end", data = "<body>")]
pub async fn record_day_end(
    pool: &State<SqlitePool>,
    player_id: i64,
    body: Json<RecordDayEnd>,
) -> Result<Json<Vec<Trophy>>, Status> {
    require_player(pool.inner(), player_id).await?;
    sqlx::query("INSERT INTO day_ends (player_id, track_empty) VALUES (?, ?)")
        .bind(player_id)
        .bind(body.track_empty)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    let awarded = evaluate_and_award(pool.inner(), player_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(awarded))
}

#[get("/players/<player_id>/trophies")]
pub async fn list_earned(
    pool: &State<SqlitePool>,
    player_id: i64,
) -> Result<Json<Vec<Trophy>>, Status> {
    require_player(pool.inner(), player_id).await?;
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT kind, tier, first_awarded_at, tier_awarded_at FROM player_trophies \
         WHERE player_id = ? ORDER BY kind",
    )
    .bind(player_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let trophies = rows
        .into_iter()
        .map(|(kind, tier, first_awarded_at, tier_awarded_at)| {
            let kind = kind_from_str(&kind).ok_or(Status::InternalServerError)?;
            let tier = tier_from_str(&tier).ok_or(Status::InternalServerError)?;
            Ok(Trophy {
                kind,
                tier,
                first_awarded_at,
                tier_awarded_at,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    Ok(Json(trophies))
}

#[get("/players/<player_id>/trophies/progress")]
pub async fn list_progress(
    pool: &State<SqlitePool>,
    player_id: i64,
) -> Result<Json<Vec<TrophyProgress>>, Status> {
    require_player(pool.inner(), player_id).await?;
    let mut progress = Vec::with_capacity(TROPHY_DEFS.len());
    for def in TROPHY_DEFS.iter() {
        let count = count_for(pool.inner(), player_id, def.kind)
            .await
            .map_err(|_| Status::InternalServerError)?;
        let tier = earned_tier(pool.inner(), player_id, def.kind)
            .await
            .map_err(|_| Status::InternalServerError)?;
        let upcoming = next_tier(tier);
        let next_threshold = upcoming.map(|t| def.thresholds[tier_index(t)]);
        progress.push(TrophyProgress {
            kind: def.kind,
            tier,
            count,
            next_tier: upcoming,
            next_threshold,
        });
    }
    Ok(Json(progress))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_for_count_is_none_below_bronze_threshold() {
        assert_eq!(tier_for_count([10, 30, 100], 9), None);
    }

    #[test]
    fn tier_for_count_reaches_each_tier_at_its_threshold() {
        assert_eq!(tier_for_count([10, 30, 100], 10), Some(TrophyTier::Bronze));
        assert_eq!(tier_for_count([10, 30, 100], 29), Some(TrophyTier::Bronze));
        assert_eq!(tier_for_count([10, 30, 100], 30), Some(TrophyTier::Silver));
        assert_eq!(tier_for_count([10, 30, 100], 99), Some(TrophyTier::Silver));
        assert_eq!(tier_for_count([10, 30, 100], 100), Some(TrophyTier::Gold));
        assert_eq!(tier_for_count([10, 30, 100], 1000), Some(TrophyTier::Gold));
    }

    #[test]
    fn next_tier_progresses_bronze_silver_gold_then_none() {
        assert_eq!(next_tier(None), Some(TrophyTier::Bronze));
        assert_eq!(next_tier(Some(TrophyTier::Bronze)), Some(TrophyTier::Silver));
        assert_eq!(next_tier(Some(TrophyTier::Silver)), Some(TrophyTier::Gold));
        assert_eq!(next_tier(Some(TrophyTier::Gold)), None);
    }
}
