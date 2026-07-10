use rocket::http::Status;
use rocket::local::asynchronous::Client;

#[rocket::async_test]
async fn health_returns_ok() {
    let pool = backend::db::connect("sqlite::memory:").await;
    let client = Client::tracked(backend::rocket(pool)).await.unwrap();
    let response = client.get("/health").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: shared::HealthResponse = response.into_json().await.unwrap();
    assert_eq!(body.status, "ok");
}
