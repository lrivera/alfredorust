## ADDED Requirements

### Requirement: CLI exposes protected API capabilities as documented commands

The system SHALL provide a CLI command surface that maps protected API capabilities to documented commands after authentication is available.

#### Scenario: Authenticated API command succeeds

- **WHEN** the user runs a CLI command for a protected API capability with a valid stored session and selected company
- **THEN** the CLI sends the request with the stored session cookie and tenant host context
- **AND** displays the server response in the requested output format

#### Scenario: Protected API command lacks session

- **WHEN** the user runs a protected API CLI command without a valid stored session
- **THEN** the CLI fails loudly and instructs the user to run login

#### Scenario: CLI capability only exists as HTML

- **WHEN** a protected capability is only available through HTML templates, browser forms, or redirects
- **THEN** `spcli` SHALL NOT scrape HTML or depend on redirects as its command contract
- **AND** a documented JSON API SHALL be added before exposing the capability through the CLI

#### Scenario: New backend API is added for CLI

- **WHEN** a new backend API is added for CLI coverage
- **THEN** it SHOULD use an explicit `/api/admin/*` JSON route where the capability is administrative
- **AND** it SHALL keep tenant isolation and permission checks in the backend handler

### Requirement: CLI command names are canonical and stable

The system SHALL use canonical English command names for CLI commands and SHALL avoid aliases until a future spec requires them.

#### Scenario: User runs a supported command

- **WHEN** the user runs a CLI command
- **THEN** the command name follows the documented English noun hierarchy
- **AND** the command remains stable for automation and future AI skill use

#### Scenario: Spanish or shorthand alias is requested

- **WHEN** a Spanish, shorthand, or ambiguous alias is considered
- **THEN** it SHALL NOT be added in the initial CLI coverage work
- **AND** a future spec SHALL define aliases only if there is a concrete need

### Requirement: CLI list commands support explicit pagination and filters

The system SHALL make list commands predictable by using explicit pagination, limits, and filters.

#### Scenario: List command is requested without pagination flags

- **WHEN** the user runs a list command without pagination flags
- **THEN** the CLI uses a documented default limit
- **AND** the default SHALL be 100 unless the command spec states otherwise

#### Scenario: List command requests a large limit

- **WHEN** the user supplies `--limit`
- **THEN** the CLI and backend SHALL enforce a documented maximum
- **AND** the maximum SHALL NOT exceed 5000 unless a command-specific spec justifies it

#### Scenario: List command uses filters

- **WHEN** the user filters a list command
- **THEN** the filters are explicit flags such as `--from`, `--to`, `--status`, `--type`, `--project-id`, or `--resource-id`
- **AND** the CLI SHALL NOT perform hidden fuzzy matching or implicit search behavior

### Requirement: CLI uses canonical identifiers and explicit input formats

The system SHALL use MongoDB `ObjectId` values as canonical identifiers and stable formats for typed inputs.

#### Scenario: Command targets a persisted record

- **WHEN** the user targets an existing record
- **THEN** the command accepts the record's `ObjectId` as the canonical identifier
- **AND** name, slug, or fuzzy lookup support SHALL wait for a future spec unless the model already has a stable slug field

#### Scenario: Command accepts dates or datetimes

- **WHEN** the user supplies dates or datetimes
- **THEN** dates use `YYYY-MM-DD`
- **AND** datetimes use RFC3339 unless a command-specific spec states otherwise

#### Scenario: Command accepts large or nested payloads

- **WHEN** a create or update command needs many fields or nested data
- **THEN** the CLI supports `--input <file.json>` in addition to simple flags where practical
- **AND** JSON input failures return structured validation errors

### Requirement: CLI documentation covers every implemented command

The system SHALL document every implemented CLI command, option, required authentication state, output format, and common failure mode.

#### Scenario: New CLI command is added

- **WHEN** a new CLI command is implemented
- **THEN** the command help text and repository documentation describe how to use it
- **AND** the documentation identifies the underlying API behavior or capability it exercises

### Requirement: CLI output supports humans and automation

The system SHALL support human-readable output by default and JSON output for automation where commands return structured data.

#### Scenario: JSON output requested

- **WHEN** the user runs a data-returning CLI command with JSON output enabled
- **THEN** the CLI prints valid JSON to stdout
- **AND** sends diagnostic errors to stderr

#### Scenario: Human output requested

- **WHEN** the user runs a CLI command without JSON output enabled
- **THEN** the CLI prints concise human-readable output

### Requirement: CLI error contracts and exit codes are stable

The system SHALL map CLI failures to stable structured error codes and documented exit codes.

#### Scenario: CLI command fails with JSON output enabled

- **WHEN** a CLI command fails and `--json` is enabled
- **THEN** the CLI returns an error object with `code`, `message`, and `details`
- **AND** `details` includes relevant context such as HTTP status, path, argument name, or server response when safe to expose

#### Scenario: CLI command maps common failures

- **WHEN** a CLI command fails
- **THEN** the CLI maps failures to stable codes including `not_authenticated`, `unauthorized`, `forbidden`, `not_found`, `validation_error`, `conflict`, `server_error`, `network_error`, `invalid_credentials`, and `confirmation_required`

#### Scenario: CLI exits after success or failure

- **WHEN** a CLI command exits
- **THEN** it uses documented exit codes: `0` success, `1` generic error, `2` validation or usage error, `3` auth error, `4` forbidden, `5` not found, `6` conflict, and `7` network or server error

### Requirement: CLI never exposes secret material in output

The system SHALL prevent secrets from appearing in human output, JSON output, logs, diagnostics, and command manifests.

#### Scenario: Command response contains sensitive fields

- **WHEN** server data includes TOTP secrets, generated TOTP codes, cookies, SAT passwords, certificate/key material, certificate/key paths, or `otpauth_url`
- **THEN** the CLI omits or redacts those fields before printing output
- **AND** the command documentation identifies the redaction behavior

#### Scenario: Command accepts secret-bearing input

- **WHEN** a command accepts secret-bearing input such as a TOTP secret or SAT password
- **THEN** the CLI SHALL NOT echo the secret in output or structured errors
- **AND** failure details SHALL avoid including the raw submitted value

### Requirement: CLI commands are safe for future AI skill automation

The system SHALL keep CLI commands deterministic, non-interactive by default, and documented with stable names, arguments, exit codes, and JSON schemas.

#### Scenario: AI skill calls a read command

- **WHEN** a future AI skill runs a read-only CLI command with JSON output enabled
- **THEN** the command returns valid JSON with a stable schema
- **AND** exits with a documented status code

#### Scenario: AI skill calls a destructive command

- **WHEN** a future AI skill runs a destructive CLI command without explicit confirmation flags
- **THEN** the command rejects the operation before sending the request
- **AND** returns a structured error explaining the required confirmation flag

#### Scenario: CLI command fails

- **WHEN** a CLI command fails with JSON output enabled
- **THEN** the CLI writes a structured error object to stderr or stdout according to the documented contract
- **AND** includes a stable error code, human message, and non-zero exit code

### Requirement: CLI mutations are explicit and backend-authorized

The system SHALL keep mutation behavior explicit in the CLI while relying on backend handlers as the authority for tenant isolation, permissions, validation, and side effects.

#### Scenario: Mutation command is requested

- **WHEN** the user runs a create, update, delete, pay, generate, import, download, or complete command
- **THEN** the CLI sends an authenticated request to a backend API that performs authorization and tenant checks
- **AND** the CLI SHALL NOT treat local validation as a substitute for backend enforcement

#### Scenario: Destructive mutation is requested

- **WHEN** the user runs a destructive mutation command without its required confirmation flag
- **THEN** the CLI rejects the operation before sending the request
- **AND** returns `confirmation_required`

#### Scenario: Financial or administrative mutation supports dry run

- **WHEN** a future spec adds `--dry-run` for a mutation command
- **THEN** the backend SHALL perform validation without persisting side effects
- **AND** the response SHALL describe what would have changed

#### Scenario: Sensitive create command needs idempotency

- **WHEN** a future spec adds idempotency for repeated or financially sensitive create commands
- **THEN** the command SHALL accept an explicit `--idempotency-key`
- **AND** the backend SHALL enforce idempotent behavior for that key

### Requirement: CLI publishes machine-readable command metadata

The system SHALL provide machine-readable command metadata suitable for generating docs and future AI skill instructions.

#### Scenario: Command manifest requested

- **WHEN** the user or future AI skill requests the CLI command manifest
- **THEN** the CLI returns `schema_version`, command names, descriptions, arguments, required auth state, required company context, permissions, output schemas, and destructive-operation flags

#### Scenario: New command is added

- **WHEN** a new CLI command is implemented
- **THEN** its command manifest entry is added or generated in the same change

#### Scenario: Manifest schema changes

- **WHEN** the manifest format changes incompatibly
- **THEN** `schema_version` is incremented
- **AND** the documentation describes the compatibility impact for automation clients
