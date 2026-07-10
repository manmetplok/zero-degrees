#[macro_use]
extern crate rocket;

pub mod db;
mod combo;
mod routes;
mod track_objects;
mod xp;

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
            xp::clear,
            xp::progress,
        ],
    )
}
