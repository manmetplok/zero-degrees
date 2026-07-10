use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, HurdleClearResult, CreateHurdleMessage, HurdleMessage, ResponseTimeStats, Urgency};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

async fn create_message(
    client: &Client,
    urgency: Urgency,
    received_at: i64,
) -> HurdleMessage {
    let response = client
        .post("/hurdles")
        .json(&CreateHurdleMessage {
            channel: Channel::Email,
            sender: "customer@example.com".into(),
            subject: "Help".into(),
            body: "Something is on fire.".into(),
            urgency,
            received_at: Some(received_at),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn fresh_message_is_open_and_not_burning() {
    let client = client().await;
    let message = create_message(&client, Urgency::Normal, now()).await;
    assert!(!message.burning);
    assert_eq!(message.response_seconds, None);
    assert!(message.waiting_seconds < 5);
}

#[rocket::async_test]
async fn open_message_ignites_once_it_outlasts_its_target() {
    let client = client().await;
    let received_at = now() - 400;
    let message = create_message(&client, Urgency::Critical, received_at).await;
    assert!(message.burning);
    assert!(message.waiting_seconds >= 400);
}

#[rocket::async_test]
async fn listing_reflects_the_same_burning_state_as_creation() {
    let client = client().await;
    create_message(&client, Urgency::Critical, now() - 400).await;
    create_message(&client, Urgency::Low, now()).await;

    let response = client.get("/hurdles").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let messages: Vec<HurdleMessage> = response.into_json().await.unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].burning);
    assert!(!messages[1].burning);
}

#[rocket::async_test]
async fn clearing_within_target_awards_full_points_and_a_speed_bonus() {
    let client = client().await;
    let message = create_message(&client, Urgency::Normal, now()).await;

    let response = client
        .post(format!("/hurdles/{}/clear", message.id))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let result: HurdleClearResult = response.into_json().await.unwrap();
    assert!(!result.burning);
    assert!(result.speed_bonus_awarded > 0);
    assert!(result.points_awarded > result.speed_bonus_awarded);
    let response_seconds = result.message.response_seconds.unwrap();
    assert!((0..5).contains(&response_seconds));
}

#[rocket::async_test]
async fn clearing_past_target_awards_partial_points_and_no_bonus() {
    let client = client().await;
    let message = create_message(&client, Urgency::Critical, now() - 400).await;

    let response = client
        .post(format!("/hurdles/{}/clear", message.id))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let result: HurdleClearResult = response.into_json().await.unwrap();
    assert!(result.burning);
    assert_eq!(result.speed_bonus_awarded, 0);
    assert!(result.points_awarded > 0);
}

#[rocket::async_test]
async fn clearing_an_already_cleared_message_returns_conflict() {
    let client = client().await;
    let message = create_message(&client, Urgency::Low, now()).await;
    let path = format!("/hurdles/{}/clear", message.id);

    let first = client.post(&path).dispatch().await;
    assert_eq!(first.status(), Status::Ok);

    let second = client.post(&path).dispatch().await;
    assert_eq!(second.status(), Status::Conflict);
}

#[rocket::async_test]
async fn clearing_an_unknown_message_returns_not_found() {
    let client = client().await;
    let response = client.post("/hurdles/999/clear").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn response_time_stats_aggregate_only_cleared_messages_per_urgency() {
    let client = client().await;
    let on_time = create_message(&client, Urgency::High, now()).await;
    let burning = create_message(&client, Urgency::High, now() - 1_000).await;
    create_message(&client, Urgency::High, now()).await;

    client
        .post(format!("/hurdles/{}/clear", on_time.id))
        .dispatch()
        .await;
    client
        .post(format!("/hurdles/{}/clear", burning.id))
        .dispatch()
        .await;

    let response = client.get("/response-times").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let stats: Vec<ResponseTimeStats> = response.into_json().await.unwrap();
    let high = stats
        .iter()
        .find(|stat| stat.urgency == Urgency::High)
        .unwrap();
    assert_eq!(high.cleared_count, 2);
    assert_eq!(high.burning_count, 1);
}
