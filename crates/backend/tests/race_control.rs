use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, RaceControlStats, Sentiment};
use sqlx::SqlitePool;

async fn setup() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

async fn insert_player(pool: &SqlitePool, device_id: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO players (device_id) VALUES (?) RETURNING id")
        .bind(device_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[derive(Default)]
struct SeedMessage<'a> {
    channel: &'a str,
    category: Option<&'a str>,
    sentiment: Option<&'a str>,
    status: &'a str,
    received_at: Option<&'a str>,
    cleared_by: Option<i64>,
}

async fn insert_message(pool: &SqlitePool, seed: SeedMessage<'_>) {
    sqlx::query(
        "INSERT INTO messages (channel, category, sentiment, status, received_at, cleared_by) \
         VALUES (?, ?, ?, ?, COALESCE(?, datetime('now')), ?)",
    )
    .bind(seed.channel)
    .bind(seed.category)
    .bind(seed.sentiment)
    .bind(seed.status)
    .bind(seed.received_at)
    .bind(seed.cleared_by)
    .execute(pool)
    .await
    .unwrap();
}

async fn fetch_stats(client: &Client) -> RaceControlStats {
    let response = client.get("/race-control/stats").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn stats_aggregate_counts_volumes_and_runner_progress() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "device-alice").await;
    insert_player(&pool, "device-bob").await;

    insert_message(
        &pool,
        SeedMessage {
            channel: "email",
            category: Some("billing"),
            sentiment: Some("angry"),
            status: "open",
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "email",
            category: Some("billing"),
            sentiment: None,
            status: "open",
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "ticket",
            category: Some("question"),
            sentiment: Some("neutral"),
            status: "cleared",
            cleared_by: Some(alice),
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "review",
            category: None,
            sentiment: None,
            status: "cleared",
            cleared_by: Some(alice),
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "web_form",
            category: Some("feedback"),
            sentiment: Some("positive"),
            status: "skipped",
            ..Default::default()
        },
    )
    .await;

    let stats = fetch_stats(&client).await;

    assert_eq!(stats.open_count, 2);
    assert_eq!(stats.cleared_count, 2);
    assert!(stats.hazard_zones.is_empty());

    let email_volume = stats
        .channel_volume
        .iter()
        .find(|c| c.channel == Channel::Email)
        .unwrap();
    assert_eq!(email_volume.count, 2);

    let billing = stats
        .category_distribution
        .iter()
        .find(|c| c.category == "billing")
        .unwrap();
    assert_eq!(billing.count, 2);
    assert!(stats
        .category_distribution
        .iter()
        .all(|c| c.category != "review"));

    let angry = stats
        .sentiment_breakdown
        .iter()
        .find(|s| s.sentiment == Sentiment::Angry)
        .unwrap();
    assert_eq!(angry.count, 1);

    let alice_progress = stats
        .runner_progress
        .iter()
        .find(|r| r.device_id == "device-alice")
        .unwrap();
    assert_eq!(alice_progress.clears, 2);
    let bob_progress = stats
        .runner_progress
        .iter()
        .find(|r| r.device_id == "device-bob")
        .unwrap();
    assert_eq!(bob_progress.clears, 0);
}

#[rocket::async_test]
async fn overdue_count_only_includes_open_messages_past_the_threshold() {
    let (client, pool) = setup().await;

    insert_message(
        &pool,
        SeedMessage {
            channel: "email",
            status: "open",
            received_at: Some("2000-01-01 00:00:00"),
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "email",
            status: "open",
            ..Default::default()
        },
    )
    .await;
    insert_message(
        &pool,
        SeedMessage {
            channel: "email",
            status: "cleared",
            received_at: Some("2000-01-01 00:00:00"),
            ..Default::default()
        },
    )
    .await;

    let stats = fetch_stats(&client).await;

    assert_eq!(stats.overdue_count, 1);
}
