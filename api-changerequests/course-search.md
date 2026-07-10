# API change request: course generation, hazard zones, message search

Requested by the game client for stories 002 (course generator), 010 (hazard
zones), and 012 (binoculars search). The game currently mocks all three with
deterministic stand-ins (`inbox::generate_course`, `hazards::detect_zones`,
`filter::search`); these endpoints replace the mocks one for one. Message
objects below are `shared::Message` (id, channel, sender, subject, body,
received_at, status) unless noted.

## 1. POST /course/generate — AI demo course (story 002)

Generates a varied demo course of sample messages with an AI model. Must
never contain real customer data. `reset: true` clears the previous demo
course before laying the new one; `seed` makes generation reproducible for
live demos.

Request:

```json
{
  "seed": 11,
  "difficulty": "chill_jog" | "normal_shift" | "nightmare_monday",
  "count": 50,
  "reset": true
}
```

`difficulty` shifts the urgency/sentiment mix (chill: calm and friendly;
nightmare: many urgent/angry messages plus a same-root-cause spike). `count`
defaults to 50 (the story minimum). Response `201`:

```json
{
  "course_id": 7,
  "messages": [ { "id": 1, "channel": "email", "sender": "s.devries@example.com",
                  "subject": "…", "body": "…", "received_at": 1780000699,
                  "status": "open" } ]
}
```

Mixed channels, topics, tones, urgency, and languages (including some Dutch)
are the generator's responsibility server-side.

## 2. GET /hazards — AI theme clustering with zone briefings (story 010)

Periodically re-clusters open and recent messages by underlying root cause.
The client renders each cluster as a named hazard zone banner and shows the
briefing when the zone is tapped.

Request: `GET /hazards?since=1780000000` (`since` optional: only consider
messages received after this timestamp).

Response `200`:

```json
{
  "zones": [
    {
      "id": 3,
      "title": "Checkout failures",
      "briefing": "The payment step at checkout is failing; 14 reports in 23 minutes point at the payment provider. Peak urgency critical.",
      "message_ids": [12, 13, 14, 15, 18],
      "open_count": 14,
      "first_seen": 1780023762,
      "last_seen": 1780024009
    }
  ]
}
```

`title` and `briefing` are AI-written; `briefing` summarizes the common root
cause for whoever takes the zone on. Clustering must be stable enough that
zone identity survives a refresh (same `id` while the underlying spike is
ongoing), otherwise the client's selected-zone state breaks.

## 3. GET /messages/search — server-side search and filters (story 012)

Free-text search over content and sender, combinable with the binocular
filters. The client currently searches its local course; once messages live
server-side this endpoint takes over.

Request:

```
GET /messages/search?q=refund
    &channel=email,ticket
    &category=billing
    &sentiment=negative,angry
    &urgency=high,critical
    &status=open
    &limit=20
```

All parameters optional and ANDed together; list-valued parameters are
comma-separated ORs within the dimension. `q` terms must all match somewhere
in sender, subject, or body; ranking is relevance (sender > subject > body
hits), tie-broken by recency — same contract as the client-side mock.

Response `200`:

```json
{
  "total": 3,
  "results": [
    {
      "message": { "id": 16, "channel": "email", "sender": "s.jansen@example.com",
                   "subject": "Invoice #4752 charged twice", "body": "…",
                   "received_at": 1780010667, "status": "open" },
      "category": "billing",
      "sentiment": "negative",
      "urgency": "normal",
      "summary": "Invoice #4752 charged twice: I was billed twice for invoice #4752 this month."
    }
  ]
}
```

`category`/`sentiment`/`urgency`/`summary` are the AI enrichment values the
result list shows as scout reports (kept identical to `meta::enrich`'s
shape so the client can render either source).
