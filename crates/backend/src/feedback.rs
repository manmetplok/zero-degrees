use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use shared::{
    AiFeature, AiFeedback, CreateAiFeedback, FeatureFeedbackSummary, FeedbackAggregate,
    FeedbackRating, FeedbackTrendPoint,
};
use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
struct FeedbackRow {
    id: i64,
    feature: String,
    message_id: Option<i64>,
    ai_output: String,
    final_value: Option<String>,
    rating: String,
    created_at: String,
}

#[derive(sqlx::FromRow)]
struct FeatureCountRow {
    feature: String,
    helpful: i64,
    unhelpful: i64,
}

#[derive(sqlx::FromRow)]
struct TrendCountRow {
    day: String,
    feature: String,
    helpful: i64,
    unhelpful: i64,
}

fn feature_str(feature: AiFeature) -> &'static str {
    match feature {
        AiFeature::Category => "category",
        AiFeature::Urgency => "urgency",
        AiFeature::Summary => "summary",
        AiFeature::DraftReply => "draft_reply",
    }
}

fn feature_from_str(s: &str) -> Option<AiFeature> {
    match s {
        "category" => Some(AiFeature::Category),
        "urgency" => Some(AiFeature::Urgency),
        "summary" => Some(AiFeature::Summary),
        "draft_reply" => Some(AiFeature::DraftReply),
        _ => None,
    }
}

fn rating_str(rating: FeedbackRating) -> &'static str {
    match rating {
        FeedbackRating::Helpful => "helpful",
        FeedbackRating::Unhelpful => "unhelpful",
    }
}

fn rating_from_str(s: &str) -> Option<FeedbackRating> {
    match s {
        "helpful" => Some(FeedbackRating::Helpful),
        "unhelpful" => Some(FeedbackRating::Unhelpful),
        _ => None,
    }
}

fn to_feedback(row: FeedbackRow) -> Result<AiFeedback, Status> {
    let feature = feature_from_str(&row.feature).ok_or(Status::InternalServerError)?;
    let rating = rating_from_str(&row.rating).ok_or(Status::InternalServerError)?;
    Ok(AiFeedback {
        id: row.id,
        feature,
        message_id: row.message_id,
        ai_output: row.ai_output,
        final_value: row.final_value,
        rating,
        created_at: row.created_at,
    })
}

fn helpful_ratio(helpful: i64, unhelpful: i64) -> f64 {
    let total = helpful + unhelpful;
    if total == 0 {
        0.0
    } else {
        helpful as f64 / total as f64
    }
}

#[post("/feedback", data = "<body>")]
pub async fn create(
    pool: &State<SqlitePool>,
    body: Json<CreateAiFeedback>,
) -> Result<status::Created<Json<AiFeedback>>, Status> {
    let row = sqlx::query_as::<_, FeedbackRow>(
        "INSERT INTO ai_feedback (feature, message_id, ai_output, final_value, rating) \
         VALUES (?, ?, ?, ?, ?) \
         RETURNING id, feature, message_id, ai_output, final_value, rating, created_at",
    )
    .bind(feature_str(body.feature))
    .bind(body.message_id)
    .bind(&body.ai_output)
    .bind(&body.final_value)
    .bind(rating_str(body.rating))
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    let feedback = to_feedback(row)?;
    let location = format!("/feedback/{}", feedback.id);
    Ok(status::Created::new(location).body(Json(feedback)))
}

#[get("/feedback/aggregate")]
pub async fn aggregate(pool: &State<SqlitePool>) -> Result<Json<FeedbackAggregate>, Status> {
    let feature_rows = sqlx::query_as::<_, FeatureCountRow>(
        "SELECT feature, \
                SUM(CASE WHEN rating = 'helpful' THEN 1 ELSE 0 END) AS helpful, \
                SUM(CASE WHEN rating = 'unhelpful' THEN 1 ELSE 0 END) AS unhelpful \
         FROM ai_feedback GROUP BY feature",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let by_feature = feature_rows
        .into_iter()
        .map(|row| {
            let feature = feature_from_str(&row.feature).ok_or(Status::InternalServerError)?;
            Ok(FeatureFeedbackSummary {
                feature,
                helpful: row.helpful,
                unhelpful: row.unhelpful,
                helpful_ratio: helpful_ratio(row.helpful, row.unhelpful),
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;

    let trend_rows = sqlx::query_as::<_, TrendCountRow>(
        "SELECT date(created_at) AS day, feature, \
                SUM(CASE WHEN rating = 'helpful' THEN 1 ELSE 0 END) AS helpful, \
                SUM(CASE WHEN rating = 'unhelpful' THEN 1 ELSE 0 END) AS unhelpful \
         FROM ai_feedback GROUP BY day, feature ORDER BY day",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let trend = trend_rows
        .into_iter()
        .map(|row| {
            let feature = feature_from_str(&row.feature).ok_or(Status::InternalServerError)?;
            Ok(FeedbackTrendPoint {
                date: row.day,
                feature,
                helpful: row.helpful,
                unhelpful: row.unhelpful,
                helpful_ratio: helpful_ratio(row.helpful, row.unhelpful),
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;

    Ok(Json(FeedbackAggregate { by_feature, trend }))
}

#[cfg(test)]
mod tests {
    use super::helpful_ratio;

    #[test]
    fn ratio_is_zero_when_no_feedback() {
        assert_eq!(helpful_ratio(0, 0), 0.0);
    }

    #[test]
    fn ratio_is_zero_when_all_unhelpful() {
        assert_eq!(helpful_ratio(0, 5), 0.0);
    }

    #[test]
    fn ratio_is_one_when_all_helpful() {
        assert_eq!(helpful_ratio(5, 0), 1.0);
    }

    #[test]
    fn ratio_is_the_helpful_share_of_total() {
        assert_eq!(helpful_ratio(3, 1), 0.75);
    }
}
