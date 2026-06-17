## ADDED Requirements

### Requirement: SPA is served as static assets on tenant subdomains

The system SHALL serve the compiled SPA as static assets from the existing Axum server on tenant subdomains, same-origin with the JSON API, without changing the subdomain→active-company resolution.

#### Scenario: SPA shell loads on a tenant subdomain

- **WHEN** an authenticated browser requests a non-API, non-asset path on `slug.<BASE_DOMAIN>`
- **THEN** the server returns the SPA `index.html`
- **AND** client-side routing resolves the path
- **AND** the active company is the one identified by the subdomain

#### Scenario: API and auth routes are not shadowed by the SPA fallback

- **WHEN** a request targets `/api/*`, `/login`, `/logout`, `/setup`, or `/qrcode`
- **THEN** the existing handler responds
- **AND** the SPA `index.html` fallback does not intercept it

#### Scenario: Hashed assets are served directly

- **WHEN** the browser requests a built asset (JS/WASM/CSS) emitted by the frontend build
- **THEN** the server serves it from the static asset directory
- **AND** does not fall back to `index.html`

### Requirement: SPA bootstraps user, permissions, and companies in one call

The system SHALL provide `GET /api/me` returning the current user's profile, role, permissions, and the companies they belong to (with the active company marked), so the SPA can bootstrap with a single request.

#### Scenario: Authenticated bootstrap

- **WHEN** the SPA requests `GET /api/me` with a valid session
- **THEN** the response includes email, active company name, role, permissions, and the list of companies with slugs and an active flag

#### Scenario: Unauthenticated bootstrap

- **WHEN** the SPA requests `GET /api/me` without a valid session
- **THEN** the server responds `401`
- **AND** the SPA routes to the login view

### Requirement: SPA gates UI affordances by permission for UX only

The system SHALL hide or disable client-side affordances the current user lacks permission for, while relying on the server as the sole authorization boundary.

#### Scenario: Affordance hidden without permission

- **WHEN** the bootstrapped permissions do not include an action
- **THEN** the SPA hides or disables the corresponding control

#### Scenario: Server remains the authority

- **WHEN** a request is made for an action the user lacks permission for, regardless of client gating
- **THEN** the server rejects it with `403`
- **AND** the SPA surfaces an explicit not-allowed state rather than silently ignoring it
