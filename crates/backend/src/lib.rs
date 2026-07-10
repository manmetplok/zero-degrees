#[macro_use]
extern crate rocket;

pub mod db;
mod race_control;
mod routes;
mod track_objects;

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
            race_control::stats,
        ],
    )
}
