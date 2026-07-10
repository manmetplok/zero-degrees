use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, CreateMessage, MessageRecord, MessageStatus, Sentiment};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

fn message(subject: &str, body: &str) -> CreateMessage {
    CreateMessage {
        channel: Channel::Email,
        sender: "customer@example.com".into(),
        subject: subject.into(),
        body: body.into(),
        received_at: 1_780_000_000,
    }
}

async fn create_message(client: &Client, msg: CreateMessage) -> MessageRecord {
    let response = client.post("/messages").json(&msg).dispatch().await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn create_persists_message_with_classified_sentiment() {
    let client = client().await;
    let record = create_message(
        &client,
        message(
            "This is unacceptable",
            "I am furious, this is the worst service I have ever had.",
        ),
    )
    .await;
    assert!(record.id > 0);
    assert_eq!(record.sentiment, Sentiment::Angry);
    assert_eq!(record.status, MessageStatus::Open);
}

#[rocket::async_test]
async fn list_filters_by_sentiment() {
    let client = client().await;
    create_message(&client, message("Thank you", "Thanks, great service!")).await;
    create_message(
        &client,
        message("Unacceptable", "This is furious and unacceptable, worst ever."),
    )
    .await;

    let response = client.get("/messages?sentiment=angry").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let records: Vec<MessageRecord> = response.into_json().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].sentiment, Sentiment::Angry);
}

#[rocket::async_test]
async fn list_without_filter_returns_every_message() {
    let client = client().await;
    create_message(&client, message("Thank you", "Thanks, great service!")).await;
    create_message(&client, message("Pricing", "Do you have a family plan?")).await;

    let response = client.get("/messages").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let records: Vec<MessageRecord> = response.into_json().await.unwrap();
    assert_eq!(records.len(), 2);
}

#[rocket::async_test]
async fn list_with_unknown_sentiment_returns_bad_request() {
    let client = client().await;
    let response = client.get("/messages?sentiment=bogus").dispatch().await;
    assert_eq!(response.status(), Status::BadRequest);
}
