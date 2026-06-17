# spcli Implementation Status

## Completed

- Authentication and session commands: `login`, `status`, `logout`, `reset-auth`.
- Local encrypted credential envelope with stored base URL, tenant host, email, TOTP secret, session cookie, and login metadata.
- Transparent re-login on missing or rejected sessions using locally generated TOTP codes.
- Company context commands: `company list`, `company use`.
- Account profile commands: `account get`, `account update --totp-secret-env` with profile reads redacting the TOTP secret.
- Finance commands for accounts, categories, contacts, forecasts, recurring plans, planned entries, and transactions.
- Service order commands for list/get/create/update/delete/complete.
- Project commands for projects, concept statuses, concepts, status summary, and workflow advance operations.
- Resource commands for resources, resource logs, resource usages, and usage allocations.
- SAT config commands for redacted list/get/create/update/delete, with passwords read from environment variables.
- CFDI read commands for list/get plus in-memory job list/status.
- Time timeline and PDF preview commands over existing JSON APIs.
- Static `spcli manifest` with auth, company-context, argument, destructive, and output-schema metadata for implemented commands.
- Harness coverage for representative JSON APIs, tenant isolation, admin/staff permission checks, redaction, mutation side effects, structured CLI errors, manifest output, and destructive confirmation checks.

## Completed (HTTP API surface for the SPA)

These close the last functional gaps that were still HTML/form-only, so a SPA (or `spcli`) can drive 100% of the app through JSON:

- Users admin JSON API: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `POST /api/admin/users`, `POST /api/admin/users/{id}/update`, `POST /api/admin/users/{id}/delete`. Admin-only and tenant-scoped (an admin only sees/manages users sharing a company where they hold Admin). The user secret is never returned in JSON; new-user provisioning material is read through the existing protected QR endpoint.
- Resource usage hourly grid bulk-save JSON API: `POST /api/admin/resource_usages/grid` — JSON twin of the form grid save, honoring the same staff "today window" permission.
- SAT config creation with file upload JSON API: `POST /api/admin/sat-configs/upload` — multipart `.cer`/`.key` upload (the prior JSON create only accepted server-side paths).
- OpenAPI/Swagger documentation (utoipa) covering the full JSON API surface, served at `/docs` (Swagger UI) and `/api-docs/openapi.json`. Both are mounted behind the session middleware, so the docs are only reachable once logged in; auth is modeled as the `session` cookie scheme.
- Integration tests for all three new endpoints (CRUD + scoping, grid permission window, multipart upload + redaction) and a unit test asserting the OpenAPI document is complete and secured.

## Completed (spcli desktop CLI parity)

- `spcli admin users list/get/create/update/delete` over `/api/admin/users*`, with `--secret-env` (optional; server generates otherwise) and `--input <file.json>` for multi-membership payloads.
- `spcli sat configs upload` — multipart `.cer`/`.key` upload over `/api/admin/sat-configs/upload`, with the key password read from `--key-password-env`.
- `spcli resources usages grid` — bulk hourly grid save over `/api/admin/resource_usages/grid`, with repeatable `--cell concept_id:hour:resource_id` or `--input`.
- All three added to the static `spcli manifest` (auth/company/destructive/argument/output metadata) and covered by CLI tests (manifest presence, destructive confirmation, local validation). The CLI authenticates exclusively with the locally-stored TOTP secret (transparent re-login via generated codes); other auth methods are out of scope for now.

## Pending
- CLI-safe CFDI import/download commands, including secret handling and job lifecycle constraints.
- Optional hardening work for OS keyring support and stronger local key derivation.
- Full command documentation for every implemented command and permission boundary.
- Per-operation OpenAPI response schemas (request bodies and status codes are documented; typed response bodies are described but not fully modeled).

## Resolved Decision (user TOTP provisioning)

The provisioning contract is now settled for the HTTP/SPA API: on create the server generates the TOTP secret when the client does not supply one, the secret is never returned in any JSON response, and an admin reads the new user's QR/secret through the existing protected `GET /admin/users/{id}/qrcode` endpoint. `spcli admin users create` can adopt the same contract (accept an optional `--secret-env`, never print the secret).
