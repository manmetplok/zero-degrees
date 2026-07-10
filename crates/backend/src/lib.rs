#[macro_use]
extern crate rocket;

pub mod db;
mod draft_writer;
mod drafts;
mod routes;
mod track_objects;

use draft_writer::{DraftWriter, TemplateDraftWriter};
use rocket::{Build, Rocket};
use sqlx::SqlitePool;
use std::sync::Arc;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    let writer: Arc<dyn DraftWriter> = Arc::new(TemplateDraftWriter);
    rocket::build().manage(pool).manage(writer).mount(
        "/",
        routes![
            routes::health,
            track_objects::create,
            track_objects::list,
            track_objects::delete,
            drafts::create,
            drafts::latest,
            drafts::recharge,
        ],
    )
}
