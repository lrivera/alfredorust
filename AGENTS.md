# AGENTS.md

Guidance for coding agents working in this repository.

## Project Concept

`alfredorust` / `alfredodev` is a multi-tenant business management web app for small-company operations. It started as a TOTP-authenticated Axum app and now combines finance, operations, SAT/CFDI tooling, time/resource tracking, and PDF generation.

Core product areas:

- Multi-company tenant platform selected by subdomain.
- Passwordless TOTP login with MongoDB-backed sessions.
- Finance administration: accounts, categories, contacts, recurring plans, planned entries, transactions, forecasts.
- Operational workflow: service orders, projects, resources, and resource logs.
- Mexican fiscal tooling: SAT FIEL configuration, massive CFDI download jobs, CFDI XML/ZIP import, and transaction creation from invoices.
- Utility screens: `/tiempo` time view and Typst-backed PDF editor/preview.

## Commands

```bash
# Build
cargo build

# Run locally, app listens on port 8090
cargo run

# Hot reload development, default job is run-long
bacon

# Tests
cargo test

# Single test
cargo test <test_name>

# Integration test file
cargo test --test <test_file_name>
```

Integration tests use isolated MongoDB databases named `alfredodevtest_*` and may skip when MongoDB is unavailable.

## Architecture

The app is built with Axum, MongoDB, Askama templates, TOTP, Typst, and SAT/CFDI integrations.

Important paths:

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Router wiring and route protection boundaries |
| `src/models.rs` | Domain models for auth, companies, finance, orders, projects, resources, SAT |
| `src/state/mod.rs` | `AppState`, MongoDB collection handles, job store, state initialization |
| `src/state/users.rs` | Users, sessions, user-company memberships |
| `src/state/companies.rs` | Company CRUD, slug handling, reserved slugs |
| `src/state/finance.rs` | Finance CRUD and planned-entry payment logic |
| `src/state/orders.rs` | Service order persistence and completion flow |
| `src/state/projects.rs` | Project persistence and status advancement |
| `src/state/resources.rs` | Resource CRUD |
| `src/state/resource_logs.rs` | Resource/time log CRUD |
| `src/state/sat_configs.rs` | SAT FIEL config persistence |
| `src/routes/` | HTTP handlers grouped by feature |
| `src/templates/` | Askama HTML templates, Tailwind via CDN |
| `src/cfdi.rs` | CFDI XML/ZIP parsing and MongoDB upsert |
| `src/sat.rs` | SAT SOAP/FIEL massive download client |
| `data/` | Seed data loaded once when the DB is empty |

## Multi-Tenancy

Companies are tenants. Production tenant URLs use `slug.alfredorivera.dev`; local development supports `slug.localhost:8090`.

Rules to preserve:

- Business records must be scoped by `company_id`.
- Session company context is selected from the current subdomain when present.
- Users can belong to multiple companies through `user_companies`, with per-company `Admin` or `Staff` roles.
- The slug `app` is reserved for the login/root app host and must not become a company slug.
- Avoid cross-tenant list/read/update/delete behavior; query by active company unless the feature is intentionally global.

## Authentication

Login is passwordless: email plus 6-digit TOTP code. Sessions are stored in MongoDB and expire after 24 hours.

Public routes include `/`, `/login`, `/secret`, `/setup`, and `/qrcode`. Protected routes include `/admin/*`, `/account`, `/pdf`, `/tiempo`, and protected APIs registered in `src/main.rs`.

After login, `routes/login.rs` computes a redirect to the user's company subdomain using `BASE_DOMAIN` when configured.

## Domain Notes

Finance entities:

- `Account`, `Category`, `Contact`, `RecurringPlan`, `PlannedEntry`, `Transaction`, `Forecast`.
- `RecurringPlan.version` marks generated planned entries as outdated when the plan changes.
- `PlannedEntry` can be covered by real transactions through the payment flow.
- Transactions may link back to CFDI UUIDs, folios, contacts, planned entries, and currencies.

Operations entities:

- `ServiceOrder` contains items, status, amount, optional contact/category/account/planned-entry links, and completion transaction references.
- `Project` tracks status, priority, budget, schedule, and can advance through its workflow.
- `Resource` and `ResourceLog` support machinery/vehicle/equipment tracking and time usage logs.

SAT/CFDI entities:

- `SatConfig` stores per-company FIEL configuration references.
- CFDIs are imported into the `cfdis` collection and keyed by UUID.
- SAT download jobs are stored in-memory in `AppState.jobs`; do not assume persistence across restarts.

## Environment

Configure via `.env` when needed. Do not commit secrets or real certificate passwords.

- `MONGODB_URI`: MongoDB connection string.
- `MONGODB_DB`: database name.
- `BASE_DOMAIN`: root domain for tenant subdomains.
- `USERS_FILE`: optional seed users file, default `./data/users.json`.
- `TYPST_BIN`: optional Typst executable path, default `typst`.

SAT download paths/passwords may be supplied through stored `SatConfig` records or request/env configuration depending on the route/client usage.

## Development Rules

- Prefer the smallest correct change.
- Keep tenant scoping explicit in new queries and handlers.
- Use existing state helpers and route patterns before adding new abstractions.
- Keep Askama templates consistent with the current Tailwind-based layout.
- Add or update tests for behavior changes, especially tenant isolation, auth, finance side effects, and SAT/CFDI parsing.
- For non-trivial features or security-sensitive changes, create or update an OpenSpec change under `openspec/changes/` and keep current behavior specs under `openspec/specs/`.
- Treat `tests/common`, `tests/fixtures`, and integration tests as the local harness. Prefer real isolated MongoDB behavior and in-memory Axum routers over mocks.
- Do not introduce backwards-compatibility code unless there is persisted data, shipped behavior, external usage, or an explicit requirement.
- Never commit `.env`, FIEL keys, certificates, downloaded CFDI packages, credentials, or production secrets.

## Production Context

Production runs behind Nginx and Cloudflare on `alfredorivera.dev`, with wildcard subdomains proxied to the Axum app on port 8090. CI deploys from pushes to `main` through GitHub Actions.
