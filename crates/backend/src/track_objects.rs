use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{CreateTrackObject, ObjectLink, TrackObject};
use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
struct TrackObjectRow {
    id: i64,
    position: f64,
    link_type: String,
    link_ref: String,
    created_at: String,
}

fn link_parts(link: &ObjectLink) -> (&'static str, &str) {
    match link {
        ObjectLink::Ticket { key } => ("ticket", key),
        ObjectLink::Email { message_id } => ("email", message_id),
        ObjectLink::Review { review_id } => ("review", review_id),
        ObjectLink::Generic { url } => ("generic", url),
    }
}

fn link_from_parts(link_type: &str, link_ref: String) -> Option<ObjectLink> {
    match link_type {
        "ticket" => Some(ObjectLink::Ticket { key: link_ref }),
        "email" => Some(ObjectLink::Email {
            message_id: link_ref,
        }),
        "review" => Some(ObjectLink::Review {
            review_id: link_ref,
        }),
        "generic" => Some(ObjectLink::Generic { url: link_ref }),
        _ => None,
    }
}

fn to_track_object(row: TrackObjectRow) -> Result<TrackObject, Status> {
    let link =
        link_from_parts(&row.link_type, row.link_ref).ok_or(Status::InternalServerError)?;
    Ok(TrackObject {
        id: row.id,
        position: row.position,
        link,
        created_at: row.created_at,
    })
}

#[post("/track/objects", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateTrackObject>,
) -> Result<status::Created<Json<TrackObject>>, Status> {
    let (link_type, link_ref) = link_parts(&body.link);
    let row = sqlx::query_as::<_, TrackObjectRow>(
        "INSERT INTO track_objects (position, link_type, link_ref) VALUES (?, ?, ?) \
         RETURNING id, position, link_type, link_ref, created_at",
    )
    .bind(body.position)
    .bind(link_type)
    .bind(link_ref)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let object = to_track_object(row)?;
    let location = format!("/track/objects/{}", object.id);
    Ok(status::Created::new(location).body(Json(object)))
}

#[get("/track/objects")]
pub async fn list(pool: &State<SqlitePool>) -> Result<Json<Vec<TrackObject>>, Status> {
    let rows = sqlx::query_as::<_, TrackObjectRow>(
        "SELECT id, position, link_type, link_ref, created_at FROM track_objects ORDER BY id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let objects = rows
        .into_iter()
        .map(to_track_object)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(objects))
}

#[delete("/track/objects/<id>")]
pub async fn delete(pool: &State<SqlitePool>, id: i64) -> Result<Status, Status> {
    let result = sqlx::query("DELETE FROM track_objects WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    if result.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        Ok(Status::NoContent)
    }
}
