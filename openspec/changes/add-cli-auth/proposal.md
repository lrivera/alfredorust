## Why

The application now exposes enough protected HTTP APIs that operators need a scriptable way to authenticate and call them without using the browser. A first-party CLI should let non-technical users configure authentication once, transparently regenerate TOTP codes, and re-login when sessions expire while also creating a stable command surface that a future AI skill can safely automate.

## What Changes

- Add a local CLI entry point for the existing application crate.
- Add CLI authentication commands for storing the user's TOTP secret, transparent re-login, session status, and logout.
- Let the CLI generate current TOTP codes locally when it needs to recover from expired or rejected sessions.
- Add CLI company selection and company switching for multi-company users.
- Establish the CLI command structure that later maps every supported protected API capability to documented CLI commands.
- Keep commands automation-friendly with stable JSON output, explicit exit codes, non-interactive flags, and machine-readable documentation.
- Store the TOTP secret and current session locally in a user-scoped config file, not in the repository.
- Require the CLI to target an explicit base URL and tenant host context before calling protected APIs.
- Keep protected API authorization on the server; the CLI uses its stored TOTP secret only to create normal server sessions.

## Capabilities

### New Capabilities

- `cli-auth`: Covers local TOTP secret persistence, transparent re-login, tenant-aware request context, status, and logout behavior.
- `cli-api`: Covers the long-term CLI command surface for protected API requests and command documentation.

### Modified Capabilities

- `auth`: Adds a CLI client scenario that uses the existing passwordless TOTP login behavior without changing browser login semantics.

## Impact

- Affected code: `Cargo.toml`, `src/bin/`, auth route client behavior, company/profile API client behavior, test harness utilities, and possibly shared route response models.
- Affected APIs: existing `/login`, protected session validation, company/profile endpoints, protected business APIs, and logout behavior.
- Dependencies: likely `clap` for command parsing, `reqwest` for HTTP, and a small user-directory helper for storing CLI session state.
- Systems: local developer/operator machines and existing Axum server instances.
