# Auth Specification

## Purpose

Authentication, session creation, session validation, logout, and TOTP enrollment behavior.

## Requirements

### Requirement: Passwordless TOTP login

The system SHALL authenticate users with email plus a valid TOTP code.

#### Scenario: Valid login creates session

- GIVEN a seeded user with a valid TOTP secret
- WHEN the user submits their email and a current TOTP code
- THEN the system creates a MongoDB-backed session
- AND returns a successful login response

#### Scenario: Invalid login is not enumerable

- GIVEN an unknown email or invalid TOTP code
- WHEN login is attempted
- THEN the system returns an unauthorized response
- AND does not reveal whether the email exists

### Requirement: Session expiration

The system SHALL expire sessions after the configured session TTL.

#### Scenario: Expired session is rejected

- GIVEN an expired session token
- WHEN a protected route is requested
- THEN the system rejects the request as unauthorized

### Requirement: TOTP setup is protected

The system SHALL expose TOTP setup and QR code data only to an authenticated user for their active session context.

#### Scenario: Authenticated user requests setup

- GIVEN an authenticated user
- WHEN they request setup or QR code data
- THEN the system returns data for that user and active company context only
