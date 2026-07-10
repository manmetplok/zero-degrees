use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{CategorizedMessage, Category, Channel, CreateMessage, SetMessageCategory};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

fn billing_message() -> CreateMessage {
    CreateMessage {
        channel: Channel::Email,
        sender: "customer@example.com".into(),
        subject: "Invoice charged twice".into(),
        body: "I was billed twice for my invoice, please refund the extra charge.".into(),
        received_at: 1_780_000_000,
    }
}

fn feedback_message() -> CreateMessage {
    CreateMessage {
        channel: Channel::Review,
        sender: "reviewer@example.com".into(),
        subject: "Great support".into(),
        body: "Just wanted to say I love the product, here's an idea for improvement.".into(),
        received_at: 1_780_000_100,
    }
}

async fn create_message(client: &Client, message: CreateMessage) -> CategorizedMessage {
    let response = client
        .post("/messages")
        .json(&message)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn create_assigns_ai_category_at_ingestion() {
    let client = client().await;
    let created = create_message(&client, billing_message()).await;
    assert_eq!(created.category, Category::Billing);
    assert_eq!(created.channel, Channel::Email);
}

#[rocket::async_test]
async fn list_returns_every_message_with_its_category() {
    let client = client().await;
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
    let client = client().await;
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
async fn override_unknown_message_returns_not_found() {
    let client = client().await;
    let response = client
        .patch("/messages/999/category")
        .json(&SetMessageCategory {
            category: Category::Feedback,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}
