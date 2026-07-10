# API change request: team screens (stories 011, 013, 017, 019)

From: game frontend (team screens). The game currently mocks the whole team
behind `crates/game/src/team.rs`; these endpoints replace that mock. All
shapes should land as types in `crates/shared` so both ends compile against
the same definitions.

Conventions used below: `runner_id` is a stable string id for a team member
(`"me"` is resolved from auth on the server side; the examples use plain
ids). Timestamps are unix seconds, matching `Message.received_at`.

## 1. GET /team/runners — team roster

Needed by: 011 (per-runner progress), 013 (assignee picker/avatars),
017 (league rows), 019 (hit attribution).

Response 200:

```json
{
  "runners": [
    {
      "id": "u_sana",
      "name": "Sana",
      "color": "#ff8c59",
      "streak_days": 6,
      "badges": 9
    }
  ]
}
```

## 2. PUT /messages/{id}/assignee — pass the baton

Needed by: 013 (assign, reassign, claim).

Request:

```json
{ "runner_id": "u_kim", "expected_version": 3 }
```

- `expected_version` (optional) makes a *claim* safe: the server rejects the
  write if the assignment changed since the client last saw it, so two
  runners can never both claim the same hurdle.

Response 200:

```json
{ "message_id": 4821, "runner_id": "u_kim", "previous": "u_sana", "version": 4 }
```

Response 409 when `expected_version` is stale (someone else claimed it):

```json
{ "error": "assignment_conflict", "runner_id": "u_diego", "version": 4 }
```

## 3. DELETE /messages/{id}/assignee — release the baton

Needed by: 013. Response 200 with the same shape as PUT (`runner_id: null`).

Assignee should also be included on every message the game syncs (add
`assignee_id: string | null` and `assignment_version` to the shared message
payload), so "my lane" is a client-side filter and needs no extra endpoint.

## 4. GET /team/stats — race control aggregates

Needed by: 011. One call for the dashboard; server computes from the live
queue so the lead's view matches ingestion.

Query: none (server scopes to the caller's team).

Response 200:

```json
{
  "open": 14,
  "cleared_today": 23,
  "per_channel": { "email": 5, "web_form": 3, "review": 2, "ticket": 4 },
  "per_category": { "billing": 4, "account": 3, "technical": 3, "shipping": 1, "feedback": 2, "other": 1 },
  "per_mood": { "positive": 2, "neutral": 5, "negative": 7 },
  "burning": 3,
  "hazard_zones": 1,
  "runners": [
    { "runner_id": "u_sana", "cleared_today": 9, "open_assigned": 2, "last_clear_at": 1783600000 }
  ]
}
```

Category/mood/urgency should be the backend's enrichment (the client
currently stands them in with keyword rules in `dashboard.rs`; when the
backend owns enrichment, ship the fields on each message instead and the
client can drop its classifier).

## 5. GET /leaderboard?period=today|week|all — league table

Needed by: 017. XP must be computed server-side per period so everyone sees
the same table.

Response 200:

```json
{
  "period": "week",
  "team": { "xp": 12840, "cleared": 41, "incoming": 55 },
  "rows": [
    {
      "runner_id": "u_kim",
      "xp": 4100,
      "streak_days": 12,
      "badges": 14,
      "avg_response_secs": 540
    }
  ]
}
```

`team` powers the team-total banner (team over individual). Requires the
backend to record an XP event per clear:

## 6. POST /events/clear — report a clear

Needed by: 017 (XP over periods), 019 (attributed boss hits), 011 (live
per-runner progress). The client sends one event per resolved hurdle:

```json
{ "message_id": 4821, "cleared_at": 1783600123, "xp": 150, "weight": 3 }
```

Response 201: `{ "ok": true }`. `weight` is the hurdle's priority weight at
clear time (urgency + burning), used for boss damage attribution.

## 7. GET /team/boss — backlog boss state

Needed by: 019. The boss is derived state; the server owns it so the whole
team fights the same monster and history survives client restarts.

Response 200:

```json
{
  "phase": "alive",
  "hp": 31,
  "max_hp": 44,
  "enraged": false,
  "burning": 2,
  "bosses_defeated": 3,
  "current_damage": [ { "runner_id": "u_sana", "damage": 9 } ],
  "last_battle": [ { "runner_id": "u_sana", "damage": 17 } ],
  "total_damage": [ { "runner_id": "u_sana", "damage": 120 } ]
}
```

`phase` is `"alive" | "victory" | "dormant"`. (Weighting/enrage rules are in
`crates/game/src/boss.rs` today; server should adopt the same: hp = sum of
urgency(1..3) + burning bonus over open messages, enrage at >= 4 burning.)

## 8. Live updates — polling first

Needed by: all four stories ("update live as hurdles are cleared", "boss
grows on arrivals", "near-real-time" race control).

Per ARCHITECTURE.md ("WebSocket only if realtime features appear"), start
with cursor polling; the demo cadence (2-5 s) is fine over REST:

### GET /team/events?since={cursor}

Response 200:

```json
{
  "cursor": 812,
  "events": [
    { "seq": 810, "type": "message_arrived", "message": { "id": 4901, "channel": "email", "...": "..." } },
    { "seq": 811, "type": "message_cleared", "message_id": 4821, "runner_id": "u_kim", "xp": 150, "weight": 3 },
    { "seq": 812, "type": "assignment_changed", "message_id": 4890, "runner_id": "u_me", "previous": null }
  ]
}
```

`assignment_changed` events targeting the caller drive the "Baton incoming!"
toast. If polling ever feels too slow on the race-control big screen, the
same event payloads can move to a WebSocket (`GET /team/events/ws`) without
reshaping the client — but that is explicitly a later step.
