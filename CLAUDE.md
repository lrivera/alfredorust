# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run (development)
cargo run

# Hot-reload development (default job is `run-long`)
bacon

# Tests
cargo test

# Run a single test
cargo test <test_name>

# Run tests in a specific file
cargo test --test <test_file_name>
```

The app runs on port **8090**. Integration tests use an isolated MongoDB DB named `alfredodevtest_*`.

## Architecture

**alfredorust** is a multi-tenant financial management web app built with Axum (Rust). It uses TOTP-based authentication, MongoDB for persistence, Askama for HTML templating, and Typst for PDF generation.

### Multi-tenancy

Companies are tenants selected via subdomain (e.g., `company1.localhost:8090`). All financial entities are scoped by `company_id`. Users can belong to multiple companies with per-company roles (Admin/Staff), bridged by the `UserWithCompany` struct and the `user_companies` collection.

### Authentication

- TOTP login: email + 6-digit code (no password)
- Sessions stored in MongoDB with 24-hour TTL
- `session.rs` provides middleware and an extractor that injects `UserWithCompany` into protected handlers
- Public routes: `/`, `/login`, `/secret`, `/setup`, `/qrcode`
- All `/admin/*`, `/account`, `/pdf`, `/tiempo` routes are session-protected

### Code Structure

| Path | Purpose |
|------|---------|
| `src/main.rs` | Router wiring — all route registrations |
| `src/models.rs` | All domain types (User, Company, Account, Category, Transaction, RecurringPlan, PlannedEntry, Forecast) |
| `src/state/mod.rs` | `AppState` struct with MongoDB collection handles |
| `src/state/users.rs` | User and session management functions |
| `src/state/companies.rs` | Company CRUD |
| `src/state/finance.rs` | Finance entity CRUD (accounts, categories, contacts, recurring plans, planned entries, transactions, forecasts) |
| `src/routes/` | HTTP handlers grouped by feature |
| `src/templates/` | Askama HTML templates |
| `data/` | Seed JSON files loaded once if DB is empty |

### Key Domain Enums

- `FlowType`: `Income` / `Expense`
- `AccountType`: `Bank` / `Cash` / `CreditCard` / `Investment` / `Other`
- `TransactionType`: `Income` / `Expense` / `Transfer`
- `PlannedStatus`: `Planned` / `PartiallyCovered` / `Covered` / `Overdue` / `Cancelled`

`RecurringPlan` has a `version` field — incrementing it marks existing `PlannedEntry` records as outdated.

### Environment

Configure via `.env` (excluded from git):
- `MONGODB_URI` — MongoDB connection string
- `MONGODB_DB` — database name
