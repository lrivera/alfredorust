# Finance Specification

## Purpose

Accounts, categories, contacts, recurring plans, planned entries, transactions, forecasts, payment behavior, and financial side effects.

## Requirements

### Requirement: Planned entries represent commitments

The system SHALL use planned entries to represent expected or real commitments before payment.

#### Scenario: Planned entry is payable

- GIVEN a planned entry that is not covered or cancelled
- WHEN an admin pays it
- THEN the system creates a transaction linked to the planned entry
- AND recalculates planned entry status

### Requirement: Transactions represent real movements

The system SHALL use transactions to represent real financial movements.

#### Scenario: Payment transaction from expense commitment

- GIVEN an expense planned entry
- WHEN it is paid from an account
- THEN the system creates an expense transaction with `account_from_id`

#### Scenario: Payment transaction from income commitment

- GIVEN an income planned entry
- WHEN it is paid into an account
- THEN the system creates an income transaction with `account_to_id`

### Requirement: Payment links are tenant safe

The system SHALL validate that planned entries, accounts, categories, contacts, projects, and parent planned entries belong to the active company before creating or updating payment records.

#### Scenario: Cross-tenant project ID is submitted

- GIVEN an admin for company A
- WHEN they submit a company B project ID in a payment form
- THEN the system rejects the operation

### Requirement: Bulk payment creates one transaction per commitment

The system SHALL support bulk payment of selected payable planned entries.

#### Scenario: Admin bulk pays two commitments

- GIVEN two payable planned entries for the active company
- WHEN the admin submits the bulk payment form
- THEN two transactions are created
- AND each transaction is linked to its planned entry
