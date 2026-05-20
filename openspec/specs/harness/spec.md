# Harness Specification

## Purpose

Local verification infrastructure that proves application behavior without production services or production data.

## Requirements

### Requirement: Isolated MongoDB databases

The harness SHALL use isolated MongoDB databases for integration tests.

#### Scenario: Integration test setup

- GIVEN MongoDB is available locally
- WHEN a test calls `common::setup_state()`
- THEN the harness creates a unique `alfredodevtest_*` database
- AND initializes application state against that database

### Requirement: In-memory HTTP routing

The harness SHALL test protected routes through in-memory Axum routers when possible.

#### Scenario: Authenticated HTTP test

- GIVEN a test user and session token
- WHEN the harness sends a request with `Host` and session cookie headers
- THEN the route executes through normal middleware and handlers

### Requirement: Fixtures are safe to commit

The harness SHALL only use synthetic, redacted, or non-sensitive fixture files.

#### Scenario: CFDI fixture added

- GIVEN a developer adds a CFDI XML or ZIP fixture
- WHEN it is committed
- THEN it contains no real customer data, real certificates, private keys, passwords, or production SAT downloads

### Requirement: External systems are not required for normal tests

The harness SHALL avoid real SAT network calls, real FIEL material, production MongoDB, and production secrets during normal tests.

#### Scenario: SAT behavior needs coverage

- GIVEN code depends on SAT download behavior
- WHEN tests need to verify it
- THEN the harness uses safe fixtures or a fake seam instead of calling SAT
