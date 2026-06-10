## ADDED Requirements

### Requirement: CLI exposes CFDI read commands through tenant-scoped JSON APIs

The system SHALL provide CFDI CLI read commands that use authenticated JSON APIs and active company context.

#### Scenario: CFDI list is requested

- **WHEN** the user runs `spcli cfdi list` with a valid session and selected company
- **THEN** the CLI consumes the CFDI JSON data endpoint
- **AND** returns only CFDIs for the active company
- **AND** includes stable fields for UUID, folio, type, date, totals, currency, payment metadata, issuer, receiver, concept, and issued/received direction

#### Scenario: CFDI detail is requested

- **WHEN** the user runs `spcli cfdi get <uuid>`
- **THEN** the backend SHALL provide a JSON detail endpoint before the CLI command is implemented
- **AND** the endpoint SHALL enforce company ownership of the CFDI

### Requirement: CLI exposes CFDI download jobs safely

The system SHALL expose CFDI download job commands only with explicit company context, admin checks, and documented job persistence limitations.

#### Scenario: CFDI download is started

- **WHEN** the user runs `spcli cfdi download start` with a SAT config, date range, and download type
- **THEN** the CLI sends an authenticated request to start one or more jobs
- **AND** the JSON response includes job ids and labels
- **AND** the command documentation explains financial side effects from imported CFDIs and optional automatic payment creation

#### Scenario: CFDI jobs are listed

- **WHEN** the user runs `spcli cfdi jobs list`
- **THEN** the CLI returns jobs for the selected company only
- **AND** the documentation states that current jobs are in-memory and do not survive server restarts unless persistent jobs are implemented

### Requirement: CLI protects SAT and CFDI secret material

The system SHALL avoid exposing certificate passwords, key material, certificate paths, key paths, TOTP secrets, cookies, `otpauth_url`, or generated TOTP codes in CLI output.

#### Scenario: SAT config data is returned for CFDI workflows

- **WHEN** the CLI lists or references SAT configs
- **THEN** secret-bearing fields are redacted or omitted
- **AND** JSON output remains useful for selecting a config by id, alias, RFC, and active state

### Requirement: CLI does not expose risky SAT APIs without review

The system SHALL require security review before exposing direct SAT download APIs that accept certificate paths or password-bearing payloads.

#### Scenario: Direct SAT download endpoint exists

- **WHEN** `POST /api/sat/cfdi/download` is considered for CLI use
- **THEN** the change SHALL document accepted inputs, rejected inputs, secret handling, output files, and error behavior before adding a command

### Requirement: CFDI CLI behavior is covered by harness tests

The system SHALL add harness coverage for CFDI CLI behavior where local fixtures can exercise it.

#### Scenario: CFDI list is tested

- **WHEN** fixture CFDIs exist for multiple companies
- **THEN** `spcli cfdi list` returns only the selected company's CFDIs

#### Scenario: CFDI download job authorization is tested

- **WHEN** a non-admin user starts or reads CFDI jobs
- **THEN** the server rejects the request
- **AND** the CLI reports a structured forbidden error
