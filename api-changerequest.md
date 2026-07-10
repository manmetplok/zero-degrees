# API change request — frontend needs beyond the current backend

The backend today exposes `GET /health` and `POST/GET/DELETE /track/objects`
(shared types in `crates/shared/src/lib.rs`). All twenty user stories are now
implemented in `crates/game` against local, deterministic mocks; this file is
the consolidated request for the real endpoints that replace them. Full JSON
shapes and per-story rationale live in the detailed files under
`api-changerequests/` — this page is the overview and the deduped common
ground.

## Cross-cutting: message ingestion + AI enrichment (most stories)

Every cluster independently needs messages to arrive from the backend carrying
AI-derived triage data. Requested once, used everywhere:

- **`GET /messages`** (and a mid-run delta mechanism) — the real inbox; the
  game currently generates its course locally (`inbox.rs`).
- **Enrichment fields on the shared `Message` type**: `category`
  (billing/complaint/question/feedback), `category_override`, `urgency`
  (critical/high/normal/low) + `urgency_signal` ("why it matters"), `sentiment`
  (positive/neutral/negative/angry), `first_responded_at`. All snake_case,
  matching the existing `Channel` convention. Mocked today in
  `crates/game/src/meta.rs::enrich`.
- **`POST /messages/{id}/enrich`** — (re)run classification for one message;
  also returns the 1–2 sentence AI **summary** (scout report, story 006;
  mocked in `meta::summarize`, skipped client-side for bodies ≤ 120 chars).
- **`PUT /messages/{id}/category`** — persist a manual category override
  (story 003; currently a local TSV).

Details: `api-changerequests/hurdle-metadata.md`, `api-changerequests/card-flow.md`.

## Reply flow (stories 007, 008)

- **`POST /messages/{id}/draft-reply`** — AI draft matching message content,
  Meridian tone, and the original language; accepts a steering note and a
  variant/recharge counter. Mocked in `crates/game/src/reply.rs`.
- **`POST /messages/{id}/reply`** — send the player-approved text and mark the
  message resolved; the game only clears the hurdle on a 2xx.

Details: `api-changerequests/card-flow.md`.

## AI coach feedback (story 020)

- **`POST /feedback`** — already added by the backend (shared types
  `AiFeature`, `FeedbackRating` exist); the game's `feedback.rs` mirrors it.
- Requested additions: **`POST /feedback/batch`** and an aggregate
  **`GET /feedback/stats`** (helpful/unhelpful ratio per AI feature, trend
  over time) for race control.

Details: `api-changerequests/card-flow.md`.

## Course generation, hazards, search (stories 002, 010, 012)

- **`POST /course/generate`** — AI-generated demo course (≥50 messages, mixed
  channels/tones/languages), with `difficulty` preset
  (chill_jog/normal_shift/nightmare_monday) and `reset` flag. Mocked by the
  seeded generator in `inbox.rs` (`ZD_SEED`/`ZD_COURSE`/`ZD_COUNT`).
- **`GET /hazards`** — AI theme clustering over open/recent messages with a
  written zone briefing per cluster. Mocked by keyword clustering in
  `hazards.rs`.
- **`GET /messages/search`** — server-side free-text search + combinable
  filters (channel, category, sentiment, urgency, status). Client-side today
  in `filter.rs`.

Details: `api-changerequests/course-search.md`.

## Player progression (stories 015, 016, 018)

- **`GET /players/{id}/profile`** — XP, streak, best streak, shield, trophies.
- **`POST /players/{id}/clears`** — batched clear events (mirrors the game's
  `ClearEvent`: urgency, sentiment, was_burning, response_seconds,
  track_cleared, at); server derives daily-goal state, streak transitions, and
  trophy awards from the same events and returns celebration events.
- **`GET /players/{id}/daily`**, **`GET /players/{id}/streak`**,
  **`GET /players/{id}/trophies`** — read models for the HUD, flame pill, and
  trophy room. All persisted locally today (`save.rs`).

Details: `api-changerequests/progression.md`.

## Team features (stories 011, 013, 017, 019) — most backend-hungry

- **`GET /team/runners`** — roster with names/avatars/colors (3 mock
  teammates simulated in `team.rs` today).
- **`PUT /messages/{id}/assignee`** / **`DELETE /messages/{id}/assignee`** —
  baton passing with a 409 on claim conflicts (exactly-one-owner rule).
- **`GET /team/stats`** — race-control aggregates (open/cleared, per-channel,
  per-category, mood breakdown, burning count, per-runner progress).
- **`GET /leaderboard?period=today|week|all`** — XP ranking with streaks,
  badges, avg response time, and the team total.
- **`POST /events/clear`** — attribute a clear to a runner (feeds boss hits
  and the leaderboard).
- **`GET /team/boss`** — backlog boss state (weighted-priority HP, enrage).
- **`GET /team/events?since={cursor}`** — cursor polling for near-real-time
  updates; WebSocket explicitly deferred per ARCHITECTURE.md.

Details: `api-changerequests/team-screens.md`.

## Response-time configuration (story 014)

- Per-urgency response targets are client constants (15m/1h/4h/8h) —
  requested as team settings served by the backend, e.g.
  **`GET /team/settings`**.

Details: `api-changerequests/hurdle-metadata.md`.
