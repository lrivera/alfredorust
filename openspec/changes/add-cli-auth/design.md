## Context

The server already authenticates users through passwordless email plus TOTP and stores sessions in MongoDB. Protected routes enforce authorization through the session cookie and active company context selected from a trusted tenant host. The CLI should act as another client of that HTTP surface while storing the user's TOTP secret locally so non-technical users do not need to repeatedly enter short-lived TOTP codes.

## Goals / Non-Goals

**Goals:**

- Provide a first CLI command set that stores email plus TOTP secret once.
- Generate TOTP codes locally to create or refresh normal server sessions transparently.
- Make tenant context explicit through a base URL and company slug option.
- Keep login failures non-enumerable and server-controlled.
- Add harness coverage for login and stored-session behavior.
- Make the command surface predictable enough for a future AI skill to call safely.

**Non-Goals:**

- Do not store passwords, SAT/FIEL secrets, or production secrets in the CLI.
- Do not let locally generated TOTP codes bypass server-side user, company, role, or permission checks.
- Do not bypass server-side authorization or query MongoDB directly from the CLI.
- Do not implement every business command in the first change.
- Do not add shell-interactive flows that block automated use.
- Do not implement the AI skill in this change.

## Decisions

- Implement the CLI as a Rust binary in the same repository.
  - Rationale: it reuses the project toolchain and can share typed request/response shapes where useful.
  - Alternative considered: a separate Node or Python CLI. That would add another runtime and duplicate project conventions.
- Name the binary `spcli`.
  - Rationale: the name is generic enough to avoid tying the tool to a personal or project codename while remaining short for shell usage.
  - Alternative considered: `alfredo`, `alfredodev`, or `alfredorust`. Those names are less generic and make the CLI feel project-personal instead of product-operator oriented.
- Configure the CLI by storing the user's email and TOTP secret locally.
  - Rationale: this lets the CLI generate current codes automatically and keeps the user experience transparent after initial setup.
  - Alternative considered: direct MongoDB session creation from the CLI. That would bypass auth logic and tenant safety.
- Store the TOTP secret in a recoverable local form, not as a hash.
  - Rationale: a hash cannot be used to generate future TOTP codes; the CLI must recover the original secret to compute a current code.
  - Alternative considered: hashing the secret. That would prevent automatic code generation and fail the transparent-login requirement.
- Store credentials in a binary encrypted envelope, not plaintext JSON.
  - Rationale: opening the file in a text editor must not reveal the TOTP secret, email, cookie, or company context. The envelope should be opaque to casual inspection and protected with authenticated encryption.
  - Alternative considered: plaintext JSON with restrictive permissions. That is simpler but too easy to read if a beginner accidentally exposes the file.
- Prefer OS keyring for the envelope key; fallback to a derived local key.
  - Rationale: keyring-backed storage uses platform credential protection when available. For systems without keyring, derive a wrapping key from local machine/user material plus an app salt and server/user salt, then use it to decrypt the binary envelope.
  - Alternative considered: a server-only unlock hash. That creates a bootstrap problem: when the session expires, the CLI needs the TOTP secret before it can authenticate to the server. A server salt can strengthen binding, but the decrypt key must be available locally for transparent re-login.
- Transparently re-login with email plus generated TOTP code after `401 Unauthorized` or missing session.
  - Rationale: protected routes continue to use normal server session and permission logic, while the CLI hides session expiration from users.
  - Alternative considered: create separate server-side CLI tokens. That improves revocation, but the explicit product decision is to store the TOTP secret for maximum beginner usability.
- Require explicit server context.
  - Rationale: local and production tenant hosts differ, and protected requests need the same host semantics as browser requests.
  - Alternative considered: infer tenant from email. That would conflict with multi-company users and subdomain-based tenancy.
- Use `--base-url` plus `company use <slug>` for normal company selection.
  - Rationale: the user logs into a server once, then switches company context by slug as needed; the CLI derives tenant host context from the selected company and saved base URL.
  - Alternative considered: require `--host <slug.localhost:8090>` on login and every command. That is explicit but repetitive and easier to mistype.
- Design commands for both humans and a future AI skill.
  - Rationale: humans need friendly defaults, while agents need deterministic output, stable names, clear errors, and no hidden prompts.
  - Alternative considered: optimize only for interactive human use first. That would likely require breaking command/output changes later when building the AI skill.
- Use human-readable output by default and stable JSON with `--json` for automation.
  - Rationale: the same command can serve users and agents without separate APIs or duplicated behavior.
  - Alternative considered: JSON-only output. That is better for automation but worse for beginner users.
- Require destructive commands to be explicit and non-interactive.
  - Rationale: a future skill must not get stuck on prompts or accidentally delete data. Destructive operations should require flags such as `--yes` and return structured warnings/errors.
  - Alternative considered: interactive confirmation prompts. That is safer for manual use but blocks automation and agent workflows.

## Risks / Trade-offs

- TOTP secret exposure -> The local credential file can generate valid codes if decrypted; mitigate with user-scoped storage outside the repo, restrictive file permissions, binary encrypted envelope storage, and clear documentation.
- Hashing misconception -> Do not hash the TOTP secret because the CLI must recover it; use reversible storage and be explicit about the risk.
- Binary storage misconception -> Binary format alone is not a security boundary; pair it with authenticated encryption and local key derivation.
- Same-user compromise -> If malware runs as the same OS user, it may still access keyring or local derivation inputs; rotating the TOTP secret server-side remains the recovery path.
- Host mismatch -> Persist and display the configured base URL and tenant host so users can inspect the active target before running commands.
- Login response shape drift -> Prefer a small typed client and integration tests against the in-memory Axum router or local server.
- Session expiration -> protected commands should generate a fresh TOTP code, login again, update the session cookie, and retry once.
- Agent misuse -> Keep command names, JSON schemas, exit codes, and error codes documented so an AI skill can reason about failures instead of scraping human prose.

## Migration Plan

- Add the CLI without changing browser behavior.
- Keep existing browser `/login` semantics unchanged.
- Reuse existing browser login semantics for CLI-generated TOTP codes unless response shape changes are needed.
- Add commands incrementally after auth is established.
- Rollback is removing the CLI binary and dependency additions; no server-side token records are introduced.

## Open Questions

- Should the first CLI support only JSON output, or both human-readable and JSON output?
- Which keyring crate and encrypted-envelope format should be used for Linux/macOS/Windows support?
- Which machine-readable documentation format should drive the future AI skill: generated command manifest JSON, OpenAPI-like metadata, or both?
- Should logout delete only the local session cookie, or delete the stored TOTP secret as well?
