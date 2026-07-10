use crate::course_generator::CourseGenerator;
use crate::seeding;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{SeedRequest, SeedResponse};
use sqlx::SqlitePool;

const DEFAULT_SEED_COUNT: usize = 60;
const MIN_SEED_COUNT: usize = 50;
const DEFAULT_SEED: u64 = 0x5EED_0002;

#[post("/demo/seed", data = "<body>")]
pub async fn seed(
    pool: &State<SqlitePool>,
    generator: &State<Box<dyn CourseGenerator>>,
    body: Json<SeedRequest>,
) -> Result<Json<SeedResponse>, Status> {
    let cleared = if body.reset {
        seeding::clear(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?
    } else {
        0
    };

    let count = body
        .count
        .map(|c| c.max(MIN_SEED_COUNT))
        .unwrap_or(DEFAULT_SEED_COUNT);
    let seed_value = body.seed.unwrap_or(DEFAULT_SEED);

    let generated = generator.generate(body.difficulty, count, seed_value);
    let created = seeding::insert_batch(pool.inner(), generated).await?;

    Ok(Json(SeedResponse {
        created: created.len(),
        cleared: cleared as usize,
        difficulty: body.difficulty,
    }))
}
