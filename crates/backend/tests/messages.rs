use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{Channel, CreateMessage, Message, MessageStatus, OpenMessages};
use sqlx::SqlitePool;

async fn client_and_pool() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

fn message_body(channel: Channel, sender: &str) -> CreateMessage {
    CreateMessage {
        channel,
        sender: sender.into(),
        subject: "Subject".into(),
        body: "Body".into(),
    }
}

async fn create_message(client: &Client, body: CreateMessage) -> Message {
    let response = client.post("/messages").json(&body).dispatch().await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn create_persists_message_as_open() {
    let (client, _pool) = client_and_pool().await;
    let message = create_message(&client, message_body(Channel::Email, "a@example.com")).await;
    assert!(message.id > 0);
    assert_eq!(message.channel, Channel::Email);
    assert_eq!(message.sender, "a@example.com");
    assert_eq!(message.status, MessageStatus::Open);
}

#[rocket::async_test]
async fn track_listing_orders_open_messages_and_reports_remaining_count() {
    let (client, _pool) = client_and_pool().await;
    create_message(&client, message_body(Channel::Ticket, "first")).await;
    create_message(&client, message_body(Channel::Review, "second")).await;
    create_message(&client, message_body(Channel::WebForm, "third")).await;

    let response = client.get("/track/messages").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let track: OpenMessages = response.into_json().await.unwrap();
    let senders: Vec<&str> = track.messages.iter().map(|m| m.sender.as_str()).collect();
    assert_eq!(senders, vec!["first", "second", "third"]);
    assert_eq!(track.remaining_count, 3);
}

#[rocket::async_test]
async fn track_listing_excludes_non_open_messages() {
    let (client, pool) = client_and_pool().await;
    let open = create_message(&client, message_body(Channel::Email, "open-one")).await;
    let cleared = create_message(&client, message_body(Channel::Email, "cleared-one")).await;
    sqlx::query("UPDATE messages SET status = 'cleared' WHERE id = ?")
        .bind(cleared.id as i64)
        .execute(&pool)
        .await
        .unwrap();

    let response = client.get("/track/messages").dispatch().await;
    let track: OpenMessages = response.into_json().await.unwrap();
    assert_eq!(track.messages.len(), 1);
    assert_eq!(track.messages[0].id, open.id);
    assert_eq!(track.remaining_count, 1);
}
