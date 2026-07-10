#[macro_use]
extern crate rocket;

pub mod db;
mod hazard_zones;
mod messages;
mod routes;
mod theme;
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
            hazard_zones::analyze,
            hazard_zones::list,
            hazard_zones::get_one,
        ],
    )
}
