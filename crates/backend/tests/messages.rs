use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, CreateMessage, MessageStatus, TriagedMessage, Urgency};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn ingest(client: &Client, subject: &str, body: &str) -> TriagedMessage {
    let response = client
        .post("/messages")
        .json(&CreateMessage {
            channel: Channel::Email,
            sender: "customer@example.com".into(),
            subject: subject.into(),
            body: body.into(),
            received_at: 1_700_000_000,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn critical_message_gets_top_urgency_and_top_reward() {
    let client = client().await;
    let triaged = ingest(&client, "URGENT", "our service is down for everyone").await;
    assert_eq!(triaged.urgency, Urgency::Critical);
    assert_eq!(triaged.point_reward, Urgency::Critical.point_reward());
    assert!(!triaged.rationale.is_empty());
    assert_eq!(triaged.message.status, MessageStatus::Open);
}

#[rocket::async_test]
async fn routine_message_gets_low_urgency_and_base_reward() {
    let client = client().await;
    let triaged = ingest(&client, "Heads up", "for your information, no action needed").await;
    assert_eq!(triaged.urgency, Urgency::Low);
    assert_eq!(triaged.point_reward, Urgency::Low.point_reward());
}

#[rocket::async_test]
async fn detail_card_lookup_returns_persisted_urgency_and_rationale() {
    let client = client().await;
    let created = ingest(&client, "URGENT", "our service is down for everyone").await;
    let response = client
        .get(format!("/messages/{}", created.message.id))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let fetched: TriagedMessage = response.into_json().await.unwrap();
    assert_eq!(fetched.urgency, created.urgency);
    assert_eq!(fetched.rationale, created.rationale);
}

#[rocket::async_test]
async fn unknown_message_id_returns_not_found() {
    let client = client().await;
    let response = client.get("/messages/999").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn list_returns_every_ingested_message() {
    let client = client().await;
    ingest(&client, "URGENT", "our service is down for everyone").await;
    ingest(&client, "Heads up", "for your information, no action needed").await;
    let response = client.get("/messages").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let messages: Vec<TriagedMessage> = response.into_json().await.unwrap();
    assert_eq!(messages.len(), 2);
}
