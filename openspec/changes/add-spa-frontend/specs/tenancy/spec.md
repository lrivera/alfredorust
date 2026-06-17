## ADDED Requirements

### Requirement: SPA relies on subdomain tenant resolution unchanged

The system SHALL resolve the SPA's active company from the request host exactly as for server-rendered routes, without introducing any client-supplied tenant override.

#### Scenario: SPA request is scoped by host

- **WHEN** the SPA calls a protected `/api/*` endpoint on a tenant subdomain
- **THEN** the server scopes the data to the company resolved from that host
- **AND** ignores any tenant hint that might be supplied by the client

#### Scenario: Subdomain outside the user's memberships

- **WHEN** the SPA is loaded on a tenant subdomain the user does not belong to
- **THEN** the server responds `401`
- **AND** the SPA routes to the login view
