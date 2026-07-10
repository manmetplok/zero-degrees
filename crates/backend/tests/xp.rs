use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{ClearHurdle, ClearResult, PlayerProgress, UrgencyLevel};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn clear_hurdle(
    client: &Client,
    device_id: &str,
    urgency: UrgencyLevel,
    on_time: bool,
) -> ClearResult {
    let response = client
        .post(format!("/players/{device_id}/clears"))
        .json(&ClearHurdle { urgency, on_time })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn base_clear_awards_xp_scaled_by_urgency() {
    let client = client().await;
    let result = clear_hurdle(&client, "device-a", UrgencyLevel::Low, false).await;
    assert_eq!(result.xp_awarded, 10);
    assert_eq!(result.total_xp, 10);
    assert_eq!(result.combo_multiplier, 1.0);
}

#[rocket::async_test]
async fn consecutive_clears_build_the_combo_multiplier() {
    let client = client().await;
    let first = clear_hurdle(&client, "device-b", UrgencyLevel::Low, false).await;
    let second = clear_hurdle(&client, "device-b", UrgencyLevel::Low, false).await;
    let third = clear_hurdle(&client, "device-b", UrgencyLevel::Low, false).await;
    assert_eq!(
        (first.combo_multiplier, second.combo_multiplier, third.combo_multiplier),
        (1.0, 1.5, 2.0)
    );
    assert_eq!(third.total_xp, first.xp_awarded + second.xp_awarded + third.xp_awarded);
}

#[rocket::async_test]
async fn critical_on_time_clear_awards_more_than_a_low_late_clear() {
    let client = client().await;
    let critical = clear_hurdle(&client, "device-c", UrgencyLevel::Critical, true).await;
    let low = clear_hurdle(&client, "device-d", UrgencyLevel::Low, false).await;
    assert!(critical.xp_awarded > low.xp_awarded);
}

#[rocket::async_test]
async fn clears_are_tracked_separately_per_device() {
    let client = client().await;
    clear_hurdle(&client, "device-e", UrgencyLevel::Normal, false).await;
    let other_device = clear_hurdle(&client, "device-f", UrgencyLevel::Normal, false).await;
    assert_eq!(other_device.total_xp, other_device.xp_awarded);
}

#[rocket::async_test]
async fn progress_reports_persisted_totals_and_combo_state() {
    let client = client().await;
    clear_hurdle(&client, "device-g", UrgencyLevel::High, true).await;
    let last = clear_hurdle(&client, "device-g", UrgencyLevel::High, true).await;

    let response = client.get("/players/device-g/progress").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let progress: PlayerProgress = response.into_json().await.unwrap();
    assert_eq!(progress.total_xp, last.total_xp);
    assert_eq!(progress.combo_multiplier, last.combo_multiplier);
    assert_eq!(progress.combo_count, last.combo_count);
}

#[rocket::async_test]
async fn progress_for_unknown_device_defaults_to_zero() {
    let client = client().await;
    let response = client
        .get("/players/never-seen/progress")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let progress: PlayerProgress = response.into_json().await.unwrap();
    assert_eq!(progress.total_xp, 0);
    assert_eq!(progress.combo_multiplier, 1.0);
    assert_eq!(progress.combo_count, 0);
}
