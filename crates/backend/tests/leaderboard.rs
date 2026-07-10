use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{CreateTrackObject, LeaderboardResponse, ObjectLink};
use sqlx::SqlitePool;

async fn setup() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

async fn insert_player(pool: &SqlitePool, device_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>("INSERT INTO players (device_id) VALUES (?) RETURNING id")
        .bind(device_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_clear(
    pool: &SqlitePool,
    player_id: i64,
    xp: i64,
    response_time_seconds: Option<i64>,
    age_modifier: &str,
) {
    sqlx::query(
        "INSERT INTO clears (player_id, xp, response_time_seconds, cleared_at) \
         VALUES (?, ?, ?, datetime('now', ?))",
    )
    .bind(player_id)
    .bind(xp)
    .bind(response_time_seconds)
    .bind(age_modifier)
    .execute(pool)
    .await
    .unwrap();
}

async fn fetch_leaderboard(client: &Client, period: Option<&str>) -> LeaderboardResponse {
    let uri = match period {
        Some(p) => format!("/leaderboard?period={p}"),
        None => "/leaderboard".to_string(),
    };
    let response = client.get(uri).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn all_time_ranks_players_by_total_xp_descending() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "alice").await;
    let bob = insert_player(&pool, "bob").await;
    insert_clear(&pool, alice, 60, None, "-1 minutes").await;
    insert_clear(&pool, alice, 40, None, "-10 days").await;
    insert_clear(&pool, bob, 50, None, "-1 minutes").await;

    let board = fetch_leaderboard(&client, Some("all_time")).await;

    assert_eq!(board.entries.len(), 2);
    assert_eq!(board.entries[0].device_id, "alice");
    assert_eq!(board.entries[0].xp, 100);
    assert_eq!(board.entries[0].rank, 1);
    assert_eq!(board.entries[1].device_id, "bob");
    assert_eq!(board.entries[1].xp, 50);
    assert_eq!(board.entries[1].rank, 2);
}

#[rocket::async_test]
async fn today_period_only_counts_clears_from_today() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "alice").await;
    let bob = insert_player(&pool, "bob").await;
    insert_clear(&pool, alice, 20, None, "-1 minutes").await;
    insert_clear(&pool, alice, 80, None, "-10 days").await;
    insert_clear(&pool, bob, 30, None, "-1 minutes").await;

    let board = fetch_leaderboard(&client, Some("today")).await;

    let alice_entry = board.entries.iter().find(|e| e.device_id == "alice").unwrap();
    let bob_entry = board.entries.iter().find(|e| e.device_id == "bob").unwrap();
    assert_eq!(alice_entry.xp, 20);
    assert_eq!(bob_entry.xp, 30);
    assert_eq!(board.entries[0].device_id, "bob");
}

#[rocket::async_test]
async fn this_week_period_excludes_clears_older_than_seven_days() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "alice").await;
    insert_clear(&pool, alice, 40, None, "-3 days").await;
    insert_clear(&pool, alice, 999, None, "-10 days").await;

    let this_week = fetch_leaderboard(&client, Some("this_week")).await;
    let all_time = fetch_leaderboard(&client, Some("all_time")).await;

    assert_eq!(this_week.entries[0].xp, 40);
    assert_eq!(all_time.entries[0].xp, 1039);
}

#[rocket::async_test]
async fn team_totals_combine_all_players_and_incoming_volume() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "alice").await;
    let bob = insert_player(&pool, "bob").await;
    insert_clear(&pool, alice, 70, None, "-1 minutes").await;
    insert_clear(&pool, bob, 30, None, "-1 minutes").await;
    for i in 0..2 {
        client
            .post("/track/objects")
            .json(&CreateTrackObject {
                position: i as f64,
                link: ObjectLink::Generic {
                    url: format!("https://example.com/{i}"),
                },
            })
            .dispatch()
            .await;
    }

    let board = fetch_leaderboard(&client, None).await;

    assert_eq!(board.team.xp, 100);
    assert_eq!(board.team.clears, 2);
    assert_eq!(board.team.incoming_volume, 4);
}

#[rocket::async_test]
async fn players_without_clears_in_period_appear_with_zero_xp_and_null_response_time() {
    let (client, pool) = setup().await;
    insert_player(&pool, "alice").await;

    let board = fetch_leaderboard(&client, Some("today")).await;

    assert_eq!(board.entries.len(), 1);
    assert_eq!(board.entries[0].xp, 0);
    assert_eq!(board.entries[0].streak, 0);
    assert_eq!(board.entries[0].badge_count, 0);
    assert_eq!(board.entries[0].avg_response_seconds, None);
}

#[rocket::async_test]
async fn average_response_time_ignores_clears_with_no_recorded_time() {
    let (client, pool) = setup().await;
    let alice = insert_player(&pool, "alice").await;
    insert_clear(&pool, alice, 10, Some(30), "-1 minutes").await;
    insert_clear(&pool, alice, 10, Some(50), "-1 minutes").await;
    insert_clear(&pool, alice, 10, None, "-1 minutes").await;

    let board = fetch_leaderboard(&client, Some("all_time")).await;

    assert_eq!(board.entries[0].avg_response_seconds, Some(40.0));
}
