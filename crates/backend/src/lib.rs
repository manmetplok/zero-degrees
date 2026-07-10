#[macro_use]
extern crate rocket;

pub mod db;
mod messages;
mod routes;
pub mod summarizer;
mod track_objects;

use rocket::{Build, Rocket};
use sqlx::SqlitePool;
use summarizer::{MockSummarizer, Summarizer};

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    let summarizer: Box<dyn Summarizer> = Box::new(MockSummarizer);
    rocket::build().manage(pool).manage(summarizer).mount(
        "/",
        routes![
            routes::health,
            track_objects::create,
            track_objects::list,
            track_objects::delete,
            messages::create,
            messages::list,
        ],
    )
}
