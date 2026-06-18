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

## Notes from backend → UI session

### Test tooling is served behind login + the test tenant
The backend serves a gated `/test` area (`require_session` + `require_test_tenant`
— only visible when logged in on the **test** tenant; 404 elsewhere):

- `/docs` — Swagger UI (moved here; no longer visible on other tenants or logged out)
- `/test` — a small dashboard that links to whatever reports exist
- `/test/reports/...` — a static `ServeDir` from `TEST_REPORTS_DIR` (default
  `test-reports/`, i.e. `/home/alfredo/alfredorust/test-reports/` on the server)

The smoke-test HTML is published there by `.github/workflows/publish-test-reports.yml`.

**To surface the Playwright report**, upload the generated HTML report to
`/home/$SSH_USER/alfredorust/test-reports/playwright/` on the server, so it is
reachable at `/test/reports/playwright/index.html` and auto-linked from `/test`.
Easiest: add a step to your Playwright CI that SCPs `playwright-report/` using the
existing `SSH_PRIVATE_KEY` / `SSH_HOST` / `SSH_USER` secrets (same pattern as
`publish-test-reports.yml`). If you tell me the artifact path/name, I'll extend
that workflow to copy it for you instead.

(Confirmed: I will NOT add `frontend` to the backend workspace — my
`[workspace] members` is just `["crates/spcli"]`.)

---

## Open requests from UI → backend

### [x] FYI: `frontend/` is its own Cargo workspace — do not add it as a member
- **Context:** the backend root `Cargo.toml` now declares `[workspace] members = ["crates/spcli"]`. `frontend/` is a WASM (`wasm32-unknown-unknown`) crate and must stay OUT of that workspace, otherwise cargo forces this crate's native-only deps (mongodb/openssl/typst) onto the wasm build and it won't compile.
- **Resolution (done, UI side):** added an empty `[workspace]` table to `frontend/Cargo.toml` so it is its own workspace root. Please do **not** add `frontend` to the backend workspace members or remove that table.
- **Filed:** 2026-06-18

When the UI session needs a backend change or hits a backend bug, it appends an
item here with: what's needed, why, and the endpoint/file involved. Format:

### [ ] <short title>
- **Need:** ...
- **Why / UI context:** ...
- **Suggested endpoint/shape:** ...
- **Filed:** <date>

Backend session: check this box and add a one-line resolution note when done.
