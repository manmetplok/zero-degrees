# API change request: hurdle card flow (stories 004/006/007/008/020)

From: game frontend (hurdle detail card / reply loop).
The game currently mocks all of this client-side behind small functions
(`crates/game/src/meta.rs` enrichment, `crates/game/src/reply.rs` draft
generation, simulated send in `crates/game/src/card.rs`), each designed to be
swapped for one HTTP call. Endpoints below are what those swaps need.

Existing endpoints we already build against (no change requested):

- `POST /feedback` and `GET /feedback/aggregate` — story 020. The game keeps
  an in-memory mirror (`crates/game/src/feedback.rs`) using the same
  `shared::CreateAiFeedback` / `shared::FeedbackAggregate` types and will
  sync ratings through these once the game gets an HTTP client.
- `GET /track/messages`, `PATCH /messages/<id>/category` — messages + typing.

## 1. Message enrichment — stories 004 (hurdle height) + 006 (scout report)

`GET /messages/<id>/enrichment`

The AI-derived read of one message: urgency for hurdle height (with the
honest "why", shown on the detail card), and the one-to-two sentence scout
report summary. Server should apply the short-message rule from story 006:
when the body is under the summary threshold, return the body verbatim as
`summary` and make no model call (`summary_is_verbatim: true`).

Response `200`:

```json
{
  "message_id": 42,
  "urgency": "critical",            // "low" | "normal" | "high" | "critical"
  "urgency_reason": "Time-critical: the customer is blocked right now.",
  "sentiment": "negative",          // "positive" | "neutral" | "negative" | "angry"
  "summary": "Team locked out after SSO change; customer demo at 15:00.",
  "summary_is_verbatim": false
}
```

Nice to have: include this object inline on `GET /track/messages` items so
the track can be built with heights in one request.

## 2. Draft reply generation — story 007 (reply power-up)

`POST /messages/<id>/draft-reply`

Drafts a reply that addresses the message content, matches Meridian's tone
of voice, and is written in the language of the original message. Called on
the first power-up (no note) and on every "recharge" (with the player's
steering note; `variant` counts recharges so identical requests can still
return a fresh take).

Request:

```json
{
  "steering_note": "keep it short, offer a refund",  // or null on first charge
  "variant": 2
}
```

Response `200`:

```json
{
  "draft": "Hi there,\n\nThanks for reaching out about ...",
  "language": "en"                  // BCP-47-ish tag of the original message
}
```

## 3. Send reply / resolve — story 008 (taking the hurdle)

`POST /messages/<id>/reply`

The one real action in the loop: the player swiped up on a reviewed draft.
Sends `body` to the customer over the message's channel, marks the message
cleared, and is the moment the game plays the jump and awards score. Must be
explicit-approval only — the game never calls this without a swipe-up.

Request:

```json
{ "body": "Hi there,\n\nThanks for ... (the player's final edited text)" }
```

Response `200` (the updated message, so the client can reconcile state):

```json
{
  "id": 42,
  "status": "cleared",
  "sent_at": "2026-07-10T14:52:57Z"
}
```

Errors: `409` if the message is not open (already cleared elsewhere) — the
game then refreshes the track instead of jumping.

## 4. AI feedback — story 020 (coach feedback)

Covered by the existing `POST /feedback` / `GET /feedback/aggregate`. One
small ask: accept a batch, `POST /feedback/batch` with a JSON array of
`CreateAiFeedback`, so the game can flush its offline in-memory store in one
request when connectivity returns. Not blocking.
