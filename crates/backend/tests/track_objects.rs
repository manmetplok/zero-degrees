use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::{CreateTrackObject, ObjectLink, TrackObject};

async fn client() -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn create_object(client: &Client, position: f64, link: ObjectLink) -> TrackObject {
    let response = client
        .post("/track/objects")
        .json(&CreateTrackObject { position, link })
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn create_returns_object_with_id_and_link() {
    let client = client().await;
    let link = ObjectLink::Ticket { key: "ZD-42".into() };
    let object = create_object(&client, 12.5, link.clone()).await;
    assert!(object.id > 0);
    assert_eq!(object.position, 12.5);
    assert_eq!(object.link, link);
}

#[rocket::async_test]
async fn list_roundtrips_every_link_kind() {
    let client = client().await;
    let links = vec![
        ObjectLink::Ticket { key: "ZD-1".into() },
        ObjectLink::Email {
            message_id: "msg-abc@example.com".into(),
        },
        ObjectLink::Review {
            review_id: "rev-7".into(),
        },
        ObjectLink::Generic {
            url: "https://example.com/thing".into(),
        },
    ];
    for (i, link) in links.iter().enumerate() {
        create_object(&client, i as f64, link.clone()).await;
    }
    let response = client.get("/track/objects").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let objects: Vec<TrackObject> = response.into_json().await.unwrap();
    let listed: Vec<ObjectLink> = objects.into_iter().map(|o| o.link).collect();
    assert_eq!(listed, links);
}

#[rocket::async_test]
async fn delete_removes_object() {
    let client = client().await;
    let object = create_object(
        &client,
        0.0,
        ObjectLink::Generic {
            url: "https://example.com".into(),
        },
    )
    .await;
    let response = client
        .delete(format!("/track/objects/{}", object.id))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NoContent);
    let objects: Vec<TrackObject> = client
        .get("/track/objects")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert!(objects.is_empty());
}

#[rocket::async_test]
async fn delete_unknown_id_returns_not_found() {
    let client = client().await;
    let response = client.delete("/track/objects/999").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}
