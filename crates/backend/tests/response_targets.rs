use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{ResponseTarget, UpdateResponseTarget, Urgency};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

#[rocket::async_test]
async fn list_returns_a_seeded_target_for_every_urgency_level() {
    let client = client().await;
    let response = client.get("/response-targets").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let targets: Vec<ResponseTarget> = response.into_json().await.unwrap();
    for urgency in Urgency::ALL {
        assert!(targets.iter().any(|target| target.urgency == urgency));
    }
}

#[rocket::async_test]
async fn update_changes_the_target_for_that_urgency_only() {
    let client = client().await;
    let response = client
        .put("/response-targets/high")
        .json(&UpdateResponseTarget {
            target_seconds: 120,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated: ResponseTarget = response.into_json().await.unwrap();
    assert_eq!(updated.target_seconds, 120);

    let targets: Vec<ResponseTarget> = client
        .get("/response-targets")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    let high = targets
        .iter()
        .find(|target| target.urgency == Urgency::High)
        .unwrap();
    assert_eq!(high.target_seconds, 120);
    let normal = targets
        .iter()
        .find(|target| target.urgency == Urgency::Normal)
        .unwrap();
    assert_ne!(normal.target_seconds, 120);
}

#[rocket::async_test]
async fn update_rejects_a_non_positive_target() {
    let client = client().await;
    let response = client
        .put("/response-targets/low")
        .json(&UpdateResponseTarget { target_seconds: 0 })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
}

#[rocket::async_test]
async fn update_of_unknown_urgency_returns_not_found() {
    let client = client().await;
    let response = client
        .put("/response-targets/urgent")
        .json(&UpdateResponseTarget {
            target_seconds: 60,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}
