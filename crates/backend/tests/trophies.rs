use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{RecordClear, RecordDayEnd, Trophy, TrophyKind, TrophyProgress, TrophyTier};
use sqlx::SqlitePool;

async fn client_with_player() -> (Client, i64) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let player_id = insert_player(&pool, "device-1").await;
    let client = Client::tracked(backend::rocket(pool)).await.unwrap();
    (client, player_id)
}

async fn insert_player(pool: &SqlitePool, device_id: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO players (device_id) VALUES (?) RETURNING id")
        .bind(device_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn fast_clear() -> RecordClear {
    RecordClear {
        duration_seconds: 60,
        was_burning: false,
        is_angry_aura: false,
        is_critical: false,
    }
}

async fn record_clear(client: &Client, player_id: i64, body: &RecordClear) -> Vec<Trophy> {
    let response = client
        .post(format!("/players/{player_id}/trophies/clears"))
        .json(body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

async fn earned_trophies(client: &Client, player_id: i64) -> Vec<Trophy> {
    client
        .get(format!("/players/{player_id}/trophies"))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap()
}

#[rocket::async_test]
async fn speed_demon_awarded_once_on_the_tenth_fast_clear() {
    let (client, player_id) = client_with_player().await;
    for _ in 0..9 {
        let awarded = record_clear(&client, player_id, &fast_clear()).await;
        assert!(awarded.is_empty());
    }
    let awarded = record_clear(&client, player_id, &fast_clear()).await;
    assert_eq!(awarded.len(), 1);
    assert_eq!(awarded[0].kind, TrophyKind::SpeedDemon);
    assert_eq!(awarded[0].tier, TrophyTier::Bronze);

    let trophies = earned_trophies(&client, player_id).await;
    assert_eq!(trophies.len(), 1);
}

#[rocket::async_test]
async fn no_duplicate_award_when_condition_is_met_again() {
    let (client, player_id) = client_with_player().await;
    for _ in 0..10 {
        record_clear(&client, player_id, &fast_clear()).await;
    }
    let awarded = record_clear(&client, player_id, &fast_clear()).await;
    assert!(awarded.is_empty());

    let trophies = earned_trophies(&client, player_id).await;
    assert_eq!(trophies.len(), 1);
}

#[rocket::async_test]
async fn tier_upgrades_from_bronze_to_silver_to_gold() {
    let (client, player_id) = client_with_player().await;
    let burning_clear = RecordClear {
        duration_seconds: 600,
        was_burning: true,
        is_angry_aura: false,
        is_critical: false,
    };

    for _ in 0..5 {
        record_clear(&client, player_id, &burning_clear).await;
    }
    let trophies = earned_trophies(&client, player_id).await;
    assert_eq!(trophies[0].tier, TrophyTier::Bronze);

    for _ in 0..10 {
        record_clear(&client, player_id, &burning_clear).await;
    }
    let trophies = earned_trophies(&client, player_id).await;
    assert_eq!(trophies.len(), 1);
    assert_eq!(trophies[0].tier, TrophyTier::Silver);

    for _ in 0..35 {
        record_clear(&client, player_id, &burning_clear).await;
    }
    let trophies = earned_trophies(&client, player_id).await;
    assert_eq!(trophies.len(), 1);
    assert_eq!(trophies[0].tier, TrophyTier::Gold);
}

#[rocket::async_test]
async fn clean_sweep_awarded_only_for_empty_track_day_ends() {
    let (client, player_id) = client_with_player().await;
    let response = client
        .post(format!("/players/{player_id}/trophies/day-end"))
        .json(&RecordDayEnd { track_empty: false })
        .dispatch()
        .await;
    let awarded: Vec<Trophy> = response.into_json().await.unwrap();
    assert!(awarded.is_empty());

    let response = client
        .post(format!("/players/{player_id}/trophies/day-end"))
        .json(&RecordDayEnd { track_empty: true })
        .dispatch()
        .await;
    let awarded: Vec<Trophy> = response.into_json().await.unwrap();
    assert_eq!(awarded.len(), 1);
    assert_eq!(awarded[0].kind, TrophyKind::CleanSweep);
}

#[rocket::async_test]
async fn progress_reports_count_and_next_tier_before_any_trophy_is_earned() {
    let (client, player_id) = client_with_player().await;
    record_clear(&client, player_id, &fast_clear()).await;

    let response = client
        .get(format!("/players/{player_id}/trophies/progress"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let progress: Vec<TrophyProgress> = response.into_json().await.unwrap();

    let speed_demon = progress
        .iter()
        .find(|p| p.kind == TrophyKind::SpeedDemon)
        .unwrap();
    assert_eq!(speed_demon.tier, None);
    assert_eq!(speed_demon.count, 1);
    assert_eq!(speed_demon.next_tier, Some(TrophyTier::Bronze));
    assert_eq!(speed_demon.next_threshold, Some(10));
}

#[rocket::async_test]
async fn unknown_player_returns_not_found() {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool)).await.unwrap();

    let response = client.get("/players/999/trophies").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);

    let response = client
        .post("/players/999/trophies/clears")
        .json(&fast_clear())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);
}
