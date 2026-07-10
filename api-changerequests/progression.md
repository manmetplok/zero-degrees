# API change request: player progression (stories 015/016/018)

Requested by the game client for daily runs & streaks (story 016) and the
trophy room (story 018). Today all progression state lives in a local save
file (`crates/game/src/save.rs`); these endpoints let it survive device
switches and feed the league/leaderboard views later. The client remains
offline-first: it plays against the local save and syncs when it can.

Auth note: routes assume some player identity (anonymous device ID is fine,
see ARCHITECTURE.md open items). `{player_id}` below is whatever that
resolves to.

## 1. Player profile

**Story:** 016 + 018 (profile screen header), 017 later (league entry).

`GET /players/{player_id}/profile`

Response `200`:
```json
{
  "player_id": "p_123",
  "display_name": "Jorrit",
  "total_xp": 48210,
  "streak": 7,
  "best_streak": 12,
  "shields": 1
}
```

## 2. Daily-goal state

**Story:** 016 — the client needs the server's notion of "today" (per player
timezone) plus the day's counters, so streak decisions don't depend on the
device clock. The client's `Day` abstraction (`crates/game/src/progress.rs`)
maps 1:1 onto `day`.

`GET /players/{player_id}/daily`

Response `200`:
```json
{
  "day": 20601,
  "goal": { "clears": 5, "xp": 600 },
  "clears_today": 3,
  "xp_today": 450,
  "goal_met": false
}
```

`POST /players/{player_id}/daily/clears` — report cleared hurdles (batched;
idempotent per `message_id` so retries are safe).

Request:
```json
{
  "clears": [
    {
      "message_id": 42,
      "xp": 150,
      "urgency": "critical",
      "sentiment": "angry",
      "was_burning": false,
      "response_seconds": 187.5,
      "track_cleared": false,
      "at": 1780004321
    }
  ]
}
```
This mirrors the client's `ClearEvent` struct so the server can recompute
daily goals, streaks, and trophy counters authoritatively.

Response `200`: the updated daily-goal state (same shape as the GET), plus
any state changes the client should celebrate:
```json
{
  "daily": { "day": 20601, "goal": { "clears": 5, "xp": 600 }, "clears_today": 5, "xp_today": 750, "goal_met": true },
  "events": [
    { "type": "goal_met", "streak": 8 },
    { "type": "shield_earned" }
  ]
}
```

## 3. Streak state

**Story:** 016 — streak flame, personal best, shield.

`GET /players/{player_id}/streak`

Response `200`:
```json
{
  "streak": 7,
  "best_streak": 12,
  "shields": 1,
  "last_goal_day": 20601
}
```

The server settles missed days (streak reset, shield consumption) on read
and when clears are posted; `events` in endpoint 2 carries
`{ "type": "shield_used", "streak": 7 }` and
`{ "type": "streak_broken", "lost": 7, "best": 12 }` when that happens.

## 4. Trophy awards sync

**Story:** 018 — trophies must be awarded only once per tier across devices
and shown next to league entries.

`GET /players/{player_id}/trophies`

Response `200`:
```json
{
  "trophies": [
    {
      "id": "speed_demon",
      "count": 12,
      "tier": "bronze",
      "next_tier": { "tier": "silver", "needed": 30 },
      "earned_at": { "bronze": 1780001000 }
    },
    { "id": "firefighter", "count": 3, "tier": null, "next_tier": { "tier": "bronze", "needed": 5 }, "earned_at": {} }
  ]
}
```
`id` is one of `speed_demon`, `firefighter`, `peacekeeper`, `clean_sweep`,
`high_jumper` (matching `TrophyId::key()` in `crates/game/src/trophies.rs`);
`tier` is `bronze` / `silver` / `gold` / `null`.

No separate award-POST is needed: awards derive from the clear events posted
in endpoint 2, which keeps "only once" enforcement server-side. New tiers
reached by a batch come back in that response's `events` as
`{ "type": "trophy", "id": "speed_demon", "tier": "bronze" }`.
