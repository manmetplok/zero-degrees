#[macro_use]
extern crate rocket;

pub mod db;
mod course_generator;
mod demo;
mod messages;
mod routes;
mod track_objects;

use course_generator::CourseGenerator;
use rocket::{Build, Rocket};
use sqlx::SqlitePool;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    let generator: Box<dyn CourseGenerator> = course_generator::default_generator();
    rocket::build().manage(pool).manage(generator).mount(
        "/",
        routes![
            routes::health,
            track_objects::create,
            track_objects::list,
            track_objects::delete,
            messages::list,
            demo::seed,
        ],
    )
}
