# API change request: hurdle metadata (stories 003, 004, 005, 009, 014)

The game currently mocks all AI enrichment locally in
`crates/game/src/meta.rs` (deterministic keyword classifier) and persists
category overrides in a local TSV. These are the backend endpoints and shared
type changes needed to replace the mocks. Written by the frontend; nothing
here is implemented server-side yet.

## Shared `Message` fields (crates/shared/src/lib.rs)

Add to `Message` (all filled at ingestion by the AI pipeline):

| Field | Type | Notes |
|---|---|---|
| `category` | `Category` enum: `billing` \| `complaint` \| `question` \| `feedback` | AI classification (story 003) |
| `category_override` | `Option<Category>` | Manual correction; wins over `category` when set (story 003) |
| `urgency` | `Urgency` enum: `critical` \| `high` \| `normal` \| `low` | Drives hurdle height, reward, response target (stories 004, 014) |
| `urgency_signal` | `Option<String>` | Short "why it matters" phrase shown on the detail card (story 004) |
| `sentiment` | `Sentiment` enum: `positive` \| `neutral` \| `negative` \| `angry` | Drives the mood aura (story 005) |
| `first_responded_at` | `Option<i64>` | Unix seconds; set when the message is cleared, for response-time stats (story 014) |

All enums serialize `snake_case`, matching the existing `Channel` convention.

## Endpoints

### POST /messages/{id}/enrich
Runs (or re-runs) AI classification for one message.
Needed by: stories 003, 004, 005 (and 009, which derives priority from the result).

- Request: empty body.
- Response `200`:

```json
{
  "message_id": 17,
  "category": "complaint",
  "urgency": "critical",
  "urgency_signal": "mentions 'service is down'",
  "sentiment": "angry"
}
```

Enrichment should normally happen automatically at ingestion; this endpoint
exists for backfills and manual re-runs. The game treats the values as data —
priority ordering (urgency + sentiment + age) is computed client-side.

### PUT /messages/{id}/category
Persists a manual category override from the detail card.
Needed by: story 003 (replaces the local TSV in `meta.rs::Overrides`).

- Request:

```json
{ "category": "billing" }
```

- Response `200`: the updated enrichment object (as above, with the override
  applied). `DELETE /messages/{id}/category` removes the override and falls
  back to the AI value.

### GET /settings/response-targets
### PUT /settings/response-targets
Configurable response-time target per urgency level, in seconds.
Needed by: story 014 (client defaults live in `Urgency::response_target()`).

- GET response / PUT request body:

```json
{ "critical": 900, "high": 3600, "normal": 14400, "low": 28800 }
```

### POST /messages/{id}/clear
Records a clear and its response time.
Needed by: story 014 (replaces the in-memory `meta.rs::ResponseLog`); race
control (story 011) reads the aggregate.

- Request:

```json
{ "cleared_at": 1780007200 }
```

- Response `200`:

```json
{
  "message_id": 17,
  "waited_secs": 5400,
  "on_time": false,
  "target_secs": 900
}
```

### GET /stats/response-times
Team statistics over cleared messages, for race control.
Needed by: stories 011 and 014.

- Response `200`:

```json
{
  "records": [
    {
      "message_id": 17,
      "category": "complaint",
      "urgency": "critical",
      "waited_secs": 5400,
      "on_time": false
    }
  ],
  "on_time_count": 12,
  "overdue_count": 3,
  "average_wait_secs": 2210
}
```
