use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, CreateMessage, Message};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn create_message(client: &Client, body: &str) -> Message {
    let response = client
        .post("/messages")
        .json(&CreateMessage {
            channel: Channel::Email,
            sender: "customer@example.com".into(),
            subject: "Help".into(),
            body: body.into(),
            received_at: 1_700_000_000,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn short_message_gets_no_summary() {
    let client = client().await;
    let message = create_message(&client, "The checkout button is missing.").await;
    assert_eq!(message.summary, None);
}

const LONG_BODY: &str = "Our checkout page has been failing since this morning. Customers see a blank screen instead of the payment form. This is affecting every browser we have tested so far, including Chrome and Safari on both desktop and mobile. Please treat this as urgent since it blocks every purchase.";

#[rocket::async_test]
async fn long_message_gets_a_persisted_summary() {
    let client = client().await;
    assert!(LONG_BODY.chars().count() > backend::summarizer::SUMMARY_THRESHOLD_CHARS);
    let message = create_message(&client, LONG_BODY).await;
    let summary = message.summary.expect("long message should get a summary");
    assert!(!summary.is_empty());
    assert!(summary.len() < LONG_BODY.len());
}

#[rocket::async_test]
async fn list_returns_persisted_summary() {
    let client = client().await;
    let created = create_message(&client, LONG_BODY).await;
    let response = client.get("/messages").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let messages: Vec<Message> = response.into_json().await.unwrap();
    let listed = messages.into_iter().find(|m| m.id == created.id).unwrap();
    assert_eq!(listed.summary, created.summary);
    assert_eq!(listed.body, LONG_BODY);
}
