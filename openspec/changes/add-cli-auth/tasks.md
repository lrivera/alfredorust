## 0. API Surface Planning

- [x] 0.1 Inventory existing protected routes and classify JSON-ready versus HTML/form-only capabilities.
- [x] 0.2 Identify current `spcli` coverage and backend routes consumed by existing commands.
- [x] 0.3 Identify backend API gaps that must be filled before implementing full CLI parity with the web UI.
- [x] 0.4 Add domain specs for finance, projects, resources, CFDI/SAT, admin, PDF, and time CLI coverage.
- [x] 0.5 Resolve open product/API questions before implementing broad command groups.

## 1. CLI Shape

- [x] 1.1 Add a Rust CLI binary entry point named `spcli` for the project.
- [x] 1.2 Add command parsing for `login`, `status`, `logout`, and `company` command groups.
- [x] 1.3 Define CLI options for server base URL, company slug, email, TOTP secret, and output format.
- [x] 1.4 Establish a command naming convention for resource groups, list/get/create/update/delete actions, and JSON output.
- [x] 1.5 Establish stable exit codes and structured error codes for future automation.
- [x] 1.6 Ensure commands are non-interactive by default and use explicit flags for confirmations.

## 2. Authentication Client

- [x] 2.1 Implement local TOTP code generation from the stored TOTP secret.
- [x] 2.2 Implement login request handling against the existing server login route using generated TOTP codes.
- [x] 2.3 Extract and persist the returned session cookie after successful login.
- [x] 2.4 Ensure invalid login does not create or overwrite credential state.
- [x] 2.5 Implement transparent re-login using the stored TOTP secret when the session cookie is missing or rejected.

## 3. Local Credential Store

- [x] 3.1 Store credential data in a user-scoped file outside the repository.
- [x] 3.2 Persist base URL, selected company slug, derived tenant host context, email, recoverable TOTP secret, session cookie, and login metadata inside a binary encrypted envelope.
- [x] 3.3 Use restrictive file permissions where supported.
- [ ] 3.4 Use OS keyring for the envelope key when available.
- [x] 3.5 Never hash the TOTP secret because automatic code generation requires recovering it.
- [ ] 3.6 Add fallback local key derivation from machine/user material, app salt, and server/user salt when keyring is unavailable.
- [x] 3.7 Use authenticated encryption for the credential envelope and reject tampered files loudly.
- [x] 3.8 Ensure opening the credential file in a text editor does not reveal plaintext credentials.

## 4. Session Store

- [x] 4.1 Separate session cookie updates from credential setup so re-login can refresh only session fields.
- [x] 4.2 Persist session cookie and last-login metadata after each transparent re-login.
- [x] 4.3 Avoid logging the TOTP secret, generated TOTP codes, or session cookie.

## 5. Status And Logout

- [x] 5.1 Implement `status` using a protected session-aware endpoint with transparent re-login when needed.
- [x] 5.2 Make rejected sessions fail loudly only when generated-login recovery also fails.
- [x] 5.3 Implement `logout` by removing local session state.
- [x] 5.4 Implement full logout or reset that removes the stored TOTP secret.

## 6. Company Selection

- [x] 6.1 Implement `company list` using the authenticated profile/company API.
- [x] 6.2 Implement `company use <slug>` or equivalent company switch command.
- [x] 6.3 Persist the selected company host context for subsequent commands.
- [x] 6.4 Reject unavailable company selections without overwriting the previous active company.

## 7. Core API Command Surface

- [ ] 7.1 Add authenticated CLI helpers for GET, POST, PUT/PATCH, and DELETE requests with transparent re-login, session cookie, and tenant host headers.
- [ ] 7.2 Add shared parsing for ObjectId parameters, dates, datetimes, amounts, enum values, and pagination.
- [x] 7.3 Add consistent error handling for unauthorized, forbidden, not found, validation, and server errors.
- [x] 7.4 Add `--json` output for structured responses and keep diagnostics on stderr.
- [x] 7.5 Add a shared output layer for human output, JSON output, and structured errors.
- [x] 7.6 Add a command manifest generator or static manifest for future AI skill integration.
- [x] 7.7 Require explicit confirmation flags for destructive commands and reject them otherwise.

## 8. Finance Commands

- [ ] 8.1 Add account list/get/create/update/delete commands.
- [ ] 8.2 Add category list/get/create/update/delete commands.
- [ ] 8.3 Add contact list/get/create/update/delete commands.
- [ ] 8.4 Add recurring plan list/get/create/update/delete/generate commands.
- [ ] 8.5 Add planned entry list/get/create/update/delete/pay/bulk-pay commands.
- [ ] 8.6 Add transaction list/get/create/update/delete commands.
- [x] 8.6.1 Add transaction create/update/delete JSON APIs and CLI commands with planned-entry recalculation side-effect responses.
- [ ] 8.7 Add forecast list/get/create/update/delete commands.

## 9. Operations Commands

- [ ] 9.1 Add service order list/get/create/update/delete/complete commands.
- [ ] 9.2 Add project list/get/create/update/delete commands.
- [x] 9.2.1 Add project list/get JSON APIs and CLI commands.
- [ ] 9.3 Add project concept list/create/update/delete/advance commands.
- [x] 9.3.1 Add project concept create/update/delete/advance and status-summary CLI commands over existing JSON APIs.
- [ ] 9.4 Add concept status list/create/update/delete commands.
- [x] 9.4.1 Add concept status create/update/delete CLI commands over existing JSON APIs.

## 10. Resource And Time Commands

- [ ] 10.1 Add resource list/get/create/update/delete commands.
- [x] 10.1.1 Add resource list/get JSON APIs and CLI commands.
- [ ] 10.2 Add resource log list/get/create/update/delete/start/end commands.
- [x] 10.2.1 Add resource log list/get JSON APIs and CLI commands.
- [ ] 10.3 Add resource usage list/get/create/update/delete commands.
- [x] 10.3.1 Add resource usage get JSON API and CLI command.
- [x] 10.3.2 Add resource usage create/update/delete and allocation CLI commands over existing JSON APIs.
- [ ] 10.4 Add timeline query commands equivalent to `/tiempo` API data access.

## 11. SAT, CFDI, And PDF Commands

- [ ] 11.1 Add SAT config list/get/create/update/delete commands where server APIs support them.
- [x] 11.1.1 Add redacted SAT config list/get JSON APIs and CLI commands.
- [x] 11.2 Add CFDI list/detail commands.
- [ ] 11.3 Add CFDI import/download/job list/job status commands.
- [x] 11.3.1 Add CFDI job list/status CLI commands over existing company-scoped job endpoints.
- [ ] 11.4 Add PDF preview/render commands where server APIs support them.

## 12. Admin And Setup Commands

- [ ] 12.1 Add user list/get/create/update/delete commands for company admins.
- [ ] 12.2 Add setup/profile/status commands that expose current user, role, permissions, and companies.
- [ ] 12.3 Add company admin commands for company metadata and maintenance endpoints.

## 13. Documentation

- [x] 13.1 Add CLI README documentation with installation, TOTP secret setup, company selection, automatic re-login, and logout examples.
- [ ] 13.2 Document every implemented CLI command with required permissions and underlying API capability.
- [x] 13.3 Document JSON output contracts and common error responses.
- [ ] 13.4 Add examples for scripting common finance, project, resource, SAT/CFDI, and admin workflows.
- [ ] 13.5 Add machine-readable command metadata covering arguments, auth requirements, company context, permissions, output schemas, and destructive flags.
- [x] 13.6 Add a short note explaining that the CLI is designed to support a future AI skill, while the skill itself is out of scope for this change.

## 14. Verification

- [ ] 14.1 Add harness coverage for successful CLI setup against the app router or test server.
- [ ] 14.2 Add harness coverage for invalid generated login not writing credential state.
- [ ] 14.3 Add harness coverage for status with valid session, expired session plus valid TOTP secret, and expired session plus rejected generated login.
- [ ] 14.4 Add harness coverage for transparent re-login and single retry behavior after unauthorized responses.
- [ ] 14.5 Add harness coverage for company list and company switch.
- [ ] 14.6 Add representative harness coverage for each CLI command group.
- [x] 14.7 Add tests for JSON output, structured errors, exit codes, and destructive command confirmation behavior.
- [x] 14.8 Run `openspec validate --all` and relevant Rust tests.

## 15. First Read-Only JSON API Slice

- [x] 15.1 Add shared authenticated JSON `GET` and `POST` helpers with transparent re-login retry.
- [x] 15.2 Add `schema_version` and read-only command entries to the manifest.
- [x] 15.3 Add `finance transactions list` using the existing transaction dashboard JSON API.
- [x] 15.4 Add `cfdi list` using the existing CFDI dashboard JSON API.
- [x] 15.5 Add `projects statuses list` and `projects concepts list --project-id` using existing project JSON APIs.
- [x] 15.6 Add `resources usages list` using the existing resource usage JSON API.
- [x] 15.7 Add `time timeline` using the existing timeline JSON API.
- [x] 15.8 Add `pdf preview` using the existing PDF preview JSON API.
- [ ] 15.9 Add harness coverage for the first read-only JSON API slice.

## 16. Finance Master Data JSON API Slice

- [x] 16.1 Add tenant-scoped JSON list APIs for accounts, categories, and contacts.
- [x] 16.2 Add `finance accounts list` using the accounts JSON API.
- [x] 16.3 Add `finance categories list` using the categories JSON API.
- [x] 16.4 Add `finance contacts list` using the contacts JSON API.
- [x] 16.5 Add tenant-scoped JSON detail APIs for accounts, categories, and contacts.
- [x] 16.6 Add `finance accounts get <id>`, `finance categories get <id>`, and `finance contacts get <id>` commands.
- [x] 16.7 Add tenant-scoped JSON create APIs for accounts, categories, and contacts.
- [x] 16.8 Add `finance accounts create`, `finance categories create`, and `finance contacts create` commands.
- [x] 16.9 Add tenant-scoped JSON update/delete APIs for accounts, categories, and contacts.
- [x] 16.10 Add `finance accounts/categories/contacts update` and `delete --yes` commands.
- [x] 16.11 Add tenant-scoped JSON list/get/create/update/delete APIs for forecasts.
- [x] 16.12 Add `finance forecasts list/get/create/update/delete` commands.
- [x] 16.13 Add tenant-scoped JSON list/get APIs for recurring plans and planned entries.
- [x] 16.14 Add `finance recurring-plans list/get` and `finance planned-entries list/get` commands.
- [x] 16.15 Add harness coverage for finance master data, forecast, recurring plan, and planned entry JSON APIs.
- [x] 16.16 Add tenant-scoped JSON detail API and CLI command for transaction reads.
