## ADDED Requirements

### Requirement: Consolidated bootstrap endpoint

The system SHALL provide `GET /api/me` that returns the authenticated user's profile, active company, role, permissions, and company memberships in a single response, without changing the existing `POST /login` or session semantics.

#### Scenario: Bootstrap with a valid session

- **WHEN** an authenticated client requests `GET /api/me`
- **THEN** the response includes email, active company name, role, the permission list for the active tenant, and the companies the user belongs to (id, name, slug, active flag)
- **AND** the response never includes the TOTP secret

#### Scenario: Bootstrap without a session

- **WHEN** a client requests `GET /api/me` without a valid session
- **THEN** the server responds `401`

#### Scenario: Active company reflects the requesting host

- **WHEN** the request arrives on a tenant subdomain the user belongs to
- **THEN** the active company, role, and permissions in the response are those of that tenant
