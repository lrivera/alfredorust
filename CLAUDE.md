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

Companies are tenants selected via subdomain: `slug.alfredorivera.dev` in production, `slug.localhost:8090` in local dev. All financial entities are scoped by `company_id`. Users can belong to multiple companies with per-company roles (Admin/Staff), bridged by the `UserWithCompany` struct and the `user_companies` collection.

`app.alfredorivera.dev` is reserved exclusively for the login page — the slug `app` is blocked in `state/companies.rs` via `RESERVED_SLUGS`.

### Authentication

- TOTP login: email + 6-digit code (no password)
- Sessions stored in MongoDB with 24-hour TTL
- `session.rs` provides middleware and an extractor that injects `UserWithCompany` into protected handlers
- Public routes: `/`, `/login`, `/secret`, `/setup`, `/qrcode`
- All `/admin/*`, `/account`, `/pdf`, `/tiempo` routes are session-protected
- After login, the app redirects to `https://slug.alfredorivera.dev` using `BASE_DOMAIN` env var

### Code Structure

| Path | Purpose |
|------|---------|
| `src/main.rs` | Router wiring — all route registrations |
| `src/models.rs` | All domain types (User, Company, Account, Category, Transaction, RecurringPlan, PlannedEntry, Forecast) |
| `src/state/mod.rs` | `AppState` struct with MongoDB collection handles |
| `src/state/users.rs` | User and session management functions |
| `src/state/companies.rs` | Company CRUD — contains `RESERVED_SLUGS` constant |
| `src/state/finance.rs` | Finance entity CRUD (accounts, categories, contacts, recurring plans, planned entries, transactions, forecasts) |
| `src/routes/login.rs` | Login handler + `compute_redirect_url` / `compute_root_domain` / `set_cookies_for_host` |
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
- `MONGODB_URI` — MongoDB Atlas connection string (`mongodb+srv://demo:...@cluster0.s3ja5ef.mongodb.net/`)
- `MONGODB_DB` — database name (`alfredodev`)
- `BASE_DOMAIN` — root domain for tenant routing (`alfredorivera.dev` in prod, omit for localhost)

---

## Production Infrastructure

### Server
- **IP:** `134.199.216.25`
- **OS:** Ubuntu 24.10 (EOL — apt sources point to `old-releases.ubuntu.com`)
- **SSH:** `ssh alfredo@134.199.216.25`
- **App binary:** `/home/alfredo/alfredorust/alfredodev`
- **Repo clone:** `/home/alfredo/alfredorust/`
- **Env file:** `/home/alfredo/alfredorust/.env`

### Systemd Service
```bash
sudo systemctl restart alfredorust   # restart app
sudo systemctl status alfredorust    # check status
sudo journalctl -u alfredorust -f    # live logs
```
Service file: `/etc/systemd/system/alfredorust.service`
Runs as user `alfredo`, reads `.env` via `EnvironmentFile`.

### Nginx
Config: `/etc/nginx/sites-available/app.alfredorivera.dev`
- Listens on port 80 and 443
- `server_name *.alfredorivera.dev` → proxies to `127.0.0.1:8090`
- WordPress at `alfredorivera.dev` and `www.alfredorivera.dev` is handled by a separate config
- SSL cert used is for `alfredorivera.dev` (Let's Encrypt via Certbot)

```bash
sudo nginx -t && sudo systemctl reload nginx   # test and reload
```

### DNS (Cloudflare)
Domain `alfredorivera.dev` managed in Cloudflare. Relevant records:
| Type | Name | Value | Proxy |
|------|------|-------|-------|
| A | `*` | `134.199.216.25` | Proxied (orange) |
| A | `app` | `134.199.216.25` | Proxied |
| A | `alfredorivera.dev` | `134.199.216.25` | Proxied |

**SSL/TLS mode: Flexible** — Cloudflare handles HTTPS with browsers, sends HTTP to origin.
The wildcard `*.alfredorivera.dev` DNS covers all company slugs (e.g. `research.alfredorivera.dev`).

### CI/CD (GitHub Actions)
- **Repo:** `https://github.com/lrivera/alfredorust`
- Workflow: `.github/workflows/deploy.yml`
- On push to `main`: builds release binary on GitHub's Ubuntu runner, SCPs binary to server, restarts `alfredorust` service
- Cache: `~/.cargo/registry` + `target/` cached by `Cargo.lock` hash
- Required GitHub Secrets: `SSH_HOST`, `SSH_USER`, `SSH_PRIVATE_KEY`
- Build takes ~3-5 min first time, faster with cache hit
