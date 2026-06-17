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

## Pending

- `spcli admin users ...` commands and JSON APIs.
- A product decision for safe user TOTP provisioning before exposing user creation.
- CLI-safe CFDI import/download commands, including secret handling and job lifecycle constraints.
- Optional hardening work for OS keyring support and stronger local key derivation.
- Full command documentation for every implemented command and permission boundary.

## Blocked Decision

User admin creation cannot be implemented safely until the TOTP secret provisioning contract is chosen. Current options are:

- Accept `--secret-env` and never print the secret.
- Add a one-time provisioning flow that returns setup material only once through a dedicated command.
- Ship only user list/get/update/delete first and leave create unsupported.

Until this is decided, `spcli admin users create` remains out of scope.
