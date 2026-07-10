#[macro_use]
extern crate rocket;

mod assignments;
mod classifier;
mod combo;
pub mod db;
mod draft_writer;
mod drafts;
mod feedback;
mod leaderboard;
pub mod messages;
mod routes;
pub mod summarizer;
mod track_objects;
mod trophies;
mod xp;

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
            xp::clear,
            xp::progress,
            assignments::get_message,
            assignments::assign,
            assignments::claim,
            assignments::lane,
            assignments::notifications,
            leaderboard::get,
            drafts::create,
            drafts::latest,
            drafts::recharge,
        ],
    )
}
