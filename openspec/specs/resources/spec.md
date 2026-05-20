# Resources Specification

## Purpose

Resources, resource logs, resource usage grids, resource usage allocations, and timeline visibility.

## Requirements

### Requirement: Resource management is admin-gated

The system SHALL require admin access for resource CRUD and administrative resource usage management.

#### Scenario: Staff attempts resource CRUD

- GIVEN a staff user
- WHEN they submit resource CRUD routes
- THEN the system rejects the request unless a specific permitted staff workflow applies

### Requirement: Staff resource usage edits are time-limited

The system SHALL allow permitted staff to edit resource usage only for today through four days before today, inclusive.

#### Scenario: Staff edits recent usage

- GIVEN a staff user with resource usage permission
- WHEN they edit usage for today or one of the previous four days
- THEN the system allows the operation when other validations pass

#### Scenario: Staff edits old or future usage

- GIVEN a staff user with resource usage permission
- WHEN they edit usage older than four days or in the future
- THEN the system rejects the operation

### Requirement: Timeline access is permission-gated

The system SHALL require timeline permission for staff access to `/tiempo` and timeline API data.

#### Scenario: Staff without timeline permission

- GIVEN a staff user without timeline permission
- WHEN they request timeline routes
- THEN the system rejects the request
