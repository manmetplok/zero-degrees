#[macro_use]
extern crate rocket;

mod assignments;
mod boss;
mod classifier;
mod combo;
mod course_generator;
pub mod db;
mod daily_run;
mod demo;
mod draft_writer;
mod drafts;
mod feedback;
mod hazard_zones;
mod hurdles;
mod leaderboard;
pub mod messages;
mod response_targets;
mod priority;
mod race_control;
mod routes;
mod scoring;
mod search;
mod sentiment;
mod seeding;
mod streak;
pub mod summarizer;
mod theme;
mod track_objects;
mod urgency;
mod urgency_scorer;
mod trophies;
mod xp;

use course_generator::CourseGenerator;
use draft_writer::{DraftWriter, TemplateDraftWriter};
use rocket::{Build, Rocket};
use sqlx::SqlitePool;
use std::sync::Arc;

pub fn rocket(pool: SqlitePool) -> Rocket<Build> {
    let writer: Arc<dyn DraftWriter> = Arc::new(TemplateDraftWriter);
    let generator: Box<dyn CourseGenerator> = course_generator::default_generator();
    let boss_config = boss::BossConfig::from_env();
    rocket::build()
        .manage(pool)
        .manage(writer)
        .manage(generator)
        .manage(boss_config)
        .mount(
            "/",
            routes![
                routes::health,
                track_objects::create,
                track_objects::list,
                track_objects::delete,
                messages::create,
                messages::list,
                messages::list_open,
                messages::list_open_prioritized,
                daily_run::status,
                daily_run::report_progress,
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
                demo::seed,
                search::search,
                hurdles::create,
                hurdles::list,
                hurdles::clear,
                hurdles::response_time_stats,
                response_targets::list,
                response_targets::update,
                boss::create_message,
                boss::clear_message,
                boss::boss_status,
                race_control::stats,
                hazard_zones::analyze,
                hazard_zones::list,
                hazard_zones::get_one,
                messages::get_detail,
                messages::save_draft,
                messages::send,
            ],
        )
}
