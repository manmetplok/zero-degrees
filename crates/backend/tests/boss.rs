use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{BossMessage, BossStatus, Channel, ClearBossMessage, CreateBossMessage, Priority};
use sqlx::SqlitePool;

async fn client() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

async fn create_message(client: &Client, subject: &str, priority: Priority) -> BossMessage {
    let response = client
        .post("/boss/messages")
        .json(&CreateBossMessage {
            channel: Channel::Email,
            sender: "customer@example.com".into(),
            subject: subject.into(),
            priority,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

async fn clear_message(client: &Client, id: i64, runner: &str) -> BossStatus {
    let response = client
        .post(format!("/boss/messages/{id}/clear"))
        .json(&ClearBossMessage {
            runner: runner.into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

async fn boss_status(client: &Client) -> BossStatus {
    let response = client.get("/boss/status").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn status_with_no_messages_reports_no_boss() {
    let (client, _pool) = client().await;
    let status = boss_status(&client).await;
    assert_eq!(status.battle_id, None);
    assert_eq!(status.health, 0);
    assert!(!status.victory);
    assert!(!status.enraged);
}

#[rocket::async_test]
async fn new_messages_grow_boss_health() {
    let (client, _pool) = client().await;
    create_message(&client, "Refund request", Priority::Normal).await;
    create_message(&client, "Cannot log in", Priority::High).await;

    let status = boss_status(&client).await;
    assert_eq!(status.health, Priority::Normal.weight() + Priority::High.weight());
    assert_eq!(status.max_health, status.health);
    assert!(status.battle_id.is_some());
    assert!(!status.victory);
}

#[rocket::async_test]
async fn clearing_a_hurdle_credits_the_runner_and_drops_health() {
    let (client, _pool) = client().await;
    let message = create_message(&client, "Billing question", Priority::Normal).await;
    create_message(&client, "Feature idea", Priority::Low).await;

    let status = clear_message(&client, message.id, "alice").await;

    assert_eq!(status.health, Priority::Low.weight());
    assert_eq!(status.recent_hits.len(), 1);
    let hit = &status.recent_hits[0];
    assert_eq!(hit.runner, "alice");
    assert_eq!(hit.message_id, message.id);
    assert_eq!(hit.damage, Priority::Normal.weight());
    assert_eq!(status.contributions.len(), 1);
    assert_eq!(status.contributions[0].runner, "alice");
    assert_eq!(status.contributions[0].hits, 1);
}

#[rocket::async_test]
async fn clearing_the_last_message_declares_victory() {
    let (client, _pool) = client().await;
    let message = create_message(&client, "Only hurdle left", Priority::Low).await;

    let status = clear_message(&client, message.id, "bob").await;

    assert_eq!(status.health, 0);
    assert!(status.victory);
}

#[rocket::async_test]
async fn clearing_an_already_cleared_message_conflicts() {
    let (client, _pool) = client().await;
    let message = create_message(&client, "One and done", Priority::Low).await;
    clear_message(&client, message.id, "bob").await;

    let response = client
        .post(format!("/boss/messages/{}/clear", message.id))
        .json(&ClearBossMessage {
            runner: "carol".into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Conflict);
}

#[rocket::async_test]
async fn new_boss_respawns_after_victory_without_losing_historical_stats() {
    let (client, _pool) = client().await;
    let first = create_message(&client, "First wave", Priority::High).await;
    clear_message(&client, first.id, "alice").await;

    let victory_status = boss_status(&client).await;
    let first_battle_id = victory_status.battle_id;
    assert!(victory_status.victory);

    let second = create_message(&client, "Second wave", Priority::Normal).await;
    let status = boss_status(&client).await;

    assert_ne!(status.battle_id, first_battle_id);
    assert_eq!(status.health, Priority::Normal.weight());
    assert_eq!(status.max_health, Priority::Normal.weight());
    assert!(!status.victory);
    assert_eq!(status.contributions.len(), 1);
    assert_eq!(status.contributions[0].runner, "alice");
    assert_eq!(status.contributions[0].damage, Priority::High.weight());

    clear_message(&client, second.id, "alice").await;
    let final_status = boss_status(&client).await;
    assert_eq!(final_status.contributions[0].hits, 2);
    assert_eq!(
        final_status.contributions[0].damage,
        Priority::High.weight() + Priority::Normal.weight()
    );
}

#[rocket::async_test]
async fn enrage_triggers_once_burning_hurdles_cross_the_threshold() {
    let (client, pool) = client().await;
    for i in 0..3 {
        create_message(&client, &format!("Aging ticket {i}"), Priority::Low).await;
    }
    let fresh_status = boss_status(&client).await;
    assert!(!fresh_status.enraged);

    sqlx::query("UPDATE messages SET received_at = received_at - 4000")
        .execute(&pool)
        .await
        .unwrap();

    let burning_status = boss_status(&client).await;
    assert_eq!(burning_status.burning_count, 3);
    assert!(burning_status.enraged);
}
