use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{MessageDetail, MessageStatus, SaveDraftRequest, SendReplyRequest};
use sqlx::SqlitePool;

const POINTS_PER_CLEAR: i64 = 10;

async fn client_and_pool() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let seed_pool = pool.clone();
    let client = Client::tracked(backend::rocket(pool)).await.unwrap();
    (client, seed_pool)
}

async fn seed_open_message(pool: &SqlitePool) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO messages (channel, sender, subject, body, received_at) \
         VALUES ('email', 'customer@example.com', 'Order issue', 'Where is my order?', 1700000000) \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

#[rocket::async_test]
async fn sending_reply_resolves_message_and_awards_points() {
    let (client, pool) = client_and_pool().await;
    let id = seed_open_message(&pool).await;

    let response = client
        .post(format!("/messages/{id}/send"))
        .json(&SendReplyRequest {
            reply: "Your order ships tomorrow.".into(),
        })
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let detail: MessageDetail = response.into_json().await.unwrap();
    assert_eq!(detail.message.status, MessageStatus::Cleared);
    assert_eq!(
        detail.reply.as_deref(),
        Some("Your order ships tomorrow.")
    );
    assert_eq!(detail.points_awarded, POINTS_PER_CLEAR);
}

#[rocket::async_test]
async fn draft_save_does_not_resolve_or_award_points() {
    let (client, pool) = client_and_pool().await;
    let id = seed_open_message(&pool).await;

    let response = client
        .put(format!("/messages/{id}/draft"))
        .json(&SaveDraftRequest {
            draft: "Draft in progress...".into(),
        })
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let detail: MessageDetail = response.into_json().await.unwrap();
    assert_eq!(detail.message.status, MessageStatus::Open);
    assert_eq!(detail.points_awarded, 0);
    assert!(detail.reply.is_none());
}

#[rocket::async_test]
async fn balk_keeps_message_open_with_draft_preserved() {
    let (client, pool) = client_and_pool().await;
    let id = seed_open_message(&pool).await;

    client
        .put(format!("/messages/{id}/draft"))
        .json(&SaveDraftRequest {
            draft: "Half-written reply".into(),
        })
        .dispatch()
        .await;

    let response = client.get(format!("/messages/{id}")).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let detail: MessageDetail = response.into_json().await.unwrap();
    assert_eq!(detail.message.status, MessageStatus::Open);
    assert_eq!(detail.draft.as_deref(), Some("Half-written reply"));
    assert_eq!(detail.points_awarded, 0);
}

#[rocket::async_test]
async fn send_after_already_resolved_is_rejected() {
    let (client, pool) = client_and_pool().await;
    let id = seed_open_message(&pool).await;

    client
        .post(format!("/messages/{id}/send"))
        .json(&SendReplyRequest {
            reply: "First reply".into(),
        })
        .dispatch()
        .await;

    let second = client
        .post(format!("/messages/{id}/send"))
        .json(&SendReplyRequest {
            reply: "Second reply".into(),
        })
        .dispatch()
        .await;

    assert_eq!(second.status(), Status::Conflict);

    let response = client.get(format!("/messages/{id}")).dispatch().await;
    let detail: MessageDetail = response.into_json().await.unwrap();
    assert_eq!(detail.reply.as_deref(), Some("First reply"));
    assert_eq!(detail.points_awarded, POINTS_PER_CLEAR);
}
