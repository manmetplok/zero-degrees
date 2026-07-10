use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{CategorizedMessage, SeedRequest, SeedResponse};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn seed(client: &Client, request: SeedRequest) -> SeedResponse {
    let response = client.post("/demo/seed").json(&request).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

async fn list_messages(client: &Client) -> Vec<CategorizedMessage> {
    client
        .get("/messages")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap()
}

#[rocket::async_test]
async fn seed_creates_at_least_fifty_messages_by_default() {
    let client = client().await;
    let response = seed(&client, SeedRequest::default()).await;
    assert!(response.created >= 50);
    let messages = list_messages(&client).await;
    assert_eq!(messages.len(), response.created);
}

#[rocket::async_test]
async fn seed_enforces_minimum_count_of_fifty() {
    let client = client().await;
    let response = seed(
        &client,
        SeedRequest {
            count: Some(5),
            ..Default::default()
        },
    )
    .await;
    assert!(response.created >= 50);
}

#[rocket::async_test]
async fn seed_reset_clears_previous_course() {
    let client = client().await;
    let first = seed(&client, SeedRequest::default()).await;
    let second = seed(
        &client,
        SeedRequest {
            reset: true,
            count: Some(50),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(second.cleared, first.created);
    let messages = list_messages(&client).await;
    assert_eq!(messages.len(), 50);
}

#[rocket::async_test]
async fn seed_without_reset_appends_to_existing_course() {
    let client = client().await;
    let first = seed(&client, SeedRequest::default()).await;
    let second = seed(&client, SeedRequest::default()).await;
    let messages = list_messages(&client).await;
    assert_eq!(messages.len(), first.created + second.created);
}
