# Tenancy Specification

## Purpose

Tenant selection, tenant isolation, company membership, and cross-company authorization behavior.

## Requirements

### Requirement: Tenant selection by trusted subdomain

The system SHALL select active company context from a trusted tenant subdomain when present.

#### Scenario: Production subdomain matches configured base domain

- GIVEN `BASE_DOMAIN` is configured
- WHEN a request host is `<company-slug>.<BASE_DOMAIN>`
- THEN the active company is selected from the matching user membership

#### Scenario: Untrusted host is ignored

- GIVEN a request host outside the configured base domain
- WHEN protected routes are requested
- THEN the host does not select a tenant context

### Requirement: Business records are company scoped

The system SHALL scope business records by active `company_id` unless a route is explicitly global.

#### Scenario: User requests another tenant record by ID

- GIVEN a user belongs to company A
- WHEN they request a company B business record by ID
- THEN the system rejects the request or behaves as not found

### Requirement: Admin rights are per company

The system SHALL treat admin access as company-specific, not global across all companies.

#### Scenario: Admin manages own company

- GIVEN a user is Admin for company A
- WHEN they manage company A users, SAT configs, or finance data
- THEN the operation is allowed when other validations pass

#### Scenario: Admin targets unrelated company

- GIVEN a user is Admin for company A only
- WHEN they target company B data
- THEN the operation is forbidden
