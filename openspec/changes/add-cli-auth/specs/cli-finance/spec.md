## ADDED Requirements

### Requirement: CLI exposes finance read commands through JSON APIs

The system SHALL provide finance CLI read commands that use authenticated, tenant-scoped JSON APIs instead of scraping HTML pages.

#### Scenario: Finance account list is requested

- **WHEN** the user runs `spcli finance accounts list` with a valid session and selected company
- **THEN** the CLI sends an authenticated request using the selected tenant context
- **AND** returns the active company's accounts in the requested output format
- **AND** excludes accounts from other companies

#### Scenario: Finance transaction list is requested

- **WHEN** the user runs `spcli finance transactions list` with JSON output enabled
- **THEN** the CLI returns a JSON array of transaction rows for the active company
- **AND** includes stable fields for id, date, description, type, amount, account names, category, contact, confirmation state, currency, CFDI folio, and notes

#### Scenario: Finance transaction detail is requested

- **WHEN** the user runs `spcli finance transactions get <id>` with a transaction from the active company
- **THEN** the CLI returns a JSON object with the transaction's canonical ObjectId fields and financial attributes
- **AND** rejects transaction IDs from another company with a forbidden response

### Requirement: CLI finance mutations require explicit backend APIs

The system SHALL only implement finance create, update, delete, pay, generate, and bulk-pay CLI commands after backend JSON APIs exist for those actions.

#### Scenario: Finance command targets an HTML-only capability

- **WHEN** a finance capability is only available through an HTML form or redirect handler
- **THEN** `spcli` SHALL NOT scrape or submit the HTML page as its command contract
- **AND** the implementation SHALL first add a documented JSON API for the capability

#### Scenario: Finance delete is requested

- **WHEN** the user runs a finance delete command without `--yes`
- **THEN** the CLI rejects the operation before sending the request
- **AND** returns a structured error describing the required confirmation flag

### Requirement: CLI finance commands document financial side effects

The system SHALL document side effects for every finance CLI command that can create, update, delete, pay, generate, or link financial records.

#### Scenario: Planned entry payment is requested

- **WHEN** the user runs a planned-entry payment command
- **THEN** the command documentation identifies the transaction records that may be created or linked
- **AND** the JSON response includes a side-effect summary with created, updated, linked, or skipped records

#### Scenario: Recurring plan generation is requested

- **WHEN** the user runs a recurring-plan generate command
- **THEN** the JSON response includes generated planned-entry counts and any skipped or outdated records

### Requirement: CLI finance command groups are stable

The system SHALL organize finance commands under stable nouns.

#### Scenario: Finance command manifest is requested

- **WHEN** the user runs `spcli --json manifest`
- **THEN** the manifest includes finance command metadata for accounts, categories, contacts, recurring plans, planned entries, transactions, and forecasts as they are implemented
- **AND** each entry identifies auth requirements, company context, permissions, destructive flags, arguments, and output schema names

### Requirement: CLI finance behavior is covered by harness tests

The system SHALL add harness coverage for representative finance CLI behavior.

#### Scenario: Finance tenant isolation is tested

- **WHEN** finance CLI read commands are tested
- **THEN** the harness creates records in at least two isolated companies
- **AND** verifies the CLI returns only records for the selected company

#### Scenario: Finance side effects are tested

- **WHEN** a finance CLI command creates or links financial records
- **THEN** the harness verifies the persisted MongoDB records and returned side-effect summary
