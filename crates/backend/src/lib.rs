#[macro_use]
extern crate rocket;

mod daily_run;
pub mod db;
mod routes;
mod streak;
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
            daily_run::status,
            daily_run::report_progress,
        ],
    )
}
