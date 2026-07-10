#[macro_use]
extern crate rocket;

mod boss;
pub mod db;
mod routes;
mod track_objects;

use boss::BossConfig;
use rocket::{Build, Rocket};
use sqlx::SqlitePool;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    rocket::build()
        .manage(pool)
        .manage(BossConfig::from_env())
        .mount(
            "/",
            routes![
                routes::health,
                track_objects::create,
                track_objects::list,
                track_objects::delete,
                boss::create_message,
                boss::clear_message,
                boss::boss_status,
            ],
        )
}
