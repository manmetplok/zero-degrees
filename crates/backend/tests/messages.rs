use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{
    CategorizedMessage, Category, Channel, CreateMessage, MessageStatus, OpenMessages,
    SetMessageCategory,
};
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

fn billing_message() -> CreateMessage {
    CreateMessage {
        channel: Channel::Email,
        sender: "customer@example.com".into(),
        subject: "Invoice charged twice".into(),
        body: "I was billed twice for my invoice, please refund the extra charge.".into(),
    }
}

fn feedback_message() -> CreateMessage {
    CreateMessage {
        channel: Channel::Review,
        sender: "reviewer@example.com".into(),
        subject: "Great support".into(),
        body: "Just wanted to say I love the product, here's an idea for improvement.".into(),
    }
}

async fn create_message(client: &Client, body: CreateMessage) -> CategorizedMessage {
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
        .bind(cleared.id)
        .execute(&pool)
        .await
        .unwrap();

    let response = client.get("/track/messages").dispatch().await;
    let track: OpenMessages = response.into_json().await.unwrap();
    assert_eq!(track.messages.len(), 1);
    assert_eq!(track.messages[0].id, open.id as u64);
    assert_eq!(track.remaining_count, 1);
}

#[rocket::async_test]
async fn create_assigns_ai_category_at_ingestion() {
    let (client, _pool) = client_and_pool().await;
    let created = create_message(&client, billing_message()).await;
    assert_eq!(created.category, Category::Billing);
    assert_eq!(created.channel, Channel::Email);
}

#[rocket::async_test]
async fn list_returns_every_message_with_its_category() {
    let (client, _pool) = client_and_pool().await;
    create_message(&client, billing_message()).await;
    create_message(&client, feedback_message()).await;

    let response = client.get("/messages").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let messages: Vec<CategorizedMessage> = response.into_json().await.unwrap();
    let categories: Vec<Category> = messages.iter().map(|m| m.category).collect();
    assert_eq!(categories, vec![Category::Billing, Category::Feedback]);
}

#[rocket::async_test]
async fn override_category_wins_over_ai_category_and_persists() {
    let (client, _pool) = client_and_pool().await;
    let created = create_message(&client, billing_message()).await;
    assert_eq!(created.category, Category::Billing);

    let response = client
        .patch(format!("/messages/{}/category", created.id))
        .json(&SetMessageCategory {
            category: Category::Complaint,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated: CategorizedMessage = response.into_json().await.unwrap();
    assert_eq!(updated.category, Category::Complaint);

    let messages: Vec<CategorizedMessage> = client
        .get("/messages")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(messages[0].category, Category::Complaint);
}

#[rocket::async_test]
async fn short_message_gets_no_summary() {
    let (client, _pool) = client_and_pool().await;
    let created = create_message(&client, message_body(Channel::Email, "short")).await;
    assert_eq!(created.summary, None);
}

#[rocket::async_test]
async fn long_message_gets_persisted_summary() {
    let (client, _pool) = client_and_pool().await;
    let long_body = "This is the first sentence of a long complaint. This is the second one. "
        .repeat(5);
    let created = create_message(
        &client,
        CreateMessage {
            channel: Channel::Email,
            sender: "long@example.com".into(),
            subject: "Long message".into(),
            body: long_body.clone(),
        },
    )
    .await;
    let summary = created.summary.expect("long message should get a summary");
    assert!(!summary.is_empty());
    assert!(summary.len() < long_body.len());

    let messages: Vec<CategorizedMessage> = client
        .get("/messages")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(messages[0].summary.as_deref(), Some(summary.as_str()));
}

#[rocket::async_test]
async fn override_unknown_message_returns_not_found() {
    let (client, _pool) = client_and_pool().await;
    let response = client
        .patch("/messages/999/category")
        .json(&SetMessageCategory {
            category: Category::Feedback,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}
