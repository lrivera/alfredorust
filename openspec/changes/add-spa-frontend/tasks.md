# Tasks — SPA Frontend (Leptos)

## Phase 0 — Scaffolding & serving
- [x] Add a `frontend/` Leptos CSR crate (**chose `trunk`** for CSR served by the existing Axum server). Standalone crate — backend type sharing deferred (see note below).
- [x] Set up Tailwind (v3 via npm, run by Trunk `pre_build` hook); add `style/input.css`.
- [ ] Install Rust/UI base components (Button, Input, Card/Table, Dialog) into `frontend/src/components/`.
- [x] Serve `frontend/dist/` from Axum with `tower-http` `ServeDir` + SPA `index.html` fallback, ensuring `/api/*`, `/login`, `/logout`, `/setup`, `/qrcode` are not shadowed. Verified: `/api/me`→401, `/dashboard`→SPA, `GET /login`→405, `.wasm`→`application/wasm`.
- [ ] Document the dev workflow (run backend on :8090, run frontend build/watch) and update the deploy pipeline to ship `dist/` assets.

> Note: the backend crate pulls native deps (mongodb/tokio) that don't compile to WASM, so the frontend cannot depend on it directly. Sharing response types requires extracting a `serde`-only DTO crate — deferred. For now the SPA redefines the few bootstrap/login structs.

## Phase 1 — Bootstrap endpoint
- [x] Add `GET /api/me` (profile + active company + role + permissions + companies) in `src/routes/profile.rs`; registered in `src/main.rs`; `#[utoipa::path]` so it appears in `/docs`.
- [x] Integration test (`me_endpoint_bootstraps_active_tenant_profile_and_companies`): `/api/me` returns the active tenant's role/permissions, lists all memberships with the host-resolved one active, redacts the TOTP secret, and `401`s without a session.

## Phase 2 — Login (first vertical slice)
- [x] Build the typed API client core (`src/api.rs`): same-origin fetch via `gloo-net`, error mapping to `ApiError::{Unauthorized,Forbidden,Status,Transport}`.
- [x] Build the login view (`src/app.rs`): email + 6-digit code → `POST /login`; generic auth-failed message; full navigation to tenant shell on success.
- [x] Build the authenticated shell: bootstrap via `GET /api/me`, header/nav, company list with active flag + subdomain switch links, logout via `POST /logout`.
- [x] 401 on bootstrap → render login (auth-state switching). _Global per-request interceptor across all screens lands with routing in Phase 3._
- [ ] Unit tests (`wasm-bindgen-test`): login form validation/state, permission gating helper (`Me::can`).
- [ ] Playwright E2E (against isolated MongoDB): successful login, invalid code, logout, session-expiry re-login.
- [ ] Manual browser verification of the login → shell → logout loop on a tenant subdomain.

## Phase 3+ — Migrate `/admin/*` by category
For each category: build SPA routes/screens over the existing JSON API → reach parity → unit + Playwright coverage → retire the matching Askama templates and HTML/form handlers.

- [x] **finance** — accounts, categories, contacts, transactions, recurring plans, planned entries (incl. bulk-pay/pay + links), forecasts. CRUD + edit, v1 parity, Playwright per field/flow.
- [x] **orders** — service orders incl. complete + dynamic line items.
- [x] **projects** — projects CRUD + advance. (Concept detail page pending below.)
- [x] **resources** — resources (allowed statuses), resource logs (incl. end). (Hourly grid pending below.)
- [ ] **resource usages** — hourly grid + allocations (the heaviest screen).
- [ ] **project detail** — concepts, concept statuses, status summary, per-concept advance.
- [ ] **cfdi** — CFDI reads + download jobs.
- [ ] **tiempo** — time timeline (read view).
- [ ] **admin** — users, companies, SAT configs (incl. multipart `.cer`/`.key` upload).

> **ON HOLD — `pdf` (PDF preview):** intentionally deferred at the user's request
> (not in use for now). Resume when needed: migrate `/pdf` (editor) + `POST
> /pdf/preview` to an SPA screen. — held 2026-06-18

## Phase N — Cleanup
- [ ] Remove `src/templates/**` and HTML/form route handlers once every category has SPA parity.
- [ ] Trim now-unused HTML-only dependencies (Askama) if nothing else uses them.
- [ ] Final E2E pass covering each migrated category.
