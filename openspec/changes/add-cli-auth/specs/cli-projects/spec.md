## ADDED Requirements

### Requirement: CLI exposes project workflow commands through JSON APIs

The system SHALL provide project CLI commands that use authenticated, tenant-scoped JSON APIs for project records, concept statuses, project concepts, and status summaries.

#### Scenario: Project list is requested

- **WHEN** the user runs `spcli projects list` with a valid session and selected company
- **THEN** the CLI returns projects for the active company only
- **AND** includes stable identifiers and workflow fields suitable for automation

#### Scenario: Project detail is requested

- **WHEN** the user runs `spcli projects get <id>`
- **THEN** the CLI returns the project detail only if it belongs to the selected company
- **AND** fails with a structured not-found or forbidden error otherwise

### Requirement: CLI uses existing project JSON APIs where available

The system SHALL use existing `/api/admin/*` project JSON APIs for concept statuses, project concepts, status summaries, and resource usage relationships before adding new APIs.

#### Scenario: Concept statuses are listed

- **WHEN** the user runs `spcli projects statuses list`
- **THEN** the CLI consumes `GET /api/admin/concept_statuses`
- **AND** returns stable JSON in automation mode

#### Scenario: Project concepts are advanced

- **WHEN** the user runs `spcli projects concepts advance <id>`
- **THEN** the CLI consumes the JSON advance endpoint
- **AND** returns the new status response from the server

#### Scenario: Concept status is created or changed

- **WHEN** the user runs `spcli projects statuses create`, `update`, or `delete --yes`
- **THEN** the CLI consumes the concept status JSON APIs
- **AND** the server remains responsible for tenant ownership, admin authorization, and workflow marker validation

#### Scenario: Project concept is created or changed

- **WHEN** the user runs `spcli projects concepts create`, `update`, or `delete --yes`
- **THEN** the CLI consumes the project concept JSON APIs
- **AND** the server remains responsible for tenant ownership, admin authorization, and status validation

#### Scenario: Project status summary is requested

- **WHEN** the user runs `spcli projects status-summary --project-id <id>`
- **THEN** the CLI consumes the project status summary JSON API
- **AND** returns status quantities for the selected project and company

### Requirement: CLI does not use project HTML forms as API contracts

The system SHALL add JSON APIs before implementing CLI commands for project capabilities that are currently HTML/form-only.

#### Scenario: Project CRUD is not yet JSON-ready

- **WHEN** project list, get, create, update, delete, or advance behavior is only available through `/admin/projects*` HTML/form routes
- **THEN** `spcli` SHALL wait for documented JSON APIs before exposing those mutations

### Requirement: Project CLI mutations enforce destructive confirmation and permissions

The system SHALL enforce server permissions and local destructive confirmation for project CLI mutations.

#### Scenario: Project delete is requested without confirmation

- **WHEN** the user runs `spcli projects delete <id>` without `--yes`
- **THEN** the CLI rejects the command before sending the request

#### Scenario: Staff user lacks project mutation permission

- **WHEN** a user without the required role or permission runs a project mutation command
- **THEN** the server rejects the request
- **AND** the CLI returns a structured forbidden error

### Requirement: Project CLI behavior is covered by harness tests

The system SHALL add harness coverage for project CLI behavior.

#### Scenario: Project CLI respects tenant isolation

- **WHEN** the harness creates projects and concepts in multiple companies
- **THEN** project CLI commands only return or mutate records for the selected company

#### Scenario: Concept workflow is tested

- **WHEN** the harness advances a project concept through the CLI
- **THEN** the persisted concept status changes according to the configured workflow
