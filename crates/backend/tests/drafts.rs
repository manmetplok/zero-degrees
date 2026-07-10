use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{RechargeDraft, ReplyDraft};
use sqlx::SqlitePool;

async fn client_and_pool() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

async fn seed_message(pool: &SqlitePool, subject: &str, body: &str, language: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO messages (subject, body, language) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(subject)
    .bind(body)
    .bind(language)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

#[rocket::async_test]
async fn create_draft_persists_and_returns_it() {
    let (client, pool) = client_and_pool().await;
    let message_id = seed_message(&pool, "Order missing", "My order never arrived.", "en").await;

    let response = client
        .post(format!("/messages/{message_id}/draft"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let draft: ReplyDraft = response.into_json().await.unwrap();
    assert_eq!(draft.message_id, message_id);
    assert!(draft.content.contains("Order missing"));
    assert!(draft.steering_note.is_none());

    let fetched: ReplyDraft = client
        .get(format!("/messages/{message_id}/draft"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(fetched.id, draft.id);
}

#[rocket::async_test]
async fn create_draft_for_unknown_message_returns_not_found() {
    let (client, _pool) = client_and_pool().await;
    let response = client.post("/messages/999/draft").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn recharge_stores_new_latest_draft_with_steering_note() {
    let (client, pool) = client_and_pool().await;
    let message_id = seed_message(&pool, "Refund request", "I want my money back.", "en").await;
    let first: ReplyDraft = client
        .post(format!("/messages/{message_id}/draft"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let response = client
        .post(format!("/messages/{message_id}/draft/recharge"))
        .json(&RechargeDraft {
            steering_note: "Offer a 10% discount instead".into(),
        })
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let recharged: ReplyDraft = response.into_json().await.unwrap();
    assert_ne!(recharged.id, first.id);
    assert_eq!(
        recharged.steering_note.as_deref(),
        Some("Offer a 10% discount instead")
    );
    assert!(recharged.content.contains("Offer a 10% discount instead"));

    let latest: ReplyDraft = client
        .get(format!("/messages/{message_id}/draft"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(latest.id, recharged.id);
}

#[rocket::async_test]
async fn recharge_for_unknown_message_returns_not_found() {
    let (client, _pool) = client_and_pool().await;
    let response = client
        .post("/messages/999/draft/recharge")
        .json(&RechargeDraft {
            steering_note: "Be more concise".into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn latest_draft_without_any_draft_returns_not_found() {
    let (client, pool) = client_and_pool().await;
    let message_id = seed_message(&pool, "Question", "Just a question.", "en").await;
    let response = client
        .get(format!("/messages/{message_id}/draft"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}
