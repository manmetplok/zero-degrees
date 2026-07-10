# Architecture

## Goal

Mobile game, Android and iOS from one Rust codebase.

## Stack

| Layer | Choice |
|---|---|
| Language | Rust |
| Game engine | macroquad, built natively per platform |
| Android build | cargo-quad-apk (docker-based, single command) |
| iOS build | macroquad XCode project export |
| Local persistence | SQLite via rusqlite, or file/JSON store for simple save data |
| Backend | Rust, Rocket |
| Backend database | SQLite via sqlx |
| API | REST (JSON over HTTPS); WebSocket only if realtime features appear |
| HTTP client | ureq or reqwest from the game crate |
| CI/CD | GitHub Actions (Linux runner for Android, macOS runner for iOS) |

## Why this stack

**macroquad, native builds**: the game compiles directly to Android and iOS binaries through macroquad's first-class mobile support — no webview, no wasm, no shell framework. Considered against these alternatives:

- **Tauri Mobile + macroquad wasm**: original plan. Dropped after research found zero shipped or documented examples of any game engine inside a Tauri Mobile webview, and the macroquad-to-Tauri IPC bridge would have been custom, unproven glue. Native macroquad is macroquad's own documented mobile route.
- **Tauri Mobile + Phaser**: proven webview tech, but TypeScript game code breaks the Rust-only goal.
- **Bevy**: more powerful (ECS, 3D, larger plugin ecosystem), heavier setup and a steeper learning curve. Better fit if the scope grows.
- **ggez / Comfy**: weaker mobile story than macroquad's docker Android builds and XCode export.

**Rust throughout**: game, backend, and shared types in one language, one workspace. No JS/TS anywhere.

**Rocket backend**: serves online features (accounts, leaderboards, sync). Simplicity is the deciding criterion; the backend stays small. Rocket over the alternatives:
- **Axum**: the current ecosystem default, but its Tower middleware model adds concepts a small API does not need.
- **Actix Web**: performance-oriented, more setup for the same result.
- **Poem**: similarly simple with built-in OpenAPI, but a smaller community and fewer resources than Rocket.

Rocket gives macro-based routing, built-in JSON handling, request guards, and typed config with the least ceremony. Since 0.5 it runs async on stable Rust.

A shared crate holds the API types (requests, responses, domain models), so client and server compile against the same definitions and drift becomes a compile error.

**SQLite + sqlx**: compile-time checked queries, async, no ORM layer to fight, and no database server to run — one file on disk. The client uses SQLite too (rusqlite), so both ends share one SQL dialect. Consequences accepted for simplicity:
- One backend instance; SQLite allows one writer. Fine at this scale.
- Backups are file copies.
- If load or replication needs ever outgrow this, sqlx makes a move to PostgreSQL mostly a connection-string and migration exercise.

## Project structure

```
zero-degrees/
  Cargo.toml            # workspace root
  crates/
    game/                # macroquad game, builds to Android, iOS, and desktop
    shared/              # API types shared by game and backend
    backend/             # Rocket server
```

`game` also builds as a desktop binary for fast local iteration and for running logic tests without a device or emulator. `backend` shares `shared` and one dependency tree, but builds and deploys independently.

## Open items

- Verify cargo-quad-apk maintenance state and current Android API level support before the first release build.
- iOS release pipeline (signing, provisioning, store upload from the exported XCode project) is undefined.
- The game must work offline; the backend feature set (accounts? leaderboards? sync?) is unscoped. Scope it before building.
- Authentication approach for the API (anonymous device ID vs full accounts) follows from that feature set.
- Native platform features (notifications, IAP) have no Tauri plugin ecosystem anymore; if needed, they require per-platform crates or FFI. None are planned yet.
- Persistence choice (rusqlite vs file/JSON store) depends on save-data complexity; decide once the gameplay data model exists.
