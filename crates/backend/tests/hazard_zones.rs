use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{HazardZone, HazardZoneDetail};
use sqlx::SqlitePool;

async fn client_with_pool() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let seed_pool = pool.clone();
    let client = Client::tracked(backend::rocket(pool)).await.unwrap();
    (client, seed_pool)
}

async fn seed_message(pool: &SqlitePool, channel: &str, subject: &str, body: &str, status: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO messages (channel, sender, subject, body, received_at, status) \
         VALUES (?, 'someone@example.com', ?, ?, 0, ?) RETURNING id",
    )
    .bind(channel)
    .bind(subject)
    .bind(body)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_checkout_spike(pool: &SqlitePool) {
    seed_message(pool, "email", "Checkout broken", "The checkout page throws an error.", "open").await;
    seed_message(pool, "ticket", "Can't checkout", "Checkout fails at payment step.", "open").await;
    seed_message(pool, "review", "Checkout issue", "Getting stuck during checkout again.", "open").await;
}

#[rocket::async_test]
async fn analyze_creates_a_zone_for_a_recurring_theme() {
    let (client, pool) = client_with_pool().await;
    seed_checkout_spike(&pool).await;

    let response = client.post("/hazard-zones/analyze").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let zones: Vec<HazardZone> = response.into_json().await.unwrap();

    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].name, "Checkout issues");
    assert_eq!(zones[0].message_count, 3);
    assert_eq!(zones[0].message_ids.len(), 3);
}

#[rocket::async_test]
async fn analyze_ignores_isolated_messages_below_threshold() {
    let (client, pool) = client_with_pool().await;
    seed_message(&pool, "email", "One off", "Nothing else like this.", "open").await;

    let response = client.post("/hazard-zones/analyze").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let zones: Vec<HazardZone> = response.into_json().await.unwrap();
    assert!(zones.is_empty());
}

#[rocket::async_test]
async fn list_returns_zones_from_the_last_analysis() {
    let (client, pool) = client_with_pool().await;
    seed_checkout_spike(&pool).await;
    client.post("/hazard-zones/analyze").dispatch().await;

    let response = client.get("/hazard-zones").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let zones: Vec<HazardZone> = response.into_json().await.unwrap();
    assert_eq!(zones.len(), 1);
}

#[rocket::async_test]
async fn get_zone_returns_messages_and_briefing() {
    let (client, pool) = client_with_pool().await;
    seed_checkout_spike(&pool).await;
    client.post("/hazard-zones/analyze").dispatch().await;

    let zones: Vec<HazardZone> = client
        .get("/hazard-zones")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    let zone_id = zones[0].id;

    let response = client
        .get(format!("/hazard-zones/{}", zone_id))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let detail: HazardZoneDetail = response.into_json().await.unwrap();

    assert_eq!(detail.messages.len(), 3);
    assert!(detail.briefing.contains("checkout"));
}

#[rocket::async_test]
async fn get_unknown_zone_returns_not_found() {
    let (client, _pool) = client_with_pool().await;
    let response = client.get("/hazard-zones/999").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn analyze_replaces_previous_zones_on_rerun() {
    let (client, pool) = client_with_pool().await;
    seed_checkout_spike(&pool).await;
    client.post("/hazard-zones/analyze").dispatch().await;

    seed_message(&pool, "email", "Shipping delay", "My shipping has not moved in a week.", "open").await;
    seed_message(&pool, "ticket", "Shipping stuck", "Shipping status has not updated.", "open").await;
    seed_message(&pool, "review", "Shipping late", "Still waiting on shipping to arrive.", "open").await;

    let response = client.post("/hazard-zones/analyze").dispatch().await;
    let zones: Vec<HazardZone> = response.into_json().await.unwrap();
    assert_eq!(zones.len(), 2);

    let listed: Vec<HazardZone> = client
        .get("/hazard-zones")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
}
