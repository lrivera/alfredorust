## ADDED Requirements

### Requirement: CLI exposes resource and time tracking commands through JSON APIs

The system SHALL provide CLI commands for resources, resource logs, resource usages, usage allocations, and time timeline data using authenticated, tenant-scoped JSON APIs.

#### Scenario: Resource usage list is requested

- **WHEN** the user runs `spcli resources usages list` with a valid session and selected company
- **THEN** the CLI consumes the resource usage JSON API
- **AND** returns only usage records for the selected company

#### Scenario: Time timeline is requested

- **WHEN** the user runs `spcli time timeline` with date or range filters
- **THEN** the CLI consumes `GET /api/tiempo` with explicit query parameters
- **AND** returns timeline buckets in JSON output when requested

### Requirement: CLI distinguishes logs from usages

The system SHALL keep resource logs and resource usages as separate CLI command groups because they represent different domain concepts.

#### Scenario: Resource log command is requested

- **WHEN** the user runs a command under `spcli resources logs`
- **THEN** the command operates on resource log records
- **AND** does not mutate resource usage allocation records

#### Scenario: Resource usage allocation command is requested

- **WHEN** the user runs `spcli resources usages allocations replace <usage-id>`
- **THEN** the command replaces allocations for that usage through the JSON API
- **AND** returns the resulting allocation ids or a structured error

### Requirement: CLI resource commands require APIs for HTML-only capabilities

The system SHALL add JSON APIs before implementing CLI commands for resource capabilities that are currently HTML/form-only.

#### Scenario: Resource CRUD is not yet JSON-ready

- **WHEN** resource create, update, delete, or detail behavior is only available through `/admin/resources*` HTML/form routes
- **THEN** `spcli` SHALL NOT scrape or submit those pages
- **AND** backend JSON APIs SHALL be specified and implemented first

#### Scenario: Resource log end is requested

- **WHEN** `spcli resources logs end <id>` is planned
- **THEN** a JSON endpoint SHALL exist that performs the same server-side validation as the web route

### Requirement: Resource CLI mutations use explicit datetime and amount formats

The system SHALL require stable input formats for dates, datetimes, durations, quantities, and costs.

#### Scenario: Resource usage create includes datetimes

- **WHEN** the user runs `spcli resources usages create`
- **THEN** started and ended datetimes are supplied as explicit RFC3339 values
- **AND** invalid datetimes fail loudly before or at the server validation boundary

#### Scenario: Resource usage is updated or deleted

- **WHEN** the user runs `spcli resources usages update <id>` or `spcli resources usages delete <id> --yes`
- **THEN** the CLI sends an authenticated request to the resource usage JSON API
- **AND** the server remains responsible for tenant ownership, admin authorization, and allocation recalculation side effects

### Requirement: Resource CLI behavior is covered by harness tests

The system SHALL add harness coverage for resource CLI behavior.

#### Scenario: Resource usage allocation replacement is tested

- **WHEN** the harness replaces allocations through the CLI
- **THEN** the persisted allocation set matches the requested concept ids or ratios

#### Scenario: Staff resource permissions are tested

- **WHEN** a staff user runs resource commands
- **THEN** the harness verifies the allowed read or mutation behavior for that user's permissions
