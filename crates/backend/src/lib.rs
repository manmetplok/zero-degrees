#[macro_use]
extern crate rocket;

pub mod db;
mod routes;

use rocket::{Build, Rocket};
use sqlx::SqlitePool;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    rocket::build()
        .manage(pool)
        .mount("/", routes![routes::health])
}
