# Design — SPA Frontend (Leptos)

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | **Leptos** (CSR/WASM) | Single language (Rust) across backend and frontend; backend domain types can be shared as a crate dependency instead of generating a client; fine-grained reactivity (signals). |
| Component kit | **Rust/UI** (`rust-ui.com`) | shadcn-style copy-paste components for Leptos, Tailwind-based, owned in-repo and freely editable; avoids building primitives (Button/Input/Dialog/Table) from scratch. |
| Styling | **Tailwind CSS** | Native support in `cargo-leptos`; dynamic state via Leptos `class:` directives. |
| Build tool | `trunk` (CSR) or `cargo-leptos` | Compiles Rust→WASM and runs the Tailwind build; emits static assets. |
| Serving | **Static directory** via `tower-http` `ServeDir` + SPA fallback | The Axum server serves `frontend/dist/` on tenant subdomains, same-origin with the JSON API, so the session cookie and subdomain→tenant model keep working unchanged. Deploy ships the `dist/` assets alongside the binary. |
| Bootstrap | **New `GET /api/me`** | One call returns profile + permissions + companies, replacing the `/setup` + `/api/me/companies` pair at startup. |
| API contract | **Typed client from shared Rust types** | The SPA reuses the backend's serde response structs (flat JSON: `id` strings + ISO-8601 dates). OpenAPI at `/docs` stays the human-readable reference. |
| Unit tests | `wasm-bindgen-test` (Leptos components/logic) | Runs component and reactive-state logic in a headless browser/runtime. |
| E2E tests | **Playwright** | Drives real login + per-screen flows against a running server with an isolated MongoDB, mirroring the existing integration-test isolation. |

## Why not React

React was the safer/faster default (huge ecosystem, mature OpenAPI generators, more help when stuck). We are accepting a smaller ecosystem and rougher frontend tooling in exchange for: one language and one toolchain, shared types with no generation/drift step, and consistency with the API-first Rust direction. Rust/UI removes the "build all UI primitives from scratch" objection.

## Architecture

```
alfredorust/                 (existing Axum backend crate)
  src/main.rs                -> mounts ServeDir + SPA fallback on tenant hosts
  src/routes/profile.rs      -> add GET /api/me (profile + companies + permissions)
frontend/                    (new Leptos CSR crate)
  src/
    main.rs                  -> mount app
    app.rs                   -> router + shell
    components/              -> Rust/UI components (copied in) + app components
    api/                     -> typed client over fetch, shared response types
    routes/                  -> one module per migrated screen
  style/input.css            -> Tailwind entry
  dist/                      -> build output served by Axum
  tests/                     -> wasm-bindgen-test
e2e/                         -> Playwright specs + fixtures
```

### Serving and routing
- Axum serves the SPA on tenant subdomains (`slug.<BASE_DOMAIN>`). `app.<BASE_DOMAIN>` remains the login host.
- `ServeDir` serves hashed static assets; unknown non-`/api`, non-asset paths fall back to `index.html` so client-side routing owns the path space.
- All `/api/*`, `/login`, `/logout`, `/setup`, `/qrcode` keep their current behavior. The SPA fallback must NOT shadow these.

### Auth & session
- Login: the SPA POSTs `{email, code}` to `POST /login`; the server sets the session cookie (HttpOnly) exactly as today. The SPA never reads or stores the cookie or the TOTP secret.
- The session middleware already returns **401 with a plain body** (not an HTML redirect) when unauthenticated — the SPA intercepts any `401` from `/api/*`, clears client state, and routes to login.
- Logout: `POST /logout`, then route to login.

### Tenant context
- The active company is the subdomain host — unchanged. Company switching = navigate to another tenant subdomain (full document navigation, which re-bootstraps the SPA under the new host). No client-side tenant override exists or is trusted.

### Permissions
- `/api/me` returns the user's role + permissions for the active tenant. The SPA gates affordances (hide/disable buttons, menu items) for UX only. Every mutation is still authorized server-side; a hidden button is never the authorization boundary.

## Migration strategy
1. **Login first** — prove the full loop end-to-end: serve SPA, login via TOTP, bootstrap `/api/me`, land on an authenticated shell, logout, 401 handling.
2. Then migrate `/admin/*` **by category**, one capability at a time, each reaching parity before the matching Askama templates and HTML/form handlers are retired:
   - finance (accounts, categories, contacts, transactions, recurring plans, planned entries, forecasts)
   - projects (projects, concepts, concept statuses, status summary)
   - resources (resources, resource logs, resource usages + grid)
   - orders (service orders)
   - admin (users, companies, SAT configs)
   - cfdi (reads + jobs)
   - tiempo (timeline) and pdf (preview)
3. JSON API endpoints already exist for each category (see the `add-cli-auth` inventory); the SPA work is purely client + wiring.

## Open questions
- `trunk` vs `cargo-leptos` for the CSR build — decide at scaffolding time based on Rust/UI's expected setup.
- Whether `frontend/` is a separate crate or a Cargo workspace member sharing the backend's model crate for type reuse. Prefer a workspace so response types are shared, not duplicated.

## Review answers (per openspec/README.md)
- **Who owns the data?** Unchanged — the tenant company resolved from the subdomain.
- **Who can see/mutate it?** Unchanged — server-side role/permission checks; the SPA only mirrors them for UX.
- **Collections read/written?** None new from the client. `GET /api/me` reads `users`, `user_companies`, `companies`.
- **Financial side effects?** None introduced by this change; the SPA calls existing endpoints whose side effects are already specified.
- **Proof?** `wasm-bindgen-test` for component/logic; Playwright E2E against an isolated MongoDB for login and each migrated screen.
- **What fails loudly?** Any `/api/*` error surfaces in the UI; a `401` forces re-login; a `403` shows an explicit "not allowed" state rather than silently hiding the failure.
