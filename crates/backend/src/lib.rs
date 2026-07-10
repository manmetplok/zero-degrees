#[macro_use]
extern crate rocket;

mod classifier;
pub mod db;
mod feedback;
mod messages;
mod routes;
mod track_objects;
mod trophies;

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
            messages::create,
            messages::list,
            messages::list_open,
            messages::set_category,
            feedback::create,
            feedback::aggregate,
            trophies::record_clear,
            trophies::record_day_end,
            trophies::list_earned,
            trophies::list_progress,
        ],
    )
}
