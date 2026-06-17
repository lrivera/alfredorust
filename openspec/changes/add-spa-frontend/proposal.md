## Why

The application's business surface is now fully available as a stable JSON API
(`/api/admin/*`, `/api/account`, `/api/me/companies`, `/api/tiempo`,
`/pdf/preview`), documented via OpenAPI/Swagger at `/docs`, and already exercised
end-to-end by `spcli` and a live smoke test. The remaining user-facing layer is
still server-rendered Askama HTML at `/admin/*`. We want to replace that HTML
layer with a single-page application (SPA) that consumes the same JSON API, so
the UI is faster, decoupled from the backend, reusable across web/mobile, and
consistent with the API-first direction (CLI today, AI skill next).

## What Changes

- Add a SPA frontend that consumes the existing JSON API exclusively; no new
  business logic moves to the client. The backend remains the authority for
  authentication, tenant isolation, authorization, validation, and side effects.
- Serve the SPA as static assets from the existing Axum server on the tenant
  subdomains, same-origin with the API, so the session cookie and tenant-by-
  subdomain model keep working unchanged.
- Implement the SPA authentication flow over the existing passwordless TOTP
  login (`POST /login`) and session cookie, with a bootstrap that loads the
  current user (`GET /setup`) and available companies (`GET /api/me/companies`).
- Implement tenant context in the SPA via the subdomain (the active company is
  determined by the host), with company switching by navigating between tenant
  subdomains.
- Drive the UI from a typed API client generated from the OpenAPI document, so
  client and server contracts stay in sync.
- Gate UI affordances by the current user's role/permissions for UX only; every
  action is still authorized server-side.
- Migrate each `/admin/*` screen to an SPA route, then retire the corresponding
  Askama templates and HTML/form handlers once parity is verified.

## Capabilities

### New Capabilities

- `spa-shell`: the static app shell, client-side routing, layout, tenant/session
  bootstrap, and role/permission-based UI gating.
- `spa-auth`: the SPA login flow, session lifecycle (401 handling, logout), and
  company selection across tenant subdomains.
- `spa-api-client`: the typed, OpenAPI-derived API client, request/error
  conventions, and how each domain screen maps to API endpoints.

### Modified Capabilities

- `auth`: serves the SPA bootstrap from the same session the browser already
  uses; no change to login or session semantics.
- `tenancy`: the SPA relies on the existing subdomain→active-company resolution;
  no change to tenant isolation rules.

## Impact

- Affected code: a new `frontend/` (or `web/`) build, `src/main.rs` static-asset
  serving and SPA fallback routing, the deploy pipeline (build + ship static
  assets), and eventual removal of `src/templates/**` and HTML/form route
  handlers.
- Affected APIs: none functionally — the SPA consumes the existing JSON API.
  A consolidated `GET /api/me` (profile + companies + permissions) may be added
  for a single bootstrap call.
- Dependencies: a frontend toolchain (recommended Vite + TypeScript + a SPA
  framework) and an OpenAPI client generator.
- Systems: browsers on tenant subdomains; the existing Axum server and MongoDB.

## Current Status

Backend prerequisites are complete: the full JSON API surface exists and returns
flat, SPA-friendly JSON (`id` strings + ISO-8601 dates), OpenAPI docs are served
behind the session, and authentication/tenant/authorization rules are enforced
and tested (including cross-tenant isolation). This change covers only the new
client and the static-serving/migration work.
