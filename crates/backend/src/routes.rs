use rocket::serde::json::Json;
use shared::HealthResponse;

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
    })
}
