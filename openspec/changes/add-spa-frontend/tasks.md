# Tasks — SPA Frontend (Leptos)

## Phase 0 — Scaffolding & serving
- [ ] Add a `frontend/` Leptos CSR crate (decide `trunk` vs `cargo-leptos`); wire it into the Cargo workspace so backend model types can be shared.
- [ ] Set up Tailwind via the chosen build tool; add `style/input.css`.
- [ ] Install Rust/UI base components (Button, Input, Card/Table, Dialog) into `frontend/src/components/`.
- [ ] Serve `frontend/dist/` from Axum with `tower-http` `ServeDir` + SPA `index.html` fallback on tenant subdomains, ensuring `/api/*`, `/login`, `/logout`, `/setup`, `/qrcode` are not shadowed.
- [ ] Document the dev workflow (run backend on :8090, run frontend build/watch) and update the deploy pipeline to ship `dist/` assets.

## Phase 1 — Bootstrap endpoint
- [ ] Add `GET /api/me` (profile + active company + role + permissions + companies) in `src/routes/profile.rs`; register in `src/main.rs`; add `#[utoipa::path]` so it appears in `/docs`.
- [ ] Integration test: `/api/me` returns the active tenant's role/permissions, redacts the TOTP secret, and `401`s without a session.

## Phase 2 — Login (first vertical slice)
- [ ] Build the typed API client core (same-origin fetch, error mapping for 401/403/4xx/5xx).
- [ ] Build the login view: email + 6-digit code → `POST /login`; generic auth-failed message; navigate to tenant shell on success.
- [ ] Build the authenticated shell: bootstrap via `GET /api/me`, layout/nav, company switcher (navigates between subdomains), logout via `POST /logout`.
- [ ] Global 401 interceptor → clear state + route to login.
- [ ] Unit tests (`wasm-bindgen-test`): login form validation/state, 401 interceptor, permission gating helper.
- [ ] Playwright E2E (against isolated MongoDB): successful login, invalid code, logout, session-expiry re-login.

## Phase 3+ — Migrate `/admin/*` by category
For each category: build SPA routes/screens over the existing JSON API → reach parity → unit + Playwright coverage → retire the matching Askama templates and HTML/form handlers.

- [ ] **finance** — accounts, categories, contacts, transactions, recurring plans, planned entries (incl. bulk-pay/pay), forecasts.
- [ ] **projects** — projects, concepts, concept statuses, status summary, workflow advance.
- [ ] **resources** — resources, resource logs (incl. end), resource usages + hourly grid save.
- [ ] **orders** — service orders (incl. complete).
- [ ] **admin** — users, companies, SAT configs (incl. multipart `.cer`/`.key` upload).
- [ ] **cfdi** — CFDI reads + download jobs.
- [ ] **tiempo / pdf** — time timeline; PDF preview.

## Phase N — Cleanup
- [ ] Remove `src/templates/**` and HTML/form route handlers once every category has SPA parity.
- [ ] Trim now-unused HTML-only dependencies (Askama) if nothing else uses them.
- [ ] Final E2E pass covering each migrated category.
