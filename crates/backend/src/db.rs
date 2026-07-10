use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub async fn connect(database_url: &str) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect(database_url)
        .await
        .expect("database connection failed");
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("migrations failed");
    pool
}
