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

### Requirement: CLI publishes machine-readable command metadata

The system SHALL provide machine-readable command metadata suitable for generating docs and future AI skill instructions.

#### Scenario: Command manifest requested

- **WHEN** the user or future AI skill requests the CLI command manifest
- **THEN** the CLI returns command names, descriptions, arguments, required auth state, required company context, permissions, output schemas, and destructive-operation flags

#### Scenario: New command is added

- **WHEN** a new CLI command is implemented
- **THEN** its command manifest entry is added or generated in the same change
