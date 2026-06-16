## ADDED Requirements

### Requirement: CLI exposes admin commands only through explicit JSON APIs

The system SHALL provide admin CLI commands for users, companies, account profile, and SAT configs only after backend JSON APIs exist for those capabilities.

#### Scenario: User list is requested

- **WHEN** the user runs `spcli admin users list` with admin privileges
- **THEN** the CLI consumes a JSON user list endpoint
- **AND** the endpoint returns users visible in the selected company context

#### Scenario: Company metadata is updated

- **WHEN** the user runs `spcli admin companies update <id>`
- **THEN** the CLI consumes a JSON company update endpoint
- **AND** the server enforces admin authorization and tenant constraints

### Requirement: CLI admin commands avoid leaking sensitive setup data

The system SHALL redact sensitive setup and SAT config fields from CLI output.

#### Scenario: User setup data is returned

- **WHEN** the CLI returns user setup or QR-related data
- **THEN** it omits TOTP secrets, generated TOTP codes, cookies, and `otpauth_url` unless a future spec explicitly adds a one-time secret provisioning command

#### Scenario: SAT config is returned

- **WHEN** the CLI returns SAT config data
- **THEN** it omits or redacts certificate passwords, key passwords, and other secret-bearing fields

#### Scenario: SAT config is created or changed

- **WHEN** the user runs `spcli sat configs create`, `update`, or `delete --yes`
- **THEN** the CLI consumes tenant-scoped SAT config JSON APIs
- **AND** create and update read the key password from `--key-password-env` instead of a raw command argument
- **AND** responses omit certificate paths and key passwords

### Requirement: CLI admin destructive commands require confirmation or remain unsupported

The system SHALL require explicit confirmation flags for supported destructive admin commands and SHALL keep dangerous company-wide maintenance commands unsupported until a future spec adds audit and recovery requirements.

#### Scenario: Supported admin delete is requested

- **WHEN** the user runs a supported admin delete command without `--yes`
- **THEN** the CLI rejects the command before sending the request

#### Scenario: Company maintenance delete-all command is requested

- **WHEN** a user attempts to delete a company, delete all company CFDIs, or delete all company transactions through `spcli`
- **THEN** the CLI reports that the command is unsupported
- **AND** no backend request is sent
- **AND** a future spec SHALL be required before exposing those operations through the CLI

### Requirement: CLI admin commands document permission boundaries

The system SHALL document role and permission requirements for each admin CLI command.

#### Scenario: Staff user runs admin command

- **WHEN** a staff user runs an admin-only CLI command
- **THEN** the server rejects the request
- **AND** the CLI returns a structured forbidden error

### Requirement: Admin CLI behavior is covered by harness tests

The system SHALL add harness coverage for representative admin CLI behavior.

#### Scenario: User admin command is tested

- **WHEN** the harness runs user admin commands as admin and staff users
- **THEN** it verifies admin success and staff rejection

#### Scenario: SAT config redaction is tested

- **WHEN** the harness reads SAT config data through the CLI
- **THEN** secret-bearing fields are absent or redacted in JSON output

#### Scenario: SAT config mutation is tested

- **WHEN** the harness creates, updates, and deletes SAT configs through JSON APIs
- **THEN** it verifies active-tenant scoping and redacted responses
