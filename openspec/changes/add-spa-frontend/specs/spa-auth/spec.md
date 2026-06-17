## ADDED Requirements

### Requirement: SPA login uses the existing passwordless TOTP flow

The system SHALL let the SPA authenticate by collecting email and a 6-digit TOTP code and posting them to `POST /login`, relying on the server to set the session cookie. The SPA SHALL NOT read, store, or transmit the TOTP secret or the session cookie value.

#### Scenario: Successful login

- **WHEN** the user submits a valid email and current TOTP code
- **THEN** the SPA posts them to `POST /login`
- **AND** on success the server sets the session cookie
- **AND** the SPA navigates to the user's tenant shell using the server-provided redirect target

#### Scenario: Invalid credentials do not enumerate accounts

- **WHEN** the user submits an unknown email or an invalid code
- **THEN** the SPA shows a generic authentication-failed message
- **AND** does not reveal whether the email exists

#### Scenario: TOTP secret is never handled by the client

- **WHEN** the SPA performs login or bootstrap
- **THEN** it only sends the email and the user-entered code
- **AND** it never receives or stores the TOTP secret

### Requirement: SPA handles session expiry by forcing re-login

The system SHALL treat any `401` response from a protected request as an expired or missing session, clear client state, and route to the login view.

#### Scenario: Session expires mid-use

- **WHEN** a protected `/api/*` request returns `401`
- **THEN** the SPA clears bootstrapped user state
- **AND** routes to the login view
- **AND** does not retry the request in a loop

### Requirement: SPA logout ends the session

The system SHALL provide a logout action that posts to `POST /logout` and returns the user to the login view.

#### Scenario: User logs out

- **WHEN** the user triggers logout
- **THEN** the SPA posts to `POST /logout`
- **AND** routes to the login view
- **AND** subsequent protected requests are unauthenticated

### Requirement: Company selection uses tenant subdomains

The system SHALL let multi-company users switch the active company by navigating between tenant subdomains, with no client-side tenant override.

#### Scenario: Switch active company

- **WHEN** the user selects a different company from the bootstrapped list
- **THEN** the SPA navigates to that company's tenant subdomain
- **AND** the shell re-bootstraps with that company as active

#### Scenario: Active company is determined by the host

- **WHEN** the SPA bootstraps on a tenant subdomain
- **THEN** the active company is the one resolved from the host by the server
- **AND** the SPA does not assert tenant context by any other means
