---
name: spcli
description: >-
  Operate the Sonora Precision / alfredodev financial platform from the command
  line via `spcli`. Use this whenever the user wants to read or change their
  company data on the platform — accounts, categories, contacts, recurring
  plans, planned entries, transactions, forecasts, service orders, projects,
  concept statuses, project concepts, resources, resource logs, resource usage
  (incl. the hourly grid), CFDIs and SAT configs, company users, the time
  timeline, or PDF previews. Triggers include requests like "list my accounts",
  "create a transaction", "show project X concepts", "register resource usage",
  "add a user to company Y", "what CFDIs do I have", or any query/mutation of
  platform records. `spcli` authenticates once with a TOTP secret and exposes a
  stable, machine-readable JSON command surface designed for automation.
---

# spcli — drive the platform from the CLI

`spcli` is the first-party command-line client for the multi-tenant financial
platform (Axum/MongoDB backend). It logs in once with a TOTP secret, stores an
encrypted local session, transparently re-logs in when the session expires, and
exposes ~115 commands with **stable JSON output and structured errors**. Always
prefer `spcli` over calling the HTTP API directly — it handles auth, the tenant
host, re-login, and validation for you.

> Login identifier: the platform now calls it the **username** (a unique handle,
> not a validated email). For backward compatibility the CLI flag is still
> `--email` and the env var `SPCLI_EMAIL`; both just hold that username value.

The full command reference lives in [`docs/spcli.md`](../../../docs/spcli.md).
The machine-readable command catalog is `spcli --json manifest` — consult it to
discover the exact command, arguments, auth/company requirements, destructive
flag, and output schema before acting.

## 0. Always use `--json`

Run every command as `spcli --json <command>` so you get parseable output.
Success prints JSON to **stdout**; errors print JSON to **stderr** with a stable
`code` (`not_authenticated`, `validation_error`, `forbidden`, `not_found`,
`confirmation_required`, `network_error`, `server_error`, …) and exit codes
(0 ok, 2 validation/confirmation, 3 not authenticated, 5 not found, 7 network).

## 1. Locate the binary

From the repo root, build once and reuse the binary:

```bash
cargo build --release --bin spcli >/dev/null 2>&1
SPCLI=./target/release/spcli            # fall back to ./target/debug/spcli if that's what exists
```

If you are not in the repo, use `cargo run -q --bin spcli --` instead of a path.
In examples below, `spcli` means "the located binary".

## 2. Authenticate (check first, log in only if needed)

Always check the current session before doing work:

```bash
spcli --json status
```

- If it returns the user/company/role → you're authenticated; continue.
- If it errors with `code: "not_authenticated"` → you must log in. **Ask the user
  for the base URL, email, and TOTP secret** (never invent them; never echo the
  secret into logs or chat). Then:

```bash
spcli --json login --base-url <APP_LOGIN_URL> --email <EMAIL> --totp-secret <BASE32_SECRET>
```

> **Critical — base URL must be the app/login host, not a tenant host.** Use the
> reserved login host (e.g. `https://app.alfredorivera.dev`), *not*
> `https://<tenant>.alfredorivera.dev`. `spcli` derives the tenant host by
> prepending the company slug to the base host, so a tenant URL becomes an
> invalid double subdomain (`tenant.tenant…`). If `company use` later errors with
> "looks like a tenant host", you logged in against the wrong base URL — re-login
> against the app host. For local dev the base URL is `http://localhost:8090`.

After login, pick the active company (tenant). Most data commands are
company-scoped, so do this once per session:

```bash
spcli --json company list          # which companies the user can access
spcli --json company use <slug>    # select the active tenant, e.g. "test"
```

## 3. Discover the right command

Don't guess command names — list them with metadata:

```bash
spcli --json manifest
```

Each entry has `name`, `auth_required`, `company_required`, `destructive`,
`confirmation_flag`, `arguments`, and `output_schema`. The command groups are:
`account`, `company`, `admin` (companies, users), `finance` (accounts,
categories, contacts, recurring-plans, planned-entries, transactions, forecasts),
`orders`, `projects` (incl. statuses, concepts), `resources` (incl. logs, usages,
usages allocations, usages grid), `sat` (configs), `cfdi`, `time`, `pdf`,
plus `status`, `login`, `logout`, `reset-auth`, `manifest`.

For argument shapes and worked examples per command, read
[`docs/spcli.md`](../../../docs/spcli.md).

## 4. Conventions when running commands

- **Read:** `list` returns an array; `get <id>` returns one record. IDs are
  MongoDB ObjectId hex strings (24 chars).
- **Create:** returns `{ "id": "<new id>" }` (status 201). Capture the id for
  follow-up commands.
- **Update:** `update <id> --field ...` returns `{ "ok": true }` (often with a
  `side_effects` object for finance mutations).
- **Delete:** destructive — must pass `--yes` (otherwise exit 2,
  `confirmation_required`). **Confirm with the user before deleting.**
- **Dates:** RFC3339 (`2026-07-01T00:00:00Z`) or `YYYY-MM-DD` (sent as midnight
  UTC). **Enums** are lowercase strings (`bank`, `expense`, `customer`,
  `monthly`, `machinery`, …) — check `docs/spcli.md` for valid values.
- **Secrets:** SAT config passwords and user TOTP secrets are passed by
  **environment variable name** (`--key-password-env`, `--secret-env`), never as
  literals. The server redacts secrets in responses — never print them.
- **Nested/bulk payloads:** several commands accept `--input <file.json>` for the
  full request body (e.g. `admin users create`, `resources usages grid`).

## 5. Common tasks

```bash
# Finance
spcli --json finance accounts list
spcli --json finance accounts create --name "BBVA" --account-type bank --currency MXN
spcli --json finance transactions create --date 2026-07-01T12:00:00Z --description "Fuel" \
  --transaction-type expense --category-id <CAT> --account-from-id <ACC> --amount 500

# Projects & resources
spcli --json projects list
spcli --json projects concepts list --project-id <PROJ>
spcli --json resources usages grid --date 2026-06-17 --status-id all --cell <CONCEPT>:8:<RESOURCE>

# Admin (admin-only, tenant-scoped; the new user's secret is never printed —
# read its QR from the web app)
spcli --json admin users list
spcli --json admin users create --email new@example.com --company-id <COMPANY> --role staff --permission view_projects

# CFDI / SAT (reads + jobs; live SAT download is initiated from the web app)
spcli --json cfdi list
spcli --json sat configs list

# Time & PDF
spcli --json time timeline --mode month --from 2026-01-01 --to 2026-12-31
spcli --json pdf preview --source "= Hello"
```

## 6. How to work a request

1. Ensure auth (`status`; log in if `not_authenticated`) and an active company
   (`company use <slug>`).
2. Find the command via `manifest` / `docs/spcli.md`; resolve any ids you need by
   listing first (e.g. get the category id before creating a transaction).
3. Run with `--json`; parse stdout. On a non-zero exit, read the JSON error on
   stderr and act on its `code` (re-login, fix validation, ask for confirmation,
   report not-found, etc.).
4. Summarize the result for the user in plain language (don't dump raw JSON
   unless asked). For multi-step changes, confirm destructive steps first.

## Safety

- Never print, log, or echo the TOTP secret, generated codes, session cookie, or
  SAT passwords. Pass secrets only via env-var-name flags.
- Always require explicit user confirmation before any `delete` / `--yes`,
  before `reset-auth`, and before bulk mutations.
- Backend handlers remain the source of truth for permissions, tenant isolation,
  and validation; `spcli` is a convenience client, not a trust boundary.
