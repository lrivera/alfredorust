## ADDED Requirements

### Requirement: CLI exposes PDF preview through the existing JSON API

The system SHALL provide a PDF CLI command that submits Typst content to the server preview endpoint and returns the preview result in the requested output format.

#### Scenario: PDF preview is requested

- **WHEN** the user runs `spcli pdf preview` with Typst content supplied from a file or stdin
- **THEN** the CLI sends an authenticated request to `POST /pdf/preview`
- **AND** returns the server result without writing implicit files unless an explicit output path is provided

#### Scenario: PDF preview fails

- **WHEN** Typst rendering fails on the server
- **THEN** the CLI returns a structured error or JSON response with the server error details
- **AND** does not hide the render failure behind a fallback

### Requirement: CLI exposes time timeline data through the existing JSON API

The system SHALL provide a time CLI command that queries timeline data through `GET /api/tiempo` with explicit filters.

#### Scenario: Time timeline is requested

- **WHEN** the user runs `spcli time timeline --date <date>` or an equivalent explicit range
- **THEN** the CLI sends the query to the timeline API using selected tenant context
- **AND** returns timeline buckets for the active company only

#### Scenario: Time query is missing required filters

- **WHEN** the user runs a time command without the required date or range inputs
- **THEN** the CLI fails before sending a request
- **AND** returns a structured validation error

### Requirement: PDF and time CLI commands remain non-destructive

The system SHALL treat PDF preview and time timeline commands as read-only/non-destructive commands.

#### Scenario: Manifest is requested

- **WHEN** `spcli --json manifest` includes PDF or time commands
- **THEN** their entries mark `destructive` as false
- **AND** identify required auth and company context

### Requirement: PDF and time CLI behavior is covered by harness tests

The system SHALL add harness coverage for PDF and time CLI behavior.

#### Scenario: PDF preview command is tested

- **WHEN** the harness submits valid and invalid Typst content through the CLI
- **THEN** it verifies success and explicit render failure behavior

#### Scenario: Time timeline command is tested

- **WHEN** records exist in multiple companies
- **THEN** the CLI timeline command returns only data for the selected company
