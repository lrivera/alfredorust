# Backend ⇄ UI session coordination

Two Claude Code sessions are working this repo in parallel:

- **UI session** — owns `frontend/` only (Leptos SPA). Does **not** edit backend code.
- **Backend session ("APP")** — owns the Axum server (`src/`, `Cargo.toml`, `tests/`, `openspec/`, etc.).

**Rule for the backend session:** please do **not** modify anything under `frontend/`.
The UI session owns that directory. Anything the UI session needs from the backend
goes in the "Open requests" section below instead of being changed directly.

---

## Backend changes already made by the UI session (heads-up — please don't revert)

While bootstrapping the SPA, the UI session made these backend edits **before** the
UI-only split was agreed. They are uncommitted. Flagging them so we don't stomp:

- `src/routes/profile.rs` — added `MeResponse` + `pub async fn me` (consolidated
  `GET /api/me`: profile + active-tenant role/permissions + companies, secret
  redacted). Refactored the company-listing into a shared `collect_companies`
  helper (also removed some stray `println!` debug lines from `me_companies`).
- `src/routes/mod.rs` — `pub use profile::{me, me_companies};`
- `src/main.rs` — registered `GET /api/me`; added `tower-http` `ServeDir` +
  `ServeFile` fallback serving `frontend/dist` (env `SPA_DIST` overrides).
- `src/openapi.rs` — registered `profile::me` in the `paths(...)`.
- `Cargo.toml` — added `tower-http = { version = "0.6", features = ["fs"] }`.
- `tests/http_cruds.rs` — added `/api/me` route to the test `build_app` and the
  test `me_endpoint_bootstraps_active_tenant_profile_and_companies` (passes).

If you (backend session) have your own changes to these files, let's reconcile
rather than overwrite. After this point the UI session will not touch `src/`.

---

## Open requests from UI → backend

_(none right now)_

When the UI session needs a backend change or hits a backend bug, it appends an
item here with: what's needed, why, and the endpoint/file involved. Format:

### [ ] <short title>
- **Need:** ...
- **Why / UI context:** ...
- **Suggested endpoint/shape:** ...
- **Filed:** <date>

Backend session: check this box and add a one-line resolution note when done.
