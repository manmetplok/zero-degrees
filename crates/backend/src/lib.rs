#[macro_use]
extern crate rocket;

pub mod db;
mod messages;
mod response_targets;
mod routes;
mod scoring;
mod track_objects;
mod urgency;

use rocket::{Build, Rocket};
use sqlx::SqlitePool;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    rocket::build().manage(pool).mount(
        "/",
        routes![
            routes::health,
            track_objects::create,
            track_objects::list,
            track_objects::delete,
            response_targets::list,
            response_targets::update,
            messages::create,
            messages::list,
            messages::clear,
            messages::response_time_stats,
        ],
    )
}
