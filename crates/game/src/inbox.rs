//! Mock inbox: invented sample messages (the assignment forbids real data).
//! Later this module becomes the sync layer against the backend; the rest of
//! the game only sees `Message` values, so the swap stays local.

use shared::{Channel, Message, MessageStatus};

const SAMPLES: &[(&str, Channel, &str, &str)] = &[
    (
        "s.devries@example.com",
        Channel::Email,
        "Invoice #4821 charged twice",
        "Hi, I was billed twice for my March invoice. Can you refund one of the charges? Order reference 4821.",
    ),
    (
        "Contact form",
        Channel::WebForm,
        "Can't reset my password",
        "The reset link in the email keeps saying 'token expired' even when I click it right away.",
    ),
    (
        "TrustSpot review",
        Channel::Review,
        "★★☆☆☆ Slow delivery, great product",
        "Product itself is great, but it took three weeks to arrive and nobody answered my emails in between.",
    ),
    (
        "Ticket #7719",
        Channel::Ticket,
        "API returns 500 on bulk export",
        "Since yesterday our nightly export job fails with a 500. Nothing changed on our side. This blocks our reporting.",
    ),
    (
        "m.jansen@example.com",
        Channel::Email,
        "Change of address before next shipment",
        "We're moving offices on the 1st. Please make sure the next shipment goes to Keizersgracht 100, Amsterdam.",
    ),
    (
        "Contact form",
        Channel::WebForm,
        "Question about family subscription",
        "Do you offer a family plan? We'd need 4 seats. Couldn't find anything on the pricing page.",
    ),
    (
        "AppStore review",
        Channel::Review,
        "★★★★★ Support turned it around",
        "Had an issue with my order but support fixed it within a day. That's how you keep customers!",
    ),
    (
        "Ticket #7723",
        Channel::Ticket,
        "Urgent: account locked out, demo in 1 hour",
        "Our whole team is locked out after the SSO change and we have a customer demo at 15:00. Please help ASAP.",
    ),
    (
        "p.bakker@example.com",
        Channel::Email,
        "Cancel my subscription",
        "I'd like to cancel effective next month. Nothing wrong with the service, we just no longer need it.",
    ),
    (
        "Ticket #7724",
        Channel::Ticket,
        "Webhook retries flooding our endpoint",
        "Your webhook retry policy is hammering our server 50x per minute for one failed delivery. Can you cap it?",
    ),
];

/// Deterministic mock inbox for the demo: `count` open messages, oldest first.
pub fn sample_messages(count: usize) -> Vec<Message> {
    let base_ts: i64 = 1_780_000_000;
    (0..count)
        .map(|i| {
            let (sender, channel, subject, body) = SAMPLES[i % SAMPLES.len()];
            Message {
                id: i as u64 + 1,
                channel,
                sender: sender.to_string(),
                subject: subject.to_string(),
                body: body.to_string(),
                received_at: base_ts + (i as i64) * 540,
                status: MessageStatus::Open,
            }
        })
        .collect()
}

/// One extra message "arriving" mid-run, for the new-hurdle-drops-in scenario.
pub fn next_incoming(existing: &[Message]) -> Message {
    let id = existing.iter().map(|m| m.id).max().unwrap_or(0) + 1;
    let (sender, channel, subject, body) = SAMPLES[(id as usize) % SAMPLES.len()];
    Message {
        id,
        channel,
        sender: sender.to_string(),
        subject: subject.to_string(),
        body: body.to_string(),
        received_at: 1_780_000_000 + id as i64 * 540,
        status: MessageStatus::Open,
    }
}
