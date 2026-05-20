# Projects Specification

## Purpose

Project workflow, project concepts, project status summaries, project financial visibility, and links between projects and finance records.

## Requirements

### Requirement: Project access is permission-gated

The system SHALL require project permissions for staff project visibility.

#### Scenario: Staff without project permission

- GIVEN a staff user without project view permission
- WHEN they request project routes
- THEN the system rejects the request

### Requirement: Project money visibility is separate

The system SHALL hide project financial amounts from staff unless they have explicit money visibility permission.

#### Scenario: Staff views projects without money permission

- GIVEN a staff user with project view permission but without money permission
- WHEN they view project pages
- THEN financial amounts are hidden or omitted

### Requirement: Finance records can link to projects

The system SHALL allow planned entries and transactions to optionally link to projects in the same company.

#### Scenario: Payment linked to project

- GIVEN a payable commitment and project in the same company
- WHEN an admin pays the commitment with the project selected
- THEN the created transaction stores the project link
