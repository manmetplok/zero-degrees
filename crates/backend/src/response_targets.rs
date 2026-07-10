use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{ResponseTarget, UpdateResponseTarget, Urgency};
use sqlx::SqlitePool;

use crate::urgency;

#[derive(sqlx::FromRow)]
struct ResponseTargetRow {
    urgency: String,
    target_seconds: i64,
}

pub async fn target_for(pool: &SqlitePool, level: Urgency) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT target_seconds FROM response_targets WHERE urgency = ?")
        .bind(urgency::to_str(level))
        .fetch_one(pool)
        .await
        .expect("response_targets is seeded for every urgency level")
}

#[get("/response-targets")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<ResponseTarget>>, Status> {
    let rows = sqlx::query_as::<_, ResponseTargetRow>(
        "SELECT urgency, target_seconds FROM response_targets ORDER BY target_seconds",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let targets = rows
        .into_iter()
        .filter_map(|row| {
            urgency::from_str(&row.urgency).map(|urgency| ResponseTarget {
                urgency,
                target_seconds: row.target_seconds,
            })
        })
        .collect();
    Ok(Json(targets))
}

#[put("/response-targets/<urgency_param>", data = "<body>")]
pub async fn update(
    pool: &State<SqlitePool>,
    urgency_param: &str,
    body: Json<UpdateResponseTarget>,
) -> Result<Json<ResponseTarget>, Status> {
    let target_urgency = urgency::from_str(urgency_param).ok_or(Status::NotFound)?;
    if body.target_seconds <= 0 {
        return Err(Status::BadRequest);
    }
    let result = sqlx::query("UPDATE response_targets SET target_seconds = ? WHERE urgency = ?")
        .bind(body.target_seconds)
        .bind(urgency::to_str(target_urgency))
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    if result.rows_affected() == 0 {
        return Err(Status::NotFound);
    }
    Ok(Json(ResponseTarget {
        urgency: target_urgency,
        target_seconds: body.target_seconds,
    }))
}
