use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{AssignRequest, AssignedMessage, AssignmentNotification, Channel};
use sqlx::SqlitePool;

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn seed_message(client: &Client, sender: &str) -> i64 {
    let pool = client.rocket().state::<SqlitePool>().unwrap();
    backend::messages::insert(
        pool,
        backend::messages::NewMessage {
            channel: Channel::Email,
            sender: sender.into(),
            subject: "Subject".into(),
            body: "Body".into(),
            received_at: 1_700_000_000,
        },
    )
    .await
}

async fn assign(client: &Client, message_id: i64, runner_device_id: &str) -> AssignedMessage {
    let response = client
        .post(format!("/messages/{message_id}/assign"))
        .json(&AssignRequest {
            runner_device_id: runner_device_id.into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

async fn lane(client: &Client, device_id: &str) -> Vec<AssignedMessage> {
    client
        .get(format!("/players/{device_id}/lane"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap()
}

#[rocket::async_test]
async fn assign_puts_message_in_runners_lane_and_sets_assignment() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;
    let assigned = assign(&client, message_id, "runner-1").await;
    assert_eq!(
        assigned.assignment.unwrap().runner_device_id,
        "runner-1"
    );
    let runner_lane = lane(&client, "runner-1").await;
    assert_eq!(runner_lane.len(), 1);
    assert_eq!(runner_lane[0].id, message_id as u64);
}

#[rocket::async_test]
async fn assign_creates_a_notification_the_runner_can_poll() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;
    assign(&client, message_id, "runner-1").await;
    let notifications: Vec<AssignmentNotification> = client
        .get("/players/runner-1/notifications")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].message_id, message_id as u64);
}

#[rocket::async_test]
async fn reassigning_moves_message_between_lanes_and_keeps_message_data() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;
    assign(&client, message_id, "runner-1").await;
    let reassigned = assign(&client, message_id, "runner-2").await;

    assert_eq!(reassigned.sender, "customer@example.com");
    assert_eq!(
        reassigned.assignment.unwrap().runner_device_id,
        "runner-2"
    );
    assert!(lane(&client, "runner-1").await.is_empty());
    assert_eq!(lane(&client, "runner-2").await.len(), 1);
}

#[rocket::async_test]
async fn claim_on_unassigned_message_succeeds() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;
    let response = client
        .post(format!("/messages/{message_id}/claim"))
        .json(&AssignRequest {
            runner_device_id: "runner-1".into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let claimed: AssignedMessage = response.into_json().await.unwrap();
    assert_eq!(
        claimed.assignment.unwrap().runner_device_id,
        "runner-1"
    );
}

#[rocket::async_test]
async fn second_claim_on_already_claimed_message_is_rejected_as_conflict() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;
    let first = client
        .post(format!("/messages/{message_id}/claim"))
        .json(&AssignRequest {
            runner_device_id: "runner-1".into(),
        })
        .dispatch()
        .await;
    assert_eq!(first.status(), Status::Ok);

    let second = client
        .post(format!("/messages/{message_id}/claim"))
        .json(&AssignRequest {
            runner_device_id: "runner-2".into(),
        })
        .dispatch()
        .await;
    assert_eq!(second.status(), Status::Conflict);
    assert_eq!(lane(&client, "runner-2").await.len(), 0);
    assert_eq!(lane(&client, "runner-1").await.len(), 1);
}

#[rocket::async_test]
async fn claim_on_unknown_message_returns_not_found() {
    let client = client().await;
    let response = client
        .post("/messages/999/claim")
        .json(&AssignRequest {
            runner_device_id: "runner-1".into(),
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn message_payload_reflects_assignment_state_before_and_after_assign() {
    let client = client().await;
    let message_id = seed_message(&client, "customer@example.com").await;

    let before: AssignedMessage = client
        .get(format!("/messages/{message_id}"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert!(before.assignment.is_none());

    assign(&client, message_id, "runner-1").await;

    let after: AssignedMessage = client
        .get(format!("/messages/{message_id}"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(after.assignment.unwrap().runner_device_id, "runner-1");
}

#[rocket::async_test]
async fn lane_excludes_other_runners_messages() {
    let client = client().await;
    let mine = seed_message(&client, "mine@example.com").await;
    let theirs = seed_message(&client, "theirs@example.com").await;
    assign(&client, mine, "runner-1").await;
    assign(&client, theirs, "runner-2").await;

    let runner_lane = lane(&client, "runner-1").await;
    assert_eq!(runner_lane.len(), 1);
    assert_eq!(runner_lane[0].id, mine as u64);
}
