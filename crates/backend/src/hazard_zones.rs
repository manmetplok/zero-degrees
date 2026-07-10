use std::time::{SystemTime, UNIX_EPOCH};

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{HazardZone, HazardZoneDetail};
use sqlx::SqlitePool;

use crate::messages;
use crate::theme::{BriefingWriter, KeywordBriefingWriter, KeywordClusterDetector, ThemeCluster, ThemeDetector};

const RECENT_WINDOW_SECS: i64 = 14 * 24 * 60 * 60;

#[derive(sqlx::FromRow)]
struct ZoneRow {
    id: i64,
    name: String,
    description: String,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn fetch_zone_message_ids(pool: &SqlitePool, zone_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT message_id FROM hazard_zone_messages WHERE zone_id = ? ORDER BY message_id")
        .bind(zone_id)
        .fetch_all(pool)
        .await
}

async fn to_hazard_zone(pool: &SqlitePool, row: ZoneRow) -> Result<HazardZone, sqlx::Error> {
    let message_ids = fetch_zone_message_ids(pool, row.id).await?;
    Ok(HazardZone {
        id: row.id,
        name: row.name,
        description: row.description,
        message_count: message_ids.len(),
        message_ids: message_ids.into_iter().map(|id| id as u64).collect(),
    })
}

async fn fetch_all_zones(pool: &SqlitePool) -> Result<Vec<HazardZone>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ZoneRow>("SELECT id, name, description FROM hazard_zones ORDER BY id")
        .fetch_all(pool)
        .await?;
    let mut zones = Vec::with_capacity(rows.len());
    for row in rows {
        zones.push(to_hazard_zone(pool, row).await?);
    }
    Ok(zones)
}

async fn replace_zones(pool: &SqlitePool, clusters: Vec<ThemeCluster>) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM hazard_zone_messages")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM hazard_zones").execute(&mut *tx).await?;

    for cluster in clusters {
        let zone_id: i64 = sqlx::query_scalar(
            "INSERT INTO hazard_zones (name, description) VALUES (?, ?) RETURNING id",
        )
        .bind(&cluster.name)
        .bind(&cluster.description)
        .fetch_one(&mut *tx)
        .await?;

        for message_id in &cluster.message_ids {
            sqlx::query("INSERT INTO hazard_zone_messages (zone_id, message_id) VALUES (?, ?)")
                .bind(zone_id)
                .bind(*message_id as i64)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await
}

#[post("/hazard-zones/analyze")]
pub async fn analyze(pool: &State<SqlitePool>) -> Result<Json<Vec<HazardZone>>, Status> {
    let recent_since = now_unix() - RECENT_WINDOW_SECS;
    let messages = messages::fetch_open_or_recent(pool.inner(), recent_since)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let clusters = KeywordClusterDetector::default().detect(&messages);
    replace_zones(pool.inner(), clusters)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let zones = fetch_all_zones(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(zones))
}

#[get("/hazard-zones")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<HazardZone>>, Status> {
    let zones = fetch_all_zones(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(zones))
}

#[get("/hazard-zones/<id>")]
pub async fn get_one(pool: &State<SqlitePool>, id: i64) -> Result<Json<HazardZoneDetail>, Status> {
    let row = sqlx::query_as::<_, ZoneRow>("SELECT id, name, description FROM hazard_zones WHERE id = ?")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    let zone = to_hazard_zone(pool.inner(), row)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let messages = messages::fetch_by_ids(pool.inner(), &zone.message_ids)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let briefing = KeywordBriefingWriter.write(&messages);

    Ok(Json(HazardZoneDetail {
        zone,
        messages,
        briefing,
    }))
}
