# CFDI Specification

## Purpose

SAT configuration, CFDI download jobs, XML/ZIP import, CFDI-derived commitments, and optional payment creation.

## Requirements

### Requirement: SAT configs are company-scoped secrets

The system SHALL store SAT config references per company and restrict access to admins of that company.

#### Scenario: Admin creates SAT config

- GIVEN an admin for a company
- WHEN they upload valid SAT certificate files and metadata
- THEN the system stores the config for that company

#### Scenario: Unrelated admin targets SAT config

- GIVEN an admin for company A only
- WHEN they target company B SAT config
- THEN the system rejects the operation

### Requirement: CFDI download jobs are company scoped

The system SHALL associate download jobs with the company that started them and restrict job visibility to admins of that company.

#### Scenario: Job status by unrelated company

- GIVEN a job for company A
- WHEN a company B admin requests that job status
- THEN the system rejects the request

### Requirement: Downloaded CFDIs create commitments by default

The system SHALL create or update CFDI-backed planned entries instead of creating transactions directly by default.

#### Scenario: CFDI downloaded without automatic payments

- GIVEN a valid issued or received CFDI
- WHEN it is imported from a download job with automatic payments disabled
- THEN the system creates or updates a planned entry keyed by CFDI UUID
- AND does not create a transaction

### Requirement: Automatic CFDI payments are explicit

The system SHALL create automatic transactions only when the user explicitly enables automatic payments.

#### Scenario: CFDI downloaded with automatic payments enabled

- GIVEN a valid CFDI-backed planned entry
- WHEN automatic payments are enabled
- THEN the system creates a transaction if one does not already exist for that planned entry
