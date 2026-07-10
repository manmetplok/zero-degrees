use backend::db;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:zero-degrees.db?mode=rwc".into());
    let pool = db::connect(&database_url).await;
    backend::rocket(pool).launch().await?;
    Ok(())
}
