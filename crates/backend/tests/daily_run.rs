use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{DailyRunStatus, ReportDailyProgress};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn report(client: &Client, player_id: i64, xp_earned: u32) -> DailyRunStatus {
    let response = client
        .post(format!("/players/{player_id}/daily-run/progress"))
        .json(&ReportDailyProgress { xp_earned })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

async fn status(client: &Client, player_id: i64) -> DailyRunStatus {
    let response = client
        .get(format!("/players/{player_id}/daily-run"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn new_player_starts_with_no_streak() {
    let client = client().await;
    let result = status(&client, 1).await;
    assert_eq!(result.current_streak, 0);
    assert_eq!(result.best_streak, 0);
    assert!(!result.has_shield);
    assert_eq!(result.today_progress_xp, 0);
    assert!(!result.goal_met_today);
}

#[rocket::async_test]
async fn progress_below_goal_does_not_complete_the_run() {
    let client = client().await;
    let result = report(&client, 1, 40).await;
    assert_eq!(result.today_progress_xp, 40);
    assert!(!result.goal_met_today);
    assert_eq!(result.current_streak, 0);
}

#[rocket::async_test]
async fn reaching_the_goal_completes_the_run_and_increments_streak() {
    let client = client().await;
    report(&client, 1, 60).await;
    let result = report(&client, 1, 60).await;
    assert_eq!(result.today_progress_xp, 120);
    assert!(result.goal_met_today);
    assert_eq!(result.current_streak, 1);
    assert_eq!(result.best_streak, 1);
}

#[rocket::async_test]
async fn extra_progress_after_goal_met_does_not_increment_streak_again() {
    let client = client().await;
    report(&client, 1, 100).await;
    let result = report(&client, 1, 50).await;
    assert_eq!(result.today_progress_xp, 150);
    assert_eq!(result.current_streak, 1);
}

#[rocket::async_test]
async fn players_track_independent_streaks() {
    let client = client().await;
    report(&client, 1, 100).await;
    let other = status(&client, 2).await;
    assert_eq!(other.current_streak, 0);
    assert_eq!(other.today_progress_xp, 0);
}
