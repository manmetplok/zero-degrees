use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{AiFeature, CreateAiFeedback, FeedbackAggregate, FeedbackRating};
use sqlx::SqlitePool;

async fn client() -> (Client, SqlitePool) {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool.clone())).await.unwrap();
    (client, pool)
}

async fn submit_feedback(
    client: &Client,
    feature: AiFeature,
    rating: FeedbackRating,
    final_value: Option<&str>,
) {
    let response = client
        .post("/feedback")
        .json(&CreateAiFeedback {
            feature,
            message_id: Some(1),
            ai_output: "draft text".into(),
            final_value: final_value.map(String::from),
            rating,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
}

#[rocket::async_test]
async fn create_persists_ai_output_and_final_value_together() {
    let (client, _pool) = client().await;
    let response = client
        .post("/feedback")
        .json(&CreateAiFeedback {
            feature: AiFeature::DraftReply,
            message_id: Some(42),
            ai_output: "Sorry for the trouble, we'll look into it.".into(),
            final_value: Some("Sorry for the trouble, we'll fix it today.".into()),
            rating: FeedbackRating::Unhelpful,
        })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let feedback: shared::AiFeedback = response.into_json().await.unwrap();
    assert!(feedback.id > 0);
    assert_eq!(feedback.feature, AiFeature::DraftReply);
    assert_eq!(feedback.message_id, Some(42));
    assert_eq!(
        feedback.ai_output,
        "Sorry for the trouble, we'll look into it."
    );
    assert_eq!(
        feedback.final_value,
        Some("Sorry for the trouble, we'll fix it today.".to_string())
    );
    assert_eq!(feedback.rating, FeedbackRating::Unhelpful);
}

#[rocket::async_test]
async fn aggregate_computes_helpful_ratio_per_feature() {
    let (client, _pool) = client().await;
    submit_feedback(&client, AiFeature::Category, FeedbackRating::Helpful, None).await;
    submit_feedback(&client, AiFeature::Category, FeedbackRating::Helpful, None).await;
    submit_feedback(&client, AiFeature::Category, FeedbackRating::Unhelpful, None).await;
    submit_feedback(
        &client,
        AiFeature::DraftReply,
        FeedbackRating::Unhelpful,
        Some("rewritten reply"),
    )
    .await;

    let response = client.get("/feedback/aggregate").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let aggregate: FeedbackAggregate = response.into_json().await.unwrap();

    let category = aggregate
        .by_feature
        .iter()
        .find(|f| f.feature == AiFeature::Category)
        .unwrap();
    assert_eq!(category.helpful, 2);
    assert_eq!(category.unhelpful, 1);
    assert!((category.helpful_ratio - (2.0 / 3.0)).abs() < 1e-9);

    let draft_reply = aggregate
        .by_feature
        .iter()
        .find(|f| f.feature == AiFeature::DraftReply)
        .unwrap();
    assert_eq!(draft_reply.helpful, 0);
    assert_eq!(draft_reply.unhelpful, 1);
    assert_eq!(draft_reply.helpful_ratio, 0.0);
}

#[rocket::async_test]
async fn aggregate_buckets_trend_by_day_per_feature() {
    let (client, pool) = client().await;
    sqlx::query(
        "INSERT INTO ai_feedback (feature, ai_output, rating, created_at) \
         VALUES ('summary', 'a summary', 'helpful', '2026-07-08 09:00:00')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai_feedback (feature, ai_output, rating, created_at) \
         VALUES ('summary', 'another summary', 'unhelpful', '2026-07-09 09:00:00')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = client.get("/feedback/aggregate").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let aggregate: FeedbackAggregate = response.into_json().await.unwrap();

    let summary_points: Vec<_> = aggregate
        .trend
        .iter()
        .filter(|p| p.feature == AiFeature::Summary)
        .collect();
    assert_eq!(summary_points.len(), 2);

    let day_one = summary_points
        .iter()
        .find(|p| p.date == "2026-07-08")
        .unwrap();
    assert_eq!(day_one.helpful, 1);
    assert_eq!(day_one.unhelpful, 0);

    let day_two = summary_points
        .iter()
        .find(|p| p.date == "2026-07-09")
        .unwrap();
    assert_eq!(day_two.helpful, 0);
    assert_eq!(day_two.unhelpful, 1);
}
