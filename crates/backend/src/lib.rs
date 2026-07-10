#[macro_use]
extern crate rocket;

mod assignments;
pub mod db;
pub mod messages;
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
            messages::list,
            messages::get,
            assignments::assign,
            assignments::claim,
            assignments::lane,
            assignments::notifications,
        ],
    )
}
