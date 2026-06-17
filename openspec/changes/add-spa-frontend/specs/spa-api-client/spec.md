## ADDED Requirements

### Requirement: SPA drives the UI through a typed API client over the existing JSON API

The system SHALL access all business data through a typed client that calls the existing JSON API exclusively, reusing the backend's response types so client and server contracts cannot drift.

#### Scenario: Reads and mutations go through the typed client

- **WHEN** a screen needs data or performs an action
- **THEN** it calls the typed client method mapped to the corresponding `/api/*` endpoint
- **AND** no business logic or validation is duplicated on the client

#### Scenario: Types are shared, not redefined

- **WHEN** a response type changes on the backend
- **THEN** the SPA consumes the shared Rust type
- **AND** a contract mismatch surfaces at compile time rather than at runtime

### Requirement: API client sends same-origin credentials and a consistent request shape

The system SHALL issue requests same-origin so the session cookie is sent automatically, and SHALL use a consistent request/response convention across all endpoints.

#### Scenario: Authenticated request

- **WHEN** the client calls a protected endpoint
- **THEN** the request is same-origin and includes the session cookie
- **AND** no auth token is stored or attached by the client

### Requirement: API client surfaces errors explicitly

The system SHALL map non-success responses to explicit client outcomes rather than failing silently or falling back to stale data.

#### Scenario: Unauthorized response

- **WHEN** a request returns `401`
- **THEN** the client signals a session-expired outcome that triggers re-login

#### Scenario: Forbidden response

- **WHEN** a request returns `403`
- **THEN** the client signals a not-allowed outcome
- **AND** the screen shows an explicit not-allowed state

#### Scenario: Validation or server error

- **WHEN** a request returns a `4xx` validation error or a `5xx`
- **THEN** the client surfaces the error to the screen
- **AND** does not present the action as having succeeded
