use rocket::http::Status;
use rocket::local::asynchronous::Client;
use shared::MessageSearchResult;
use sqlx::SqlitePool;

struct SeedMessage {
    channel: &'static str,
    sender: &'static str,
    subject: &'static str,
    body: &'static str,
    received_at: i64,
    status: &'static str,
    category: Option<&'static str>,
    sentiment: Option<&'static str>,
    urgency: Option<&'static str>,
    summary: Option<&'static str>,
}

async fn seed(pool: &SqlitePool, messages: &[SeedMessage]) {
    for m in messages {
        sqlx::query(
            "INSERT INTO messages \
             (channel, sender, subject, body, received_at, status, ai_category, sentiment, urgency, summary) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(m.channel)
        .bind(m.sender)
        .bind(m.subject)
        .bind(m.body)
        .bind(m.received_at)
        .bind(m.status)
        .bind(m.category.unwrap_or("question"))
        .bind(m.sentiment.unwrap_or("neutral"))
        .bind(m.urgency.unwrap_or("normal"))
        .bind(m.summary)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn client_with(messages: &[SeedMessage]) -> Client {
    let pool = backend::db::connect("sqlite::memory:").await;
    seed(&pool, messages).await;
    Client::tracked(backend::rocket(pool)).await.unwrap()
}

async fn search(client: &Client, query: &str) -> Vec<MessageSearchResult> {
    let response = client.get(format!("/messages/search{}", query)).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.unwrap()
}

#[rocket::async_test]
async fn combined_filters_narrow_to_matching_channel_category_sentiment_urgency_and_status() {
    let client = client_with(&[
        SeedMessage {
            channel: "email",
            sender: "a@example.com",
            subject: "Invoice question",
            body: "Why was I charged twice?",
            received_at: 100,
            status: "open",
            category: Some("billing"),
            sentiment: Some("angry"),
            urgency: Some("high"),
            summary: Some("Customer disputes a double charge."),
        },
        SeedMessage {
            channel: "email",
            sender: "b@example.com",
            subject: "Invoice question",
            body: "Why was I charged twice?",
            received_at: 200,
            status: "cleared",
            category: Some("billing"),
            sentiment: Some("angry"),
            urgency: Some("high"),
            summary: None,
        },
        SeedMessage {
            channel: "ticket",
            sender: "c@example.com",
            subject: "Invoice question",
            body: "Why was I charged twice?",
            received_at: 300,
            status: "open",
            category: Some("billing"),
            sentiment: Some("angry"),
            urgency: Some("high"),
            summary: None,
        },
        SeedMessage {
            channel: "email",
            sender: "d@example.com",
            subject: "Feature request",
            body: "Could you add dark mode?",
            received_at: 400,
            status: "open",
            category: Some("feedback"),
            sentiment: Some("neutral"),
            urgency: Some("low"),
            summary: None,
        },
    ])
    .await;

    let results = search(
        &client,
        "?channel=email&category=billing&sentiment=angry&urgency=high&status=open",
    )
    .await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].sender, "a@example.com");
    assert_eq!(results[0].summary.as_deref(), Some("Customer disputes a double charge."));
}

#[rocket::async_test]
async fn text_search_matches_sender_or_body_and_ranks_subject_and_sender_hits_above_body_only() {
    let client = client_with(&[
        SeedMessage {
            channel: "email",
            sender: "Keizersgracht Support",
            subject: "General question",
            body: "Just checking in on my order.",
            received_at: 100,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
        SeedMessage {
            channel: "ticket",
            sender: "d.devries@example.com",
            subject: "Delivery delay",
            body: "Nothing to add, just following up.",
            received_at: 200,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
        SeedMessage {
            channel: "review",
            sender: "AppStore review",
            subject: "Slow delivery",
            body: "Support was great once someone from Keizersgracht answered.",
            received_at: 300,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
        SeedMessage {
            channel: "web_form",
            sender: "Contact form",
            subject: "Password reset",
            body: "The reset link is broken.",
            received_at: 400,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
    ])
    .await;

    let results = search(&client, "?q=Keizersgracht").await;

    let senders: Vec<&str> = results.iter().map(|r| r.sender.as_str()).collect();
    assert_eq!(senders, vec!["Keizersgracht Support", "AppStore review"]);
}

#[rocket::async_test]
async fn no_query_or_filters_returns_everything_ordered_by_recency() {
    let client = client_with(&[
        SeedMessage {
            channel: "email",
            sender: "a@example.com",
            subject: "Oldest",
            body: "First in.",
            received_at: 100,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
        SeedMessage {
            channel: "ticket",
            sender: "b@example.com",
            subject: "Newest",
            body: "Just arrived.",
            received_at: 300,
            status: "cleared",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
        SeedMessage {
            channel: "review",
            sender: "c@example.com",
            subject: "Middle",
            body: "Somewhere in between.",
            received_at: 200,
            status: "open",
            category: None,
            sentiment: None,
            urgency: None,
            summary: None,
        },
    ])
    .await;

    let results = search(&client, "").await;

    let subjects: Vec<&str> = results.iter().map(|r| r.subject.as_str()).collect();
    assert_eq!(subjects, vec!["Newest", "Middle", "Oldest"]);
}

#[rocket::async_test]
async fn unknown_filter_value_is_rejected() {
    let client = client_with(&[]).await;

    let response = client.get("/messages/search?urgency=extreme").dispatch().await;

    assert_eq!(response.status(), Status::UnprocessableEntity);
}
