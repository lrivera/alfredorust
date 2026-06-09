## ADDED Requirements

### Requirement: CLI client can reuse passwordless login

The system SHALL allow non-browser CLI clients to authenticate through the same passwordless email plus TOTP login semantics as browser users.

#### Scenario: CLI login receives session cookie

- **WHEN** a CLI client submits valid email plus current TOTP code to the login route
- **THEN** the server creates a MongoDB-backed session
- **AND** the response provides enough cookie information for the CLI to persist and reuse the session

#### Scenario: CLI login preserves browser behavior

- **WHEN** browser login is used after CLI support is added
- **THEN** existing browser redirect and cookie behavior remains unchanged
