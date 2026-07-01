# AGENTS.md — altp-producer

A **Rust (edition 2024) feature-flag web service** using Axum, PostgreSQL via SQLx, OpenTelemetry, and Prometheus metrics.

---

## Repo at a glance

| What | Where |
|---|---|
| Entrypoint | `src/main.rs` — tokio main, telemetry init, graceful shutdown |
| Router | `src/router.rs` — `/health`, `/metrics` wired; `/flag` routes **commented out** |
| State | `src/state.rs` — `AppState { db: PgPool }` only |
| Feature modules | `src/features/template/` — models, handlers, DTOs, repo |
| Infra | `src/infra/` — telemetry (OTLP) + metrics (Prometheus) |
| Error handling | `src/error.rs` — `AppError` enum with `IntoResponse` via `thiserror` |

**This is a work-in-progress scaffold.** The handlers in `src/features/template/handlers.rs` call
`state.get_cached_flag()` and `state.flags_cache.remove()` which **do not exist** on `AppState`.
The `lib.rs` declares `pub mod features { }` (empty), so the template module is **not compiled in**.
The `migrations/` and `.sqlx/` directories are both empty.

---

## Developer commands

```sh
cargo check              # Type-check only (fastest verification)
cargo build              # Build debug
cargo build --release    # Release build
cargo test               # All tests (none exist yet)
cargo fmt                # Format (uses stable defaults)
cargo clippy             # Lint (uses stable defaults)
```

There is **no** `rust-toolchain.toml`, no `Makefile`, no CI config, no pre-commit hooks.

---

## Required environment

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | **required** — no default | Panics at startup if unset |
| `RUST_LOG` | `app=debug,tower_http=debug,axum::rejection=trace` | EnvFilter for tracing |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | hardcoded `http://localhost:4317` in `telemetry.rs` | gRPC OTLP endpoint |

`.env` is loaded via `dotenvy` at startup (missing file is silently ignored).

---

## Known structural gaps (don't get tripped up)

- **`lib.rs` `pub mod features { }` is empty** — the `template` submodule (models, handlers, db, dtos) exists on disk but won't compile until it is declared in `lib.rs`.
- **`state.rs` lacks cache** — `handlers.rs` references `get_cached_flag()` and `flags_cache` which are not implemented.
- **`router.rs` has commented routes** — `/flag` endpoints exist in handlers but are not wired.
- **`find_by_key()` has a hardcoded 4-second `sleep()`** — simulated latency, likely a dev placeholder.
- **`bootstrap::start()` uses `request_id = %"TODO_UUID"` placeholder** — all instrument macros inherit this.
- **No DB migrations** — `migrations/` directory exists but is empty; `sqlx::migrate!()` is not called anywhere.
- **`.sqlx/` is empty** — `SQLX_OFFLINE=true` is set in the Dockerfile but no offline data has been prepared.
- **Port 3000 hardcoded** in `src/main.rs`.

---

## Docker build

Multi-stage build using `cargo-chef`, targets `x86_64-unknown-linux-musl`, final image from `scratch` (~0 MB base).
Set `SQLX_OFFLINE=true` during build (no DB needed at build time). Requires `.sqlx/` offline data if compile-time
query verification is ever added.

---

## Agent configuration

The `.opencode/opencode.json` pre-configures the agent as a **DDD-focused developer** with full read/write/bash
permissions and a detailed DDD prompt (ubiquitous language, aggregates, bounded contexts, domain events, etc.).
This prompt already covers the DDD principles — do not duplicate it here. Follow it when modifying domain code.

---

## Conventions

- **Error handling**: use `AppError` enum (import from `crate::error::AppError`). Add new variants via `#[error(...)]`
  with `#[from]` for conversions. `IntoResponse` is already implemented.
- **Feature structure**: each bounded context lives under `src/features/<name>/` with `mod.rs`, `models.rs`,
  `handlers.rs`, `db.rs`, `dtos.rs`. Currently only `template/` exists (not wired).
- **Tracing**: use `#[instrument(name = "...", skip(...), fields(...))]` on handlers and repository methods.
- **Infra layer**: cross-cutting concerns (telemetry, metrics) go in `src/infra/`. No domain logic here.
- **State**: shared app state goes in `AppState` (`src/state.rs`). Add fields there when needed.
- **DB queries**: use `sqlx::query_as!` with compile-time verification in `db.rs` files.
- **Telemetry service name**: `"app-core"` (constant `TELEMETRY_APP_ID` in `telemetry.rs`).
- **Prometheus metric names**: prefixed `THISAPP_` (constants in `metrics.rs`).
