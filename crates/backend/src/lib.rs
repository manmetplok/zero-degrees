#[macro_use]
extern crate rocket;

pub mod db;
mod messages;
mod routes;
mod sentiment;
mod track_objects;

use rocket::{Build, Rocket};
use sentiment::{KeywordSentimentClassifier, SentimentClassifier};
use sqlx::SqlitePool;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    let classifier: Box<dyn SentimentClassifier> = Box::new(KeywordSentimentClassifier);
    rocket::build()
        .manage(pool)
        .manage(classifier)
        .mount(
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
